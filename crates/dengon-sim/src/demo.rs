//! Mode démo (`dengon-sim --demo`) : rejoue les 5 scénarios réels du DoD
//! de l'US-304 avec une narration lisible pour l'oral — pas juste un
//! `cargo test` au vert.
//!
//! **Même logique, même graines** que `tests/scenarios_reel.rs`
//! (`NoeudClient`, le vrai `dengon-core::api::Node`) et
//! `tests/scenarios_relais.rs` (`NoeudRelais`, le vrai
//! `dengon-core::relay::Relay`) — ces deux fichiers de test restent la
//! **source de vérité** pour la correction (assertions strictes) ; ce
//! module n'ajoute qu'un texte imprimé à côté du même résultat.

use std::fmt::Write as _;

use dengon_ble::TransportConfig;
use dengon_core::protocol::PacketType;
use dengon_core::relay::WALL_CLOCK_MIN_MS;

use crate::cli::Sortie;
use crate::harness::Comportement;
use crate::noeud_client::{charge_adressee, NoeudClient};
use crate::noeud_relais::{paquet_diffuse, NoeudRelais, MARQUEUR_CACHE, MARQUEUR_RELAYE};
use crate::reseau::ParametresLien;
use crate::{NoeudId, Simulation, SEED_PAR_DEFAUT};

const LIEN: ParametresLien = ParametresLien {
    latence_ms: 20,
    gigue_ms: 0,
    perte_pour_mille: 0,
};

fn ajouter(sim: &mut Simulation, i: u64, comportement: Box<dyn Comportement>) -> NoeudId {
    let cfg = TransportConfig {
        local_peer_id: i.to_be_bytes(),
        ..TransportConfig::default()
    };
    sim.ajouter_noeud(comportement, cfg)
        .unwrap_or_else(|e| unreachable!("transport de démo : {e}"))
}

/// Un scénario de démo : son nom, s'il a réussi, et sa narration.
struct Resultat {
    nom: &'static str,
    reussi: bool,
    texte: String,
}

fn direct() -> Resultat {
    let mut texte = String::new();
    let mut alice = NoeudClient::new("alice", 1, 10, 100);
    let mut bob = NoeudClient::new("bob", 2, 20, 200);
    alice.add_contact(bob.public_identity());
    bob.add_contact(alice.public_identity());
    let peer_bob = bob.peer_id();

    let mut sim = Simulation::new(SEED_PAR_DEFAUT);
    let a = ajouter(&mut sim, 0, Box::new(alice));
    let b = ajouter(&mut sim, 1, Box::new(bob));
    sim.reseau().relier(a, b, LIEN);
    sim.executer_jusqu_a(200, 10);

    let _ = writeln!(texte, "  alice et bob sont déjà connectés.");
    let _ = writeln!(texte, "  alice envoie « bonjour » à bob…");
    sim.emettre(a, &charge_adressee(peer_bob, "bonjour"));
    sim.executer_jusqu_a(500, 10);

    let reussi = sim.livres(b) == [b"bonjour".to_vec()];
    let _ = writeln!(
        texte,
        "  {} bob a {} « bonjour »",
        if reussi { '✓' } else { '✗' },
        if reussi { "reçu" } else { "PAS reçu" }
    );
    Resultat {
        nom: "direct",
        reussi,
        texte,
    }
}

fn recipient_offline() -> Resultat {
    let mut texte = String::new();
    let mut alice = NoeudClient::new("alice", 3, 30, 300);
    let mut bob = NoeudClient::new("bob", 4, 40, 400);
    alice.add_contact(bob.public_identity());
    bob.add_contact(alice.public_identity());
    let peer_bob = bob.peer_id();

    let mut sim = Simulation::new(SEED_PAR_DEFAUT);
    let a = ajouter(&mut sim, 0, Box::new(alice));
    let b = ajouter(&mut sim, 1, Box::new(bob));

    let _ = writeln!(texte, "  bob est hors de portée.");
    let _ = writeln!(
        texte,
        "  alice envoie quand même « en attendant que tu reviennes »…"
    );
    sim.emettre(
        a,
        &charge_adressee(peer_bob, "en attendant que tu reviennes"),
    );
    sim.executer_jusqu_a(200, 10);
    let rien_narrive = sim.livres(b).is_empty();
    let _ = writeln!(
        texte,
        "  {} rien n'arrive tant que bob est absent.",
        if rien_narrive { '✓' } else { '✗' }
    );

    let _ = writeln!(texte, "  bob revient à portée…");
    sim.reseau().relier(a, b, LIEN);
    sim.executer_jusqu_a(600, 10);
    let recu = sim.livres(b) == [b"en attendant que tu reviennes".to_vec()];
    let _ = writeln!(
        texte,
        "  {} le message arrive dès la reconnexion (remise automatique).",
        if recu { '✓' } else { '✗' }
    );

    Resultat {
        nom: "recipient_offline",
        reussi: rien_narrive && recu,
        texte,
    }
}

fn sender_offline() -> Resultat {
    let mut texte = String::new();
    let mut alice = NoeudClient::new("alice", 5, 50, 500);
    let mut bob = NoeudClient::new("bob", 6, 60, 600);
    alice.add_contact(bob.public_identity());
    bob.add_contact(alice.public_identity());
    let peer_bob = bob.peer_id();

    let mut sim = Simulation::new(SEED_PAR_DEFAUT);
    let a = ajouter(&mut sim, 0, Box::new(alice));
    let b = ajouter(&mut sim, 1, Box::new(bob));
    sim.reseau().relier(a, b, LIEN);
    sim.executer_jusqu_a(200, 10);

    let _ = writeln!(texte, "  alice coupe son Bluetooth…");
    sim.reseau().delier(a, b);
    sim.pas(10);

    let texte_msg = "j'ecris hors ligne";
    let _ = writeln!(texte, "  … et écrit quand même « {texte_msg} ».");
    sim.emettre(a, &charge_adressee(peer_bob, texte_msg));
    sim.executer_jusqu_a(400, 10);
    let rien_narrive = sim.livres(b).is_empty();
    let _ = writeln!(
        texte,
        "  {} rien ne part tant qu'alice est hors ligne.",
        if rien_narrive { '✓' } else { '✗' }
    );

    let _ = writeln!(texte, "  alice revient…");
    sim.reseau().relier(a, b, LIEN);
    sim.executer_jusqu_a(800, 10);
    let recu = sim.livres(b) == [texte_msg.as_bytes().to_vec()];
    let _ = writeln!(
        texte,
        "  {} le message part et arrive dès la reconnexion.",
        if recu { '✓' } else { '✗' }
    );

    Resultat {
        nom: "sender_offline",
        reussi: rien_narrive && recu,
        texte,
    }
}

fn multihop() -> Resultat {
    let mut texte = String::new();
    let r0 = NoeudRelais::new("relais-0", 1, 10);
    let r1 = NoeudRelais::new("relais-1", 2, 20);
    let r2 = NoeudRelais::new("relais-2", 3, 30);

    let mut sim = Simulation::new(SEED_PAR_DEFAUT);
    let n0 = ajouter(&mut sim, 0, Box::new(r0));
    let n1 = ajouter(&mut sim, 1, Box::new(r1));
    let n2 = ajouter(&mut sim, 2, Box::new(r2));
    sim.reseau().relier(n0, n1, LIEN);
    sim.reseau().relier(n1, n2, LIEN);
    sim.executer_jusqu_a(200, 10);

    let _ = writeln!(
        texte,
        "  chaîne de 3 relais : relais-0 — relais-1 — relais-2."
    );
    let _ = writeln!(
        texte,
        "  un paquet signé est déposé sur relais-0 (relais-2 est hors de portée directe)…"
    );
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

    let relaye = sim.livres(n1) == [MARQUEUR_CACHE.to_vec(), MARQUEUR_RELAYE.to_vec()];
    let _ = writeln!(
        texte,
        "  {} relais-1 reçoit PUIS relaie vers relais-2.",
        if relaye { '✓' } else { '✗' }
    );
    let arrive = sim.livres(n2) == [MARQUEUR_CACHE.to_vec()];
    let _ = writeln!(
        texte,
        "  {} relais-2 reçoit — bout de chaîne, ne relaie pas plus loin.",
        if arrive { '✓' } else { '✗' }
    );

    Resultat {
        nom: "multihop",
        reussi: relaye && arrive,
        texte,
    }
}

fn partition_merge() -> Resultat {
    let mut texte = String::new();
    let r0 = NoeudRelais::new("relais-0", 4, 40);
    let r1 = NoeudRelais::new("relais-1", 5, 50);
    let r2 = NoeudRelais::new("relais-2", 6, 60);
    let r3 = NoeudRelais::new("relais-3", 7, 70);

    let mut sim = Simulation::new(SEED_PAR_DEFAUT);
    let n0 = ajouter(&mut sim, 0, Box::new(r0));
    let n1 = ajouter(&mut sim, 1, Box::new(r1));
    let n2 = ajouter(&mut sim, 2, Box::new(r2));
    let n3 = ajouter(&mut sim, 3, Box::new(r3));
    sim.reseau().relier(n0, n1, LIEN);
    sim.reseau().relier(n1, n2, LIEN);
    sim.reseau().relier(n2, n3, LIEN);
    sim.executer_jusqu_a(200, 10);

    let _ = writeln!(texte, "  chaîne de 4 relais : 0 — 1 — 2 — 3.");
    let _ = writeln!(texte, "  le réseau se coupe en deux : {{0,1}} | {{2,3}}…");
    sim.reseau().partitionner(&[vec![n0, n1], vec![n2, n3]]);
    sim.executer_jusqu_a(400, 10);

    let _ = writeln!(texte, "  un paquet est déposé côté {{0,1}}…");
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
    let confine = sim.livres(n1).contains(&MARQUEUR_CACHE.to_vec())
        && sim.livres(n2).is_empty()
        && sim.livres(n3).is_empty();
    let _ = writeln!(
        texte,
        "  {} reste confiné à {{0,1}} tant que la coupure dure.",
        if confine { '✓' } else { '✗' }
    );

    let _ = writeln!(texte, "  la jonction se reforme…");
    sim.reseau().reunir();
    sim.executer_jusqu_a(3000, 10);
    let traverse = sim.livres(n2).contains(&MARQUEUR_CACHE.to_vec())
        && sim.livres(n3).contains(&MARQUEUR_CACHE.to_vec());
    let _ = writeln!(
        texte,
        "  {} le paquet finit par traverser jusqu'au bout de la chaîne.",
        if traverse { '✓' } else { '✗' }
    );

    Resultat {
        nom: "partition_merge",
        reussi: confine && traverse,
        texte,
    }
}

/// Rejoue les 5 scénarios réels et rend une [`Sortie`] narrée, dans le même
/// style que le mode `.ron` (`✓`/`✗` par scénario, code de sortie non nul en
/// cas d'échec).
#[must_use]
pub fn executer() -> Sortie {
    let mut texte = String::new();
    let mut succes = true;
    for resultat in [
        direct(),
        recipient_offline(),
        sender_offline(),
        multihop(),
        partition_merge(),
    ] {
        let marque = if resultat.reussi { '✓' } else { '✗' };
        let _ = writeln!(texte, "{marque} {}", resultat.nom);
        texte.push_str(&resultat.texte);
        succes &= resultat.reussi;
    }
    Sortie { texte, succes }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_5_scenarios_reels_reussissent_et_se_narrent() {
        let s = executer();
        assert!(s.succes, "{}", s.texte);
        for nom in [
            "direct",
            "recipient_offline",
            "sender_offline",
            "multihop",
            "partition_merge",
        ] {
            assert!(
                s.texte.contains(&format!("✓ {nom}")),
                "{nom} absent ou en échec :\n{}",
                s.texte
            );
        }
    }
}
