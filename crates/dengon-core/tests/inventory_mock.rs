//! `sync::inventory` de bout en bout contre `MockTransport` (US-210).
//!
//! Chaque nœud = un `MockTransport` + un `Router<LinkId>` + un
//! `Inventory<LinkId>`, branchés comme le ferait la boucle d'événements
//! réelle :
//!
//! - à la connexion d'un voisin, envoi d'un `INVENTORY` (nos `msgID`) ;
//! - tout paquet reçu passe par le routeur ; un `INVENTORY` livré alimente
//!   la file de push, un paquet que [`cacheable`] retient entre au cache ;
//! - à chaque tour, relais échus (`poll_due`) puis pushs cadencés
//!   (`poll_push`), TTL réécrit à l'octet 2.
//!
//! Le point vérifié : les paquets **poussés** sont des paquets ordinaires
//! chez le receveur — dédup, anti-inondation et re-relais s'appliquent.
//!
//! **Décodage provisoire** : codec de test partagé dans `common/mod.rs`.

use std::collections::BTreeMap;

use dengon_ble::{LinkId, MockTransport, Transport, TransportConfig, TransportEvent};
use dengon_core::protocol::{Flags, MsgId, PacketType, PeerId, TTL_DEFAULT};
use dengon_core::sync::inventory::{
    cacheable, decode_payload, encode_payload, Inventory, InventoryConfig, PUSH_MAX_PER_MIN,
};
use dengon_core::sync::routing::{Decision, Now, Router, RoutingConfig};

mod common;
use common::{decoder, decoder_complet, paquet, paquet_type, peer, OCTET_TTL, T0};

/// Pas d'horloge de la simulation, en ms.
const PAS_MS: u64 = 100;
const MINUTE: u64 = 60_000;

fn a(t: u64) -> Now {
    Now::new(t, t)
}

// --- Un nœud : transport mock + routeur + inventaire -----------------------

struct Noeud {
    id: PeerId,
    t: MockTransport,
    r: Router<LinkId>,
    inv: Inventory<LinkId>,
    /// Octets des paquets en attente de relais, par msgID.
    a_relayer: BTreeMap<MsgId, Vec<u8>>,
    /// Paquets (hors `INVENTORY`) livrés localement.
    livres: Vec<MsgId>,
    /// `peerID` du voisin au bout de chaque lien — ce que le handshake
    /// apprendrait ; renseigné par [`Reseau::relier`].
    voisins: BTreeMap<LinkId, PeerId>,
}

impl Noeud {
    fn new(id: PeerId, icfg: InventoryConfig, seed: u64) -> Self {
        let mut t = MockTransport::new();
        let cfg = TransportConfig {
            local_peer_id: id,
            max_connections: 16,
            ..TransportConfig::default()
        };
        assert!(t.start(cfg).is_ok());
        Self {
            id,
            t,
            r: Router::new(RoutingConfig::new(id), seed),
            inv: Inventory::new(icfg),
            a_relayer: BTreeMap::new(),
            livres: Vec::new(),
            voisins: BTreeMap::new(),
        }
    }

    /// Le nœud détient déjà ce paquet (émis par lui, ou reçu avant la
    /// scène) : noté au seen-set **et** mis en cache de réconciliation.
    fn deposer(&mut self, bytes: &[u8], now: u64) {
        let Some((h, id)) = decoder(bytes) else {
            panic!("paquet déposé indécodable");
        };
        self.r.note_originated(&id, h.timestamp_ms, a(now));
        assert!(self
            .inv
            .remember(id, h.timestamp_ms, h.ttl, bytes.to_vec(), a(now)));
    }

    fn envoyer_inventaire(&mut self, lien: LinkId, now: u64) {
        let ids = self.inv.link_up(lien, a(now));
        let Some(&voisin) = self.voisins.get(&lien) else {
            return;
        };
        let Ok(payload) = encode_payload(&ids) else {
            panic!("inventaire trop grand");
        };
        // Strictement local : pas de RELAY_OK, TTL 1.
        let p = paquet_type(
            PacketType::Inventory,
            Flags::empty(),
            self.id,
            Some(voisin),
            1,
            now,
            &payload,
        );
        assert!(self.t.send(lien, &p).is_ok());
    }

    /// Une itération de la boucle d'événements du nœud.
    fn tick(&mut self, now: u64) {
        for ev in self.t.poll() {
            match ev {
                TransportEvent::PeerConnected { peer_link_id, .. } => {
                    self.r.link_up(peer_link_id);
                    if let Some(&p) = self.voisins.get(&peer_link_id) {
                        self.r.bind_peer(peer_link_id, p, now);
                    }
                    self.envoyer_inventaire(peer_link_id, now);
                }
                TransportEvent::PeerDisconnected { peer_link_id, .. } => {
                    self.r.link_down(peer_link_id);
                    self.inv.link_down(peer_link_id);
                }
                TransportEvent::FrameReceived {
                    peer_link_id,
                    bytes,
                } => self.recevoir(peer_link_id, bytes, now),
            }
        }
        for ordre in self.r.poll_due(now) {
            let Some(mut bytes) = self.a_relayer.remove(&ordre.msg_id) else {
                continue;
            };
            bytes[OCTET_TTL] = ordre.ttl;
            for cible in ordre.targets {
                assert!(self.t.send(cible, &bytes).is_ok());
            }
        }
        if self.r.pending_len() == 0 {
            self.a_relayer.clear();
        }
        for push in self.inv.poll_push(a(now)) {
            let mut bytes = push.bytes;
            bytes[OCTET_TTL] = push.ttl;
            assert!(self.t.send(push.target, &bytes).is_ok());
        }
    }

    fn recevoir(&mut self, lien: LinkId, bytes: Vec<u8>, now: u64) {
        let Some((h, id, payload)) = decoder_complet(&bytes) else {
            return;
        };
        let payload = payload.to_vec();
        let d = self.r.on_packet(lien, &h, &id, a(now));
        match d {
            Decision::RelayScheduled { .. } => {
                self.a_relayer.insert(id, bytes.clone());
            }
            Decision::Deliver if h.packet_type == PacketType::Inventory => {
                if let Ok(ids) = decode_payload(&payload) {
                    let _ = self.inv.on_inventory(lien, &ids, a(now));
                }
            }
            Decision::Deliver => self.livres.push(id),
            _ => {}
        }
        if let Some(ttl) = cacheable(&h, &d) {
            let _ = self.inv.remember(id, h.timestamp_ms, ttl, bytes, a(now));
        }
    }

    fn ids_tries(&self) -> Vec<MsgId> {
        let mut ids = self.inv.ids();
        ids.sort_unstable();
        ids
    }
}

// --- Réseau : recopie des trames entre mocks --------------------------------

struct Reseau {
    noeuds: Vec<Noeud>,
    /// (nœud, lien local) → (nœud voisin, lien chez le voisin).
    fils: BTreeMap<(usize, LinkId), (usize, LinkId)>,
    /// Trames déjà recopiées, par (nœud, lien).
    curseurs: BTreeMap<(usize, LinkId), usize>,
    now: u64,
}

impl Reseau {
    fn new(n: u8) -> Self {
        Self::avec(n, |_| InventoryConfig::new())
    }

    fn avec(n: u8, regler: impl Fn(usize) -> InventoryConfig) -> Self {
        Self {
            noeuds: (0..n)
                .map(|i| Noeud::new(peer(i + 1), regler(usize::from(i)), u64::from(i)))
                .collect(),
            fils: BTreeMap::new(),
            curseurs: BTreeMap::new(),
            now: T0,
        }
    }

    fn relier(&mut self, x: usize, y: usize) {
        let lx = self.noeuds[x].t.connecter_pair(None);
        let ly = self.noeuds[y].t.connecter_pair(None);
        self.fils.insert((x, lx), (y, ly));
        self.fils.insert((y, ly), (x, lx));
        let (px, py) = (self.noeuds[x].id, self.noeuds[y].id);
        self.noeuds[x].voisins.insert(lx, py);
        self.noeuds[y].voisins.insert(ly, px);
    }

    /// Fait tourner le réseau pendant `duree_ms`.
    fn avancer(&mut self, duree_ms: u64) {
        let fin = self.now + duree_ms;
        while self.now < fin {
            for n in &mut self.noeuds {
                n.tick(self.now);
            }
            let fils: Vec<_> = self.fils.iter().map(|(k, v)| (*k, *v)).collect();
            for ((x, lx), (y, ly)) in fils {
                let deja = self.curseurs.get(&(x, lx)).copied().unwrap_or(0);
                let trames: Vec<Vec<u8>> = self.noeuds[x].t.trames_envoyees_a(lx)[deja..].to_vec();
                self.curseurs.insert((x, lx), deja + trames.len());
                for tr in trames {
                    self.noeuds[y].t.injecter_trame(ly, &tr);
                }
            }
            self.now += PAS_MS;
        }
    }
}

/// `n` messages pour un destinataire absent (`peer(9)`), horodatés à `t`.
fn messages(n: u32, t: u64) -> Vec<Vec<u8>> {
    (0..n)
        .map(|i| {
            paquet(
                peer(1),
                Some(peer(9)),
                TTL_DEFAULT,
                t + u64::from(i),
                &i.to_be_bytes(),
            )
        })
        .collect()
}

fn id_de(bytes: &[u8]) -> MsgId {
    match decoder(bytes) {
        Some((_, id)) => id,
        None => panic!("paquet indécodable"),
    }
}

// --- Scénarios --------------------------------------------------------------

/// A détient 30 messages, B 10 dont 5 en commun.
fn scene_convergence(push_a: u16) -> Reseau {
    let mut net = Reseau::avec(2, |i| InventoryConfig {
        push_max_per_min: if i == 0 { push_a } else { PUSH_MAX_PER_MIN },
        ..InventoryConfig::new()
    });
    let msgs = messages(35, net.now);
    for m in &msgs[..30] {
        net.noeuds[0].deposer(m, T0);
    }
    for m in &msgs[25..] {
        net.noeuds[1].deposer(m, T0);
    }
    net.relier(0, 1);
    net.avancer(3 * MINUTE);
    net
}

#[test]
fn reconciliation_a_b_convergente() {
    let net = scene_convergence(PUSH_MAX_PER_MIN);
    let (na, nb) = (&net.noeuds[0], &net.noeuds[1]);

    assert_eq!(na.inv.len(), 35);
    assert_eq!(na.ids_tries(), nb.ids_tries(), "caches identiques");
    assert_eq!(
        na.inv.stats().pushes_emitted,
        25,
        "A pousse ce qui manque à B"
    );
    assert_eq!(
        nb.inv.stats().pushes_emitted,
        5,
        "B pousse ce qui manque à A"
    );
    // Le push cadencé reste sous l'anti-inondation du voisin.
    assert_eq!(na.r.stats().flood_limited, 0);
    assert_eq!(nb.r.stats().flood_limited, 0);
    // Chaque paquet poussé est passé par le routeur du receveur : 1
    // INVENTORY + 25 pushs chez B.
    assert_eq!(nb.r.stats().received, 26);
}

#[test]
fn temoin_sans_cadence_l_anti_inondation_rejette() {
    // Même scène, A pousse sans limite : B reçoit l'INVENTORY de A puis 25
    // pushs d'un bloc ; son routeur n'accepte que 20 nouveaux msgID/min.
    let net = scene_convergence(u16::MAX);
    let nb = &net.noeuds[1];
    assert_eq!(nb.r.stats().flood_limited, 6);
    assert_eq!(
        nb.inv.len(),
        10 + 19,
        "6 paquets perdus pour cette rencontre"
    );
}

#[test]
fn un_push_deja_vu_est_rejete_comme_doublon() {
    // A n'annonce que son msgID le plus récent (inventaire tronqué) : B lui
    // pousse le plus ancien, que A a pourtant — son routeur le rejette.
    let mut net = Reseau::avec(2, |i| InventoryConfig {
        max_ids: if i == 0 { 1 } else { 120 },
        ..InventoryConfig::new()
    });
    let msgs = messages(2, net.now);
    for n in 0..2 {
        for m in &msgs {
            net.noeuds[n].deposer(m, T0);
        }
    }
    net.relier(0, 1);
    net.avancer(MINUTE);

    let (na, nb) = (&net.noeuds[0], &net.noeuds[1]);
    assert_eq!(nb.inv.stats().pushes_emitted, 1);
    assert_eq!(na.r.stats().duplicates, 1);
    assert_eq!(na.r.stats().relays_scheduled, 0);
    assert_eq!(na.inv.len(), 2);
}

#[test]
fn un_push_frais_est_relaye_plus_loin() {
    // B — C déjà reliés ; A arrive avec un message pour C.
    let mut net = Reseau::new(3);
    net.relier(1, 2);
    net.avancer(1_000);
    let m = paquet(peer(1), Some(peer(3)), TTL_DEFAULT, net.now, b"pour C");
    let now = net.now;
    net.noeuds[0].deposer(&m, now);
    net.relier(0, 1);
    net.avancer(10_000);

    let id = id_de(&m);
    assert_eq!(net.noeuds[2].livres, vec![id], "C reçoit via le push de A");
    assert_eq!(net.noeuds[1].r.stats().relays_emitted, 1);
    // B le garde pour ses prochaines rencontres, un saut de moins.
    assert!(net.noeuds[1].inv.contains(&id));
    assert!(!net.noeuds[2].inv.contains(&id), "livré : pas à porter");
}

#[test]
fn un_message_acquitte_n_est_ni_annonce_ni_pousse() {
    let mut net = Reseau::new(2);
    let msgs = messages(2, net.now);
    for m in &msgs {
        net.noeuds[0].deposer(m, T0);
    }
    assert!(net.noeuds[0].inv.forget(&id_de(&msgs[0])));
    net.relier(0, 1);
    net.avancer(MINUTE);

    let nb = &net.noeuds[1];
    assert!(!nb.inv.contains(&id_de(&msgs[0])));
    assert!(nb.inv.contains(&id_de(&msgs[1])));
    assert_eq!(net.noeuds[0].inv.stats().pushes_emitted, 1);
}

#[test]
fn deconnexion_puis_retour_reprend_la_reconciliation() {
    // 30 paquets à pousser : la connexion tombe après la première minute
    // (15 poussés), la reprise envoie un nouvel inventaire et finit le travail.
    let mut net = Reseau::new(2);
    for m in &messages(30, net.now) {
        net.noeuds[0].deposer(m, T0);
    }
    net.relier(0, 1);
    net.avancer(30_000);
    assert_eq!(net.noeuds[1].inv.len(), usize::from(PUSH_MAX_PER_MIN));

    let liens: Vec<(usize, LinkId)> = net.fils.keys().copied().collect();
    for (n, l) in liens {
        net.noeuds[n].t.couper_brutalement(l);
    }
    net.fils.clear();
    net.avancer(MINUTE);
    net.relier(0, 1);
    net.avancer(2 * MINUTE);

    assert_eq!(net.noeuds[1].inv.len(), 30);
    assert_eq!(net.noeuds[1].r.stats().flood_limited, 0);
}
