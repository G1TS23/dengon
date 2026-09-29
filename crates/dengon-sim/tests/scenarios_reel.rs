//! Scénarios **réels** du DoD de l'US-304 : le vrai `dengon-core::api::Node`
//! ([`NoeudClient`](dengon_sim::noeud_client::NoeudClient)), pas
//! [`Inondation`](dengon_sim::Inondation) (le bouchon de flood générique des
//! scénarios `.ron`, voir `scenarios/*.ron`).
//!
//! Écrits en Rust plutôt qu'en `.ron` : le format `.ron` existant
//! (`crate::scenario::Action::Emettre`) ne porte pas de destinataire —
//! seul `Inondation` diffuse sans adresse. `dengon-core::api::send_message`
//! exige un `dest_peer_id` : voir la doc de module de
//! `dengon_sim::noeud_client`.
//!
//! Périmètre de ce fichier (revue de portée, US-304) : `direct`,
//! `recipient_offline`, `sender_offline` — les trois scénarios réalisables
//! avec seulement [`NoeudClient`] (deux correspondants, jamais de relais).
//! `multihop` et `partition_merge` ont besoin d'un troisième comportement,
//! le nœud **relais** (`dengon-core::relay::Relay`, pas `api::Node` — cette
//! façade n'appelle jamais `Router::poll_due`, voir sa doc de module) : ils
//! suivent dans une session séparée. Voir
//! `docs/suivi/03-ecarts-conception.md`.

use dengon_ble::TransportConfig;
use dengon_sim::noeud_client::{charge_adressee, NoeudClient};
use dengon_sim::reseau::ParametresLien;
use dengon_sim::{NoeudId, Simulation};

/// Ajoute un [`NoeudClient`] à `sim` ; `i` sert à la fois de `local_peer_id`
/// transport (couche BLE) et de graine — les deux espaces d'identifiants
/// sont indépendants dans le vrai système (une adresse BLE n'est pas un
/// `PeerId` dengon), il n'y a aucune raison de les faire coïncider ici sauf
/// pour la lisibilité des traces de test.
fn ajouter(sim: &mut Simulation, pseudo: &str, i: u64, client: NoeudClient) -> NoeudId {
    let cfg = TransportConfig {
        local_peer_id: i.to_be_bytes(),
        ..TransportConfig::default()
    };
    sim.ajouter_noeud(Box::new(client), cfg)
        .unwrap_or_else(|e| panic!("transport de {pseudo} : {e}"))
}

const LIEN_DIRECT: ParametresLien = ParametresLien {
    latence_ms: 20,
    gigue_ms: 0,
    perte_pour_mille: 0,
};

/// A et B sont connectés dès le départ : le message part et arrive
/// **immédiatement** (remise dès la connexion, `docs/synthese/07` — pas
/// besoin d'attendre la fin de la poignée de main Noise `XX`, le chemin
/// enveloppe suffit dès que le lien est ouvert et le contact connu).
#[test]
fn direct() {
    let mut alice = NoeudClient::new("alice", 1, 10, 100);
    let mut bob = NoeudClient::new("bob", 2, 20, 200);
    alice.add_contact(bob.public_identity());
    bob.add_contact(alice.public_identity());
    let peer_bob = bob.peer_id();

    let mut sim = Simulation::new(dengon_sim::SEED_PAR_DEFAUT);
    let a = ajouter(&mut sim, "alice", 0, alice);
    let b = ajouter(&mut sim, "bob", 1, bob);
    sim.reseau().relier(a, b, LIEN_DIRECT);

    // Laisse le temps à l'échange ANNOUNCE + poignée de main Noise :
    // largement plus que nécessaire (marge de test, pas une contrainte
    // temps réel — l'horloge est virtuelle).
    sim.executer_jusqu_a(200, 10);

    sim.emettre(a, &charge_adressee(peer_bob, "bonjour"));
    sim.executer_jusqu_a(500, 10);

    assert_eq!(
        sim.livres(b),
        [b"bonjour".to_vec()],
        "bob doit recevoir le message"
    );
    assert!(
        sim.livres(a).is_empty(),
        "alice ne se livre pas à elle-même"
    );
}

/// Bob (le destinataire) n'est **jamais connecté** quand Alice envoie : le
/// message reste en attente (chemin enveloppe scellée, dans l'outbox
/// d'Alice, sans lien pour partir) jusqu'à ce que Bob se connecte — la
/// remise est alors automatique (`redeliver_pending_envelopes`,
/// `Node::on_peer_connected`), sans d'action supplémentaire côté Alice.
#[test]
fn recipient_offline() {
    let mut alice = NoeudClient::new("alice", 3, 30, 300);
    let mut bob = NoeudClient::new("bob", 4, 40, 400);
    alice.add_contact(bob.public_identity());
    bob.add_contact(alice.public_identity());
    let peer_bob = bob.peer_id();

    let mut sim = Simulation::new(dengon_sim::SEED_PAR_DEFAUT);
    let a = ajouter(&mut sim, "alice", 0, alice);
    let b = ajouter(&mut sim, "bob", 1, bob);
    // Aucun lien pour l'instant : bob est hors de portée.

    sim.emettre(
        a,
        &charge_adressee(peer_bob, "en attendant que tu reviennes"),
    );
    sim.executer_jusqu_a(200, 10);
    assert!(
        sim.livres(b).is_empty(),
        "bob hors de portée : rien ne doit encore arriver"
    );

    // Bob revient : le lien s'ouvre, l'ANNOUNCE et la poignée de main
    // s'échangent, l'enveloppe en attente part.
    sim.reseau().relier(a, b, LIEN_DIRECT);
    sim.executer_jusqu_a(600, 10);

    assert_eq!(
        sim.livres(b),
        [b"en attendant que tu reviennes".to_vec()],
        "le message doit arriver dès que bob se reconnecte"
    );
}

/// Alice (l'émettrice) envoie **après** avoir coupé son lien avec Bob : le
/// message part quand même (chemin enveloppe, aucune connectivité requise
/// pour composer/chiffrer un message à un contact déjà connu), et n'est
/// physiquement transmis qu'à la reconnexion.
#[test]
fn sender_offline() {
    let mut alice = NoeudClient::new("alice", 5, 50, 500);
    let mut bob = NoeudClient::new("bob", 6, 60, 600);
    alice.add_contact(bob.public_identity());
    bob.add_contact(alice.public_identity());
    let peer_bob = bob.peer_id();

    let mut sim = Simulation::new(dengon_sim::SEED_PAR_DEFAUT);
    let a = ajouter(&mut sim, "alice", 0, alice);
    let b = ajouter(&mut sim, "bob", 1, bob);
    sim.reseau().relier(a, b, LIEN_DIRECT);
    sim.executer_jusqu_a(200, 10);

    // Alice coupe son lien (elle éteint son Bluetooth) avant d'écrire.
    sim.reseau().delier(a, b);
    sim.pas(10);

    let texte = "je t'ecris hors ligne";
    sim.emettre(a, &charge_adressee(peer_bob, texte));
    sim.executer_jusqu_a(400, 10);
    assert!(
        sim.livres(b).is_empty(),
        "alice est hors ligne : rien ne doit encore arriver"
    );

    // Alice revient.
    sim.reseau().relier(a, b, LIEN_DIRECT);
    sim.executer_jusqu_a(800, 10);

    assert_eq!(
        sim.livres(b),
        [texte.as_bytes().to_vec()],
        "le message doit arriver dès qu'alice se reconnecte"
    );
}
