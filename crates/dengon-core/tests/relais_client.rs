//! Deux téléphones (`api::Node`) et un relais ESP32 (`relay::Relay`) en
//! mémoire : les scénarios 2 et 3 du DoD (US-312) avant le vrai matériel.
//!
//! Les téléphones ne sont **jamais** reliés entre eux : tout passe par le
//! relais, comme deux téléphones hors de portée l'un de l'autre. Le harness
//! suit ce que fait `Maillage.kt` côté Android : la première trame de chaque
//! lien est l'`ANNOUNCE` du voisin (→ [`Node::on_neighbor_announced`]), la
//! suite part dans [`Node::on_bytes_received`]. Côté relais, c'est ce que
//! fait `dengon_relay_app.c` : `link_up` (le relais s'annonce), puis
//! `on_frame` pour chaque trame.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use dengon_core::api::{MessageStatus, Node, NodeEvent};
use dengon_core::identity::Identity;
use dengon_core::ledger::Anchor;
use dengon_core::relay::{Relay, RelayConfig, RelaySecrets};
use dengon_core::sync::routing::Now;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

const T0: u64 = 1_800_000_000_000;

fn now(ms: u64) -> Now {
    Now::new(T0 + ms, ms)
}

fn rng(seed: u64) -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(seed)
}

/// Un téléphone : son nœud, et s'il est en ce moment lié au relais.
struct Telephone {
    node: Node,
    /// `LinkId` côté relais (un par téléphone, fixe dans ce harness).
    link: u64,
    lie: bool,
    /// L'`ANNOUNCE` du relais n'a pas encore été lu sur le lien courant.
    attend_annonce: bool,
}

impl Telephone {
    fn nouveau(pseudo: &str, seed: u64, link: u64) -> Self {
        let identity = Identity::generate(pseudo, rng(seed)).unwrap();
        Self {
            node: Node::new(identity, seed),
            link,
            lie: false,
            attend_annonce: false,
        }
    }
}

struct Reseau {
    relais: Relay<u64>,
    relais_id: [u8; 8],
    a: Telephone,
    b: Telephone,
    t: u64,
}

impl Reseau {
    /// Alice et Bob se sont appairés par QR (contacts mutuels), puis se sont
    /// éloignés : ils ne se verront plus qu'à travers le relais.
    fn nouveau() -> Self {
        let relais = Relay::new(
            &RelaySecrets {
                dh_secret: [3; 32],
                sign_seed: [103; 32],
            },
            Anchor::GENESIS,
            RelayConfig::new("relais-test", 3),
        );
        let relais_id = relais.peer_id();
        let mut a = Telephone::nouveau("alice", 1, 1);
        let mut b = Telephone::nouveau("bob", 2, 2);
        a.node.add_contact(b.node.public_identity());
        b.node.add_contact(a.node.public_identity());
        Self {
            relais,
            relais_id,
            a,
            b,
            t: 0,
        }
    }

    fn tel(&mut self, qui: char) -> &mut Telephone {
        if qui == 'a' {
            &mut self.a
        } else {
            &mut self.b
        }
    }

    /// Ouvre le lien téléphone ↔ relais : chacun écrit son `ANNOUNCE` en
    /// première trame, puis tout ce qui s'ensuit est pompé.
    fn lier(&mut self, qui: char) {
        self.t += 1_000;
        let n = now(self.t);
        let tel = self.tel(qui);
        tel.lie = true;
        tel.attend_annonce = true;
        let link = tel.link;
        let annonce = tel.node.announce_packet(n).unwrap();
        self.relais.link_up(link, n);
        self.relais.on_frame(link, &annonce, n);
        self.pomper();
    }

    /// Coupe le lien (téléphone éteint ou parti).
    fn couper(&mut self, qui: char) {
        let relais_id = self.relais_id;
        let tel = self.tel(qui);
        tel.lie = false;
        let link = tel.link;
        tel.node.on_peer_disconnected(relais_id);
        let _ = tel.node.take_outgoing();
        self.relais.link_down(link);
    }

    /// Recopie les trames jusqu'à ce que plus personne n'ait rien à dire.
    /// Une trame vers un lien coupé est perdue, comme à la radio.
    fn pomper(&mut self) {
        for _ in 0..32 {
            self.t += 10;
            let n = now(self.t);
            let mut calme = true;

            self.relais.poll(n);
            for (link, trame) in self.relais.take_outgoing() {
                calme = false;
                let relais_id = self.relais_id;
                let tel = if link == self.a.link {
                    &mut self.a
                } else {
                    &mut self.b
                };
                if !tel.lie {
                    continue;
                }
                if tel.attend_annonce {
                    tel.attend_annonce = false;
                    let pair = tel
                        .node
                        .on_neighbor_announced(&trame, n, rng(self.t))
                        .expect("ANNOUNCE du relais valide");
                    assert_eq!(pair, relais_id);
                } else {
                    tel.node.on_bytes_received(relais_id, &trame, n);
                }
            }

            for tel in [&mut self.a, &mut self.b] {
                for (dest, trame) in tel.node.take_outgoing() {
                    calme = false;
                    // Aucun lien direct entre téléphones : seul le relais
                    // reçoit quelque chose.
                    assert_eq!(dest, self.relais_id, "trame hors du lien relais");
                    if tel.lie {
                        self.relais.on_frame(tel.link, &trame, n);
                    }
                }
            }
            if calme {
                return;
            }
        }
        panic!("le réseau ne se calme pas");
    }

    fn envoyer(&mut self, de: char, texte: &str) -> [u8; 16] {
        self.t += 10;
        let n = now(self.t);
        let (tel, dest) = if de == 'a' {
            (&mut self.a, self.b.node.peer_id())
        } else {
            (&mut self.b, self.a.node.peer_id())
        };
        let id = tel.node.send_message(dest, texte, n, rng(self.t)).unwrap();
        self.pomper();
        id
    }

    /// Dernier statut annoncé pour `msg` par le nœud de `qui` (événements
    /// consommés).
    fn statut(&mut self, qui: char, msg: [u8; 16]) -> Option<MessageStatus> {
        let n = now(self.t);
        self.tel(qui)
            .node
            .poll_events(n)
            .into_iter()
            .filter_map(|e| match e {
                NodeEvent::StatusChanged(id, s) if id == msg => Some(s),
                _ => None,
            })
            .next_back()
    }

    /// Textes reçus par `qui` (événements consommés).
    fn recus(&mut self, qui: char) -> Vec<String> {
        let n = now(self.t);
        self.tel(qui)
            .node
            .poll_events(n)
            .into_iter()
            .filter_map(|e| match e {
                NodeEvent::MessageReceived(m) => Some(m.body),
                _ => None,
            })
            .collect()
    }
}

/// Scénario 2 : Alice et Bob sont hors de portée l'un de l'autre, tous deux
/// près du relais. Le message passe par le relais, et l'accusé revient par
/// le même chemin.
#[test]
fn message_traverse_le_relais_et_l_accuse_revient() {
    let mut r = Reseau::nouveau();
    r.lier('a');
    r.lier('b');

    let msg = r.envoyer('a', "salut Bob, via le relais");

    assert_eq!(r.recus('b'), vec![String::from("salut Bob, via le relais")]);
    assert_eq!(r.statut('a', msg), Some(MessageStatus::Delivered));
    let stats = r.relais.stats();
    assert!(
        stats.envelopes_stored >= 2,
        "message + accusé déposés : {stats:?}"
    );
    assert!(
        stats.envelopes_handed_off >= 2,
        "message + accusé remis : {stats:?}"
    );
    assert_eq!(stats.unauthentic, 0, "aucune trame rejetée : {stats:?}");
}

/// Scénario 3 : Bob est éteint quand Alice écrit ; Alice part avant qu'il
/// revienne. Le message attend sur le relais, Bob le récupère, son accusé
/// attend à son tour, et Alice le récupère à son retour.
#[test]
fn destinataire_eteint_puis_expediteur_parti() {
    let mut r = Reseau::nouveau();
    r.lier('a');

    let msg = r.envoyer('a', "tu liras ça plus tard");
    assert_eq!(
        r.statut('a', msg),
        Some(MessageStatus::InFlight),
        "« parti » : confié au relais"
    );
    assert_eq!(r.relais.stats().envelopes_stored, 1);

    r.couper('a');
    r.lier('b');
    assert_eq!(r.recus('b'), vec![String::from("tu liras ça plus tard")]);

    r.couper('b');
    r.lier('a');
    assert_eq!(r.statut('a', msg), Some(MessageStatus::Delivered));
}

/// Un message écrit alors qu'aucun relais n'est lié part au relais dès que
/// l'expéditeur s'en approche (rejeu de l'outbox).
#[test]
fn message_ecrit_hors_de_tout_lien_part_a_la_liaison() {
    let mut r = Reseau::nouveau();
    let msg = r.envoyer('a', "écrit dans le métro");
    assert_eq!(r.statut('a', msg), None, "reste « en attente »");

    r.lier('a');
    assert_eq!(r.statut('a', msg), Some(MessageStatus::InFlight));
    r.lier('b');
    assert_eq!(r.recus('b'), vec![String::from("écrit dans le métro")]);
    assert_eq!(r.statut('a', msg), Some(MessageStatus::Delivered));
}

/// Un téléphone n'ouvre pas de session avec un relais : aucune trame
/// `NOISE_*` non signée, que le relais journaliserait `bad_sig`.
#[test]
fn liaison_au_relais_sans_handshake() {
    let mut r = Reseau::nouveau();
    r.lier('a');
    r.lier('b');
    assert_eq!(r.relais.stats().unauthentic, 0);
}

/// Le relais ne réassemble pas au-delà d'une trame (514 octets) : un texte
/// qui fait passer l'enveloppe au palier de padding 512 ne lui est pas
/// confié et reste « en attente » (écart US-312). Un texte court (palier
/// 256, ≈ 456 octets d'enveloppe) passe.
#[test]
fn texte_trop_long_pour_une_trame_du_relais_reste_en_attente() {
    let mut r = Reseau::nouveau();
    r.lier('a');
    let long = "x".repeat(300);
    let msg = r.envoyer('a', &long);
    assert_eq!(r.statut('a', msg), None);
    assert_eq!(r.relais.stats().envelopes_stored, 0);

    let court = "y".repeat(200);
    let msg = r.envoyer('a', &court);
    assert_eq!(r.statut('a', msg), Some(MessageStatus::InFlight));
    assert_eq!(r.relais.stats().envelopes_stored, 1);
}
