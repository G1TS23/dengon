//! [`Session`] — la boucle qui relie un [`Node`] à un [`Transport`].
//!
//! `dengon-core` ne possède aucune radio (`api.rs`, « Ce que cette façade N'est
//! PAS ») : c'est à l'appelant de faire circuler les octets. Ce module est
//! cette colle, écrite **générique sur `Transport`** pour que la même boucle
//! tourne sur `btleplug` en production et sur `MockTransport` dans les tests.
//!
//! # Limite : un seul pair par session
//!
//! Le contrat `Transport` ne remonte qu'un [`LinkId`], pas un `peerID` : le
//! transport ne sait pas qui est au bout du lien tant que le handshake n'a pas
//! eu lieu, et `Node::on_peer_connected` exige déjà un `peerID`. La conception
//! n'a pas encore câblé l'`ANNOUNCE` qui lèverait l'ambiguïté (`api.rs`, la
//! portée d'US-301). Le nœud CLI est un **banc de test à deux** : il est
//! configuré avec le contact distant (`--peer`), et tout lien ouvert est
//! attribué à ce pair. Écart consigné dans `docs/suivi/03-ecarts-conception.md`.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use dengon_ble::{LinkId, Transport, TransportConfig, TransportError, TransportEvent};
use dengon_core::api::{DengonError, Node, NodeEvent};
use dengon_core::protocol::PeerId;
use dengon_core::sync::routing::Now;
use dengon_core::sync::status::MsgUuid;
use rand_core::OsRng;

/// Plafond des trames gardées pendant qu'aucun lien n'est ouvert.
const ATTENTE_MAX: usize = 64;

/// Un nœud, son transport et le pair distant.
#[derive(Debug)]
pub struct Session<T: Transport> {
    node: Node,
    transport: T,
    pair: PeerId,
    lien: Option<LinkId>,
    /// Trames produites par le nœud alors qu'aucun lien n'était ouvert.
    en_attente: Vec<Vec<u8>>,
    debut: Instant,
}

impl<T: Transport> Session<T> {
    /// Assemble la session. Le transport n'est pas encore démarré.
    pub fn new(node: Node, transport: T, pair: PeerId) -> Self {
        Self {
            node,
            transport,
            pair,
            lien: None,
            en_attente: Vec::new(),
            debut: Instant::now(),
        }
    }

    /// Le transport (bancs d'essai).
    #[cfg(test)]
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Démarre le transport en **scan seul, un lien** : c'est ce que sait faire
    /// `btleplug` (rôle central), et un seul pair est attendu.
    ///
    /// # Errors
    ///
    /// Erreur de [`Transport::start`] (adaptateur absent…).
    pub fn demarrer(&mut self) -> Result<(), TransportError> {
        self.transport.start(TransportConfig {
            local_peer_id: self.node.peer_id(),
            advertise: false,
            scan: true,
            max_connections: 1,
            ..TransportConfig::default()
        })
    }

    fn maintenant(&self) -> Now {
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        let mono = u64::try_from(self.debut.elapsed().as_millis()).unwrap_or(u64::MAX);
        Now::new(wall, mono)
    }

    /// Écrit `texte` au pair distant.
    ///
    /// # Errors
    ///
    /// [`DengonError`] de [`Node::send_message`] (contact inconnu…).
    pub fn envoyer(&mut self, texte: &str) -> Result<MsgUuid, DengonError> {
        let now = self.maintenant();
        let uuid = self.node.send_message(self.pair, texte, now, OsRng)?;
        self.vider_sortie();
        Ok(uuid)
    }

    /// Un tour de boucle : relève le transport, fait avancer le nœud, envoie ce
    /// qu'il produit. Rend les événements du nœud (messages reçus, statuts…).
    pub fn tourner(&mut self) -> Vec<NodeEvent> {
        let now = self.maintenant();
        for evenement in self.transport.poll() {
            match evenement {
                TransportEvent::PeerConnected { peer_link_id, .. } => {
                    if self.lien.is_none() {
                        self.lien = Some(peer_link_id);
                        self.node.on_peer_connected(self.pair, now, OsRng);
                    }
                }
                TransportEvent::PeerDisconnected { peer_link_id, .. } => {
                    if self.lien == Some(peer_link_id) {
                        self.lien = None;
                        self.node.on_peer_disconnected(self.pair);
                    }
                }
                TransportEvent::FrameReceived {
                    peer_link_id,
                    bytes,
                } => {
                    if self.lien == Some(peer_link_id) {
                        self.node.on_bytes_received(self.pair, &bytes, now);
                    }
                }
            }
        }
        self.vider_sortie();
        self.node.poll_events(now)
    }

    /// Envoie ce que le nœud a produit ; garde en attente s'il n'y a pas de lien.
    fn vider_sortie(&mut self) {
        for (_, octets) in self.node.take_outgoing() {
            if self.en_attente.len() < ATTENTE_MAX {
                self.en_attente.push(octets);
            }
        }
        let Some(lien) = self.lien else { return };
        for octets in std::mem::take(&mut self.en_attente) {
            if let Err(e) = self.transport.send(lien, &octets) {
                eprintln!("dengon-node : envoi impossible : {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use dengon_ble::MockTransport;
    use dengon_core::identity::Identity;

    fn noeud(pseudo: &str) -> Node {
        Node::new(Identity::generate(pseudo, OsRng).unwrap(), 1)
    }

    /// Câble deux bouchons « face à face » : ce que l'un envoie, l'autre le
    /// reçoit. Le bouchon garde tout l'historique des envois, on ne rejoue donc
    /// que ce qui est nouveau depuis le dernier passage.
    #[derive(Default)]
    struct Pont {
        vues_a: usize,
        vues_b: usize,
    }

    impl Pont {
        fn faire_passer(
            &mut self,
            a: &mut Session<MockTransport>,
            b: &mut Session<MockTransport>,
            lien: LinkId,
        ) {
            let de_a = a.transport_mut().trames_envoyees_a(lien).to_vec();
            let de_b = b.transport_mut().trames_envoyees_a(lien).to_vec();
            for trame in &de_a[self.vues_a..] {
                b.transport_mut().injecter_trame(lien, trame);
            }
            for trame in &de_b[self.vues_b..] {
                a.transport_mut().injecter_trame(lien, trame);
            }
            self.vues_a = de_a.len();
            self.vues_b = de_b.len();
        }
    }

    #[test]
    fn deux_noeuds_s_echangent_un_message_de_bout_en_bout() {
        let (na, nb) = (noeud("alice"), noeud("bob"));
        let (pa, pb) = (na.public_identity(), nb.public_identity());
        let (ida, idb) = (na.peer_id(), nb.peer_id());

        let mut alice = Session::new(na, MockTransport::new(), idb);
        let mut bob = Session::new(nb, MockTransport::new(), ida);
        alice.node.add_contact(pb);
        bob.node.add_contact(pa);
        alice.demarrer().unwrap();
        bob.demarrer().unwrap();

        // Les deux bouchons ouvrent « le même » lien (numéro 0 des deux côtés).
        let lien = alice.transport_mut().connecter_pair(Some(-50));
        let lien_b = bob.transport_mut().connecter_pair(Some(-50));
        assert_eq!(lien, lien_b);
        let _ = alice.tourner();
        let _ = bob.tourner();

        alice.envoyer("bonjour bob").unwrap();

        let mut pont = Pont::default();
        let mut recu = None;
        for _ in 0..10 {
            pont.faire_passer(&mut alice, &mut bob, lien);
            let _ = alice.tourner();
            for e in bob.tourner() {
                if let NodeEvent::MessageReceived(m) = e {
                    recu = Some(m);
                }
            }
            if recu.is_some() {
                break;
            }
        }

        let m = recu.expect("bob doit recevoir le message d'alice");
        assert_eq!(m.body, "bonjour bob");
        assert_eq!(m.author_peer_id, ida);
        assert!(!m.outgoing);
    }

    #[test]
    fn un_envoi_sans_lien_est_garde_puis_remis_a_la_connexion() {
        let (na, nb) = (noeud("alice"), noeud("bob"));
        let (pb, idb) = (nb.public_identity(), nb.peer_id());
        let mut alice = Session::new(na, MockTransport::new(), idb);
        alice.node.add_contact(pb);
        alice.demarrer().unwrap();

        // Sans lien, le nœud garde le message en outbox : il ne produit rien
        // à transmettre, et le rejouera lui-même à la connexion.
        alice.envoyer("plus tard").unwrap();
        assert!(alice.en_attente.is_empty());

        let lien = alice.transport_mut().connecter_pair(None);
        let _ = alice.tourner();
        assert!(
            !alice.transport_mut().trames_envoyees_a(lien).is_empty(),
            "le message doit partir dès que le lien s'ouvre"
        );
    }
}
