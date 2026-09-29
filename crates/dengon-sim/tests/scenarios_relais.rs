//! Scénarios **réels** du DoD de l'US-304, 2/2 : le vrai
//! `dengon-core::relay::Relay`
//! ([`NoeudRelais`](dengon_sim::noeud_relais::NoeudRelais)) — `multihop` et
//! `partition_merge`, qui ont besoin d'un maillage, pas seulement de deux
//! correspondants directs (voir `tests/scenarios_reel.rs` pour `direct`,
//! `recipient_offline`, `sender_offline`, avec
//! `dengon_sim::noeud_client::NoeudClient`).
//!
//! `dengon-core::api::Node` n'appelle jamais `Router::poll_due` (elle ne
//! relaie jamais, voir sa doc de module) — un scénario à 3+ sauts réels a
//! besoin du nœud **relais**, pas du nœud client. Voir la doc de module de
//! `dengon_sim::noeud_relais` pour ce que ça implique (horloge murale,
//! injection d'un paquet, observabilité via `ctx.livrer`).

use dengon_ble::TransportConfig;
use dengon_core::protocol::PacketType;
use dengon_core::relay::WALL_CLOCK_MIN_MS;
use dengon_sim::noeud_relais::{paquet_diffuse, NoeudRelais, MARQUEUR_CACHE, MARQUEUR_RELAYE};
use dengon_sim::reseau::ParametresLien;
use dengon_sim::{NoeudId, Simulation};

fn ajouter(sim: &mut Simulation, pseudo: &str, i: u64, relais: NoeudRelais) -> NoeudId {
    let cfg = TransportConfig {
        local_peer_id: i.to_be_bytes(),
        ..TransportConfig::default()
    };
    sim.ajouter_noeud(Box::new(relais), cfg)
        .unwrap_or_else(|e| panic!("transport de {pseudo} : {e}"))
}

const LIEN: ParametresLien = ParametresLien {
    latence_ms: 20,
    gigue_ms: 0,
    perte_pour_mille: 0,
};

/// Trois relais en chaîne `0 — 1 — 2` : un paquet diffusé injecté au
/// maillon `0` doit être **relayé** par `1` (`MARQUEUR_RELAYE`, après
/// l'avoir mis en cache) et **arriver** jusqu'à `2` (`MARQUEUR_CACHE`),
/// qui ne le relaie pas plus loin (dernier maillon, aucune cible).
#[test]
fn multihop() {
    let r0 = NoeudRelais::new("relais-0", 1, 10);
    let r1 = NoeudRelais::new("relais-1", 2, 20);
    let r2 = NoeudRelais::new("relais-2", 3, 30);

    let mut sim = Simulation::new(dengon_sim::SEED_PAR_DEFAUT);
    let n0 = ajouter(&mut sim, "relais-0", 0, r0);
    let n1 = ajouter(&mut sim, "relais-1", 1, r1);
    let n2 = ajouter(&mut sim, "relais-2", 2, r2);
    sim.reseau().relier(n0, n1, LIEN);
    sim.reseau().relier(n1, n2, LIEN);

    // Laisse les liens s'établir (ANNOUNCE + INVENTORY initiaux) avant
    // d'injecter quoi que ce soit.
    sim.executer_jusqu_a(200, 10);

    let paquet = paquet_diffuse(
        999,
        [0xAA; 8],
        PacketType::SealedEnvelope,
        4,
        WALL_CLOCK_MIN_MS + 200,
        b"attestation de test".to_vec(),
    );
    sim.emettre(n0, &paquet);
    sim.executer_jusqu_a(800, 10);

    assert_eq!(
        sim.livres(n1),
        [MARQUEUR_CACHE.to_vec(), MARQUEUR_RELAYE.to_vec()],
        "le relais du milieu doit recevoir PUIS relayer"
    );
    assert_eq!(
        sim.livres(n2),
        [MARQUEUR_CACHE.to_vec()],
        "le dernier maillon doit recevoir, mais n'a personne à qui relayer"
    );
    assert!(
        sim.livres(n0).is_empty(),
        "le maillon d'origine n'a jamais reçu son propre paquet"
    );
}

/// Quatre relais en chaîne `0 — 1 — 2 — 3`, partitionnés en `{0,1}` et
/// `{2,3}` : un paquet injecté d'un côté doit rester confiné à sa moitié
/// tant que la partition dure, puis atteindre l'autre moitié une fois la
/// jonction reformée (réconciliation d'inventaire à la reconnexion,
/// `sync::inventory` — pas une nouvelle tentative du relais jitté
/// d'origine, dont l'échéance est déjà passée sans cible pendant la
/// partition).
#[test]
fn partition_merge() {
    let r0 = NoeudRelais::new("relais-0", 4, 40);
    let r1 = NoeudRelais::new("relais-1", 5, 50);
    let r2 = NoeudRelais::new("relais-2", 6, 60);
    let r3 = NoeudRelais::new("relais-3", 7, 70);

    let mut sim = Simulation::new(dengon_sim::SEED_PAR_DEFAUT);
    let n0 = ajouter(&mut sim, "relais-0", 0, r0);
    let n1 = ajouter(&mut sim, "relais-1", 1, r1);
    let n2 = ajouter(&mut sim, "relais-2", 2, r2);
    let n3 = ajouter(&mut sim, "relais-3", 3, r3);
    sim.reseau().relier(n0, n1, LIEN);
    sim.reseau().relier(n1, n2, LIEN);
    sim.reseau().relier(n2, n3, LIEN);
    sim.executer_jusqu_a(200, 10);

    sim.reseau().partitionner(&[vec![n0, n1], vec![n2, n3]]);
    sim.executer_jusqu_a(400, 10);

    let gauche = paquet_diffuse(
        111,
        [0xAA; 8],
        PacketType::SealedEnvelope,
        4,
        WALL_CLOCK_MIN_MS + 400,
        b"cote gauche".to_vec(),
    );
    sim.emettre(n0, &gauche);
    sim.executer_jusqu_a(900, 10);

    assert!(
        sim.livres(n1).contains(&MARQUEUR_CACHE.to_vec()),
        "le voisin direct doit recevoir malgré la partition"
    );
    assert!(
        sim.livres(n2).is_empty() && sim.livres(n3).is_empty(),
        "l'autre moitié ne doit rien voir tant que la partition dure"
    );

    // La jonction reforme : la réconciliation d'inventaire doit, avec le
    // temps, faire traverser ce que `n1` a déjà en cache vers `n2`, puis
    // `n3`.
    sim.reseau().reunir();
    sim.executer_jusqu_a(3000, 10);

    assert!(
        sim.livres(n2).contains(&MARQUEUR_CACHE.to_vec()),
        "après la jonction, le paquet doit finir par traverser vers n2"
    );
    assert!(
        sim.livres(n3).contains(&MARQUEUR_CACHE.to_vec()),
        "et jusqu'au bout de la chaîne, n3"
    );
}
