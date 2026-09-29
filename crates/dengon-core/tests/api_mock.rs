//! `api::Node` de bout en bout contre `MockTransport` (US-301).
//!
//! Deux nœuds (Alice, Bob), chacun avec son propre `MockTransport`. Le
//! « fil » entre les deux est recopié à la main : les octets qu'un nœud
//! remet à [`Node::take_outgoing`] sont donnés à `Transport::send` sur son
//! propre transport (qui se contente de les mémoriser, voir
//! `dengon_ble::MockTransport`), puis le test copie ces octets vers le
//! transport de l'autre via `injecter_trame` — même principe que
//! `tests/routing_mock.rs` (US-209), réduit à deux nœuds.
//!
//! Le `peerID` de l'autre bout est connu du harness **avant** la connexion
//! (comme dans la vraie architecture : le `manufacturer data` de l'annonce
//! BLE porte déjà un préfixe de `peerID` — voir
//! `docs/suivi/03-ecarts-conception.md`) : `on_peer_connected` reçoit donc
//! directement le bon `PeerId`, jamais un `LinkId` à résoudre après coup.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use dengon_ble::{LinkId, MockTransport, Transport, TransportConfig, TransportEvent};
use dengon_core::api::{DengonError, MessageStatus, Node, NodeEvent};
use dengon_core::identity::Identity;
use dengon_core::sync::routing::Now;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

const T0: u64 = 1_800_000_000_000;

fn rng(seed: u64) -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(seed)
}

/// Un nœud applicatif + son transport mock + le lien vers l'unique pair.
///
/// Réduit à un seul lien (pas de `BTreeMap<LinkId, PeerId>`) : ce harness ne
/// sert qu'à des tests à deux nœuds, un vrai réseau à N nœuds appartient à
/// `dengon-sim` (US-221).
struct Participant {
    node: Node,
    transport: MockTransport,
    peer_id: [u8; 8],
    autre_peer_id: [u8; 8],
    lien: LinkId,
    /// Index dans `trames_envoyees_a(lien)` déjà recopié vers l'autre bout —
    /// `MockTransport` ne vide pas cette liste toute seule (elle n'est là que
    /// pour l'observation), il faut donc un curseur pour ne pas rejouer deux
    /// fois les mêmes octets à chaque tick.
    deja_transmis: usize,
    seed: u64,
}

impl Participant {
    fn new(pseudo: &str, seed: u64) -> Self {
        let identity = Identity::generate(pseudo, rng(seed)).expect("pseudo valide");
        let peer_id = identity.peer_id();
        let mut transport = MockTransport::new();
        transport
            .start(TransportConfig {
                local_peer_id: peer_id,
                max_connections: 4,
                ..TransportConfig::default()
            })
            .expect("MockTransport démarre toujours");
        Self {
            node: Node::new(identity, seed),
            transport,
            peer_id,
            autre_peer_id: [0; 8],
            lien: LinkId::new(0),
            deja_transmis: 0,
            seed,
        }
    }

    /// Traite les événements du transport (déconnexions, octets reçus) —
    /// `PeerConnected` de `MockTransport` est ignoré ici : il ne porte que le
    /// `LinkId`, jamais le `PeerId` distant, et la connexion applicative
    /// (`on_peer_connected`) est déclenchée explicitement par
    /// `connecter_paire` avec le `PeerId` déjà connu du harness.
    fn absorber_transport(&mut self, now_ms: u64) {
        let now = Now::new(now_ms, now_ms);
        for ev in self.transport.poll() {
            match ev {
                TransportEvent::PeerConnected { .. } => {}
                TransportEvent::PeerDisconnected { .. } => {
                    self.node.on_peer_disconnected(self.autre_peer_id);
                }
                TransportEvent::FrameReceived { bytes, .. } => {
                    self.node.on_bytes_received(self.autre_peer_id, &bytes, now);
                }
            }
        }
    }

    /// Vide `Node::take_outgoing` vers le transport mock.
    fn pomper_sortie(&mut self) {
        for (dest, bytes) in self.node.take_outgoing() {
            debug_assert_eq!(dest, self.autre_peer_id, "un seul pair dans ce harness");
            let _ = self.transport.send(self.lien, &bytes);
        }
    }

    /// Recopie les octets envoyés depuis le dernier appel vers le transport
    /// de l'autre nœud, puis lui fait absorber ce qu'il vient de recevoir.
    fn transmettre_a(&mut self, autre: &mut Participant, now_ms: u64) {
        let neuves = &self.transport.trames_envoyees_a(self.lien)[self.deja_transmis..];
        for bytes in neuves {
            autre.transport.injecter_trame(autre.lien, bytes);
        }
        self.deja_transmis = self.transport.trames_envoyees_a(self.lien).len();
        autre.absorber_transport(now_ms);
    }
}

/// Connecte `a` et `b` (transports puis nœuds applicatifs via
/// `on_peer_connected`), et fait tourner la boucle événements/sortie des deux
/// côtés jusqu'à `max_ticks` — largement suffisant pour absorber un
/// handshake Noise `XX` (3 messages).
fn connecter_et_stabiliser(
    a: &mut Participant,
    b: &mut Participant,
    now_ms: &mut u64,
    max_ticks: u32,
) {
    a.lien = a.transport.connecter_pair(None);
    b.lien = b.transport.connecter_pair(None);
    a.autre_peer_id = b.peer_id;
    b.autre_peer_id = a.peer_id;
    let _ = a.transport.poll();
    let _ = b.transport.poll();

    a.node.on_peer_connected(
        b.peer_id,
        Now::new(*now_ms, *now_ms),
        rng(a.seed * 1000 + 1),
    );
    b.node.on_peer_connected(
        a.peer_id,
        Now::new(*now_ms, *now_ms),
        rng(b.seed * 1000 + 1),
    );

    for _ in 0..max_ticks {
        *now_ms += 5;
        a.pomper_sortie();
        b.pomper_sortie();
        a.transmettre_a(b, *now_ms);
        b.transmettre_a(a, *now_ms);
    }
}

#[test]
fn message_transite_de_bout_en_bout_en_session() {
    let mut alice = Participant::new("alice", 1);
    let mut bob = Participant::new("bob", 2);
    let mut now = T0;

    // Se connaître avant de se connecter (échange de carte de contact — QR,
    // US-215 — hors périmètre de ce test) : sans ça, `send_message`
    // refuserait un `dest_peer_id` inconnu (`DengonError::UnknownPeer`).
    alice.node.add_contact(bob.node.public_identity());
    bob.node.add_contact(alice.node.public_identity());

    connecter_et_stabiliser(&mut alice, &mut bob, &mut now, 6);

    let events_alice = alice.node.poll_events(Now::new(now, now));
    let events_bob = bob.node.poll_events(Now::new(now, now));
    assert!(events_alice.contains(&NodeEvent::PeerConnected(bob.peer_id)));
    assert!(events_bob.contains(&NodeEvent::PeerConnected(alice.peer_id)));

    let msg_uuid = alice
        .node
        .send_message(bob.peer_id, "salut Bob", Now::new(now, now), rng(99))
        .expect("bob est un contact connu et connecté");

    // Un seul tick suffit une fois la session établie : chiffrer/déchiffrer
    // ne prend pas plusieurs allers-retours contrairement au handshake.
    now += 5;
    alice.pomper_sortie();
    alice.transmettre_a(&mut bob, now);

    let events_bob = bob.node.poll_events(Now::new(now, now));
    let received = events_bob.iter().find_map(|ev| match ev {
        NodeEvent::MessageReceived(m) if m.msg_uuid == msg_uuid => Some(m),
        _ => None,
    });
    let message = received.expect("le message chiffré en session doit arriver chez Bob");
    assert_eq!(message.body, "salut Bob");
    assert_eq!(message.author_peer_id, alice.peer_id);
    assert!(!message.outgoing);
}

#[test]
fn send_message_reussit_avec_une_session_etablie_sans_add_contact() {
    // Trouvé en revue de la PR #102 : la doc de `send_message` promet que
    // l'identité d'un pair peut être apprise automatiquement d'une session
    // établie avec lui, mais aucun chemin ne peuplait `peer.identity` à la
    // fin du handshake XX — `send_message` échouait avec `UnknownPeer`
    // malgré une session active.
    let mut alice = Participant::new("alice", 30);
    let mut bob = Participant::new("bob", 31);
    let mut now = T0;

    // Seule alice connaît bob (`add_contact`) — bob, lui, ne fait JAMAIS
    // `add_contact(alice)`, seulement la connexion + le handshake XX.
    alice.node.add_contact(bob.node.public_identity());

    connecter_et_stabiliser(&mut alice, &mut bob, &mut now, 6);

    let events_bob = bob.node.poll_events(Now::new(now, now));
    assert!(events_bob.contains(&NodeEvent::PeerConnected(alice.peer_id)));

    // Bob doit pouvoir répondre malgré tout : la session est établie.
    bob.node
        .send_message(
            alice.peer_id,
            "salut alice, moi aussi",
            Now::new(now, now),
            rng(98),
        )
        .expect("bob a une session établie avec alice, même sans add_contact");
}

#[test]
fn envoi_vers_un_pair_inconnu_est_refuse() {
    let mut alice = Participant::new("alice", 10);
    let inconnu = [0xEE; 8];
    let err = alice
        .node
        .send_message(inconnu, "personne", Now::new(T0, T0), rng(1))
        .unwrap_err();
    assert_eq!(err, DengonError::UnknownPeer);
}

#[test]
fn statut_progresse_vers_in_flight_a_l_envoi() {
    let mut alice = Participant::new("alice", 20);
    let mut bob = Participant::new("bob", 21);
    let mut now = T0;
    alice.node.add_contact(bob.node.public_identity());
    bob.node.add_contact(alice.node.public_identity());
    connecter_et_stabiliser(&mut alice, &mut bob, &mut now, 6);

    let msg_uuid = alice
        .node
        .send_message(bob.peer_id, "ping", Now::new(now, now), rng(5))
        .expect("bob connu et connecté");

    let events = alice.node.poll_events(Now::new(now, now));
    let status = events.iter().find_map(|ev| match ev {
        NodeEvent::StatusChanged(uuid, status) if *uuid == msg_uuid => Some(*status),
        _ => None,
    });
    assert_eq!(status, Some(MessageStatus::InFlight));
}

/// Scénario 1 du DoD (`synthese/10` §3.1) : en attente → parti → distribué,
/// le dernier pas porté par l'`Ack` que Bob renvoie dans la session (US-306).
#[test]
fn statut_atteint_delivered_quand_bob_accuse() {
    let mut alice = Participant::new("alice", 40);
    let mut bob = Participant::new("bob", 41);
    let mut now = T0;
    alice.node.add_contact(bob.node.public_identity());
    connecter_et_stabiliser(&mut alice, &mut bob, &mut now, 6);
    let _ = alice.node.poll_events(Now::new(now, now));

    let msg_uuid = alice
        .node
        .send_message(bob.peer_id, "ping", Now::new(now, now), rng(6))
        .expect("bob connu et connecté");

    for _ in 0..3 {
        now += 5;
        alice.pomper_sortie();
        alice.transmettre_a(&mut bob, now);
        bob.pomper_sortie();
        bob.transmettre_a(&mut alice, now);
    }

    let statuts: Vec<MessageStatus> = alice
        .node
        .poll_events(Now::new(now, now))
        .into_iter()
        .filter_map(|ev| match ev {
            NodeEvent::StatusChanged(uuid, status) if uuid == msg_uuid => Some(status),
            _ => None,
        })
        .collect();
    assert_eq!(
        statuts,
        vec![MessageStatus::InFlight, MessageStatus::Delivered]
    );
}
