//! [`NoeudClient`] — le **vrai** `dengon-core::api::Node`, câblé en
//! [`Comportement`] du simulateur (US-304).
//!
//! Remplace [`crate::harness::Inondation`] (un bouchon de flood générique)
//! pour les scénarios qui doivent démontrer le **vrai** protocole : Noise
//! `XX`, statuts de message, outbox rejouée à la reconnexion — pas une
//! approximation.
//!
//! # Découverte de pair : `ANNOUNCE`, pas une triche du simulateur
//!
//! `api::Node` ne reçoit qu'un `PeerId` explicite en entrée
//! (`on_peer_connected`, `on_bytes_received`) — contrairement à
//! [`crate::harness::Inondation`], il ne connaît la couche transport que par
//! ce que l'appelant lui donne. Le [`SimTransport`] ne remonte, lui, qu'un
//! [`LinkId`] opaque (même limite que le vrai Bluetooth — voir
//! `crates/dengon-ble/src/transport.rs`). Ce module fait exactement ce que
//! `relay.rs` et `dengon-node::session.rs` font déjà : envoie son propre
//! `ANNOUNCE` ([`Node::announce_packet`]) à l'ouverture d'un lien, et
//! attend le même paquet du voisin pour associer ce lien à un `PeerId`
//! ([`api::parse_announce`]) avant d'appeler `on_peer_connected`. Aucune
//! correspondance lien → pair n'est donnée d'avance par le harness.
//!
//! # Adressage d'un envoi : convention locale, pas une extension du format
//! # de scénario
//!
//! [`Comportement::emettre`] ne porte qu'une charge opaque (`&[u8]`) — les
//! scénarios `.ron` existants (`direct`, `multihop`, `partition_merge`,
//! `lossy_mesh`) restent inchangés, ils continuent de diffuser via
//! [`crate::harness::Inondation`]. Les scénarios qui pilotent un
//! [`NoeudClient`] sont écrits en Rust (`tests/scenarios_reel.rs`), pas en
//! RON : [`NoeudClient::emettre`] attend les 8 premiers octets de la charge
//! comme le `PeerId` destinataire (big-endian, voir [`PeerId`]), le reste
//! comme le texte UTF-8 du message — encodé par [`charge_adressee`].

use std::collections::BTreeMap;

use dengon_ble::{LinkId, Transport, TransportEvent};
use dengon_core::api::{self, Node, NodeEvent};
use dengon_core::identity::{Identity, PublicIdentity};
use dengon_core::protocol::PeerId;
use dengon_core::sync::routing::Now as RoutingNow;
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};

use crate::harness::{Comportement, Contexte};

/// Encode une charge adressée pour [`NoeudClient::emettre`] : `dest`
/// (8 octets) suivi du texte UTF-8. Réciproque décodée dans `emettre`.
#[must_use]
pub fn charge_adressee(dest: PeerId, texte: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + texte.len());
    out.extend_from_slice(&dest);
    out.extend_from_slice(texte.as_bytes());
    out
}

/// État d'un lien radio local, du point de vue de [`NoeudClient`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EtatLien {
    /// Connecté, `ANNOUNCE` du voisin pas encore reçu.
    EnAttente,
    /// `ANNOUNCE` vérifié : `on_peer_connected` a été appelé pour ce pair.
    Associe(PeerId),
}

/// Le vrai nœud `dengon-core::api::Node`, piloté par le harness.
#[derive(Debug)]
pub struct NoeudClient {
    node: Node,
    /// Source d'aléa **déterministe** : chaque appel qui a besoin d'une RNG
    /// (poignée de main Noise, scellement d'enveloppe, UUID de message) en
    /// dérive une nouvelle instance jetable — jamais `OsRng`, la
    /// reproductibilité à graine fixe est le point même du simulateur (voir
    /// `SEED_PAR_DEFAUT`).
    rng_source: ChaCha20Rng,
    liens: BTreeMap<LinkId, EtatLien>,
}

impl NoeudClient {
    /// Un nœud client dont l'identité est dérivée de `graine_identite`
    /// (via [`Identity::generate`]) et le routage de `graine_routage`.
    /// `graine_rng` alimente les poignées de main/UUID ultérieurs — une
    /// troisième graine, distincte des deux premières, pour que changer
    /// l'une ne fasse pas dériver silencieusement les deux autres.
    ///
    /// # Panics
    ///
    /// Si `pseudo` est invalide pour [`Identity::generate`] (vide ou trop
    /// long) — une erreur de programmation du scénario, pas une entrée
    /// utilisateur à récupérer proprement.
    #[must_use]
    pub fn new(pseudo: &str, graine_identite: u64, graine_routage: u64, graine_rng: u64) -> Self {
        let identity = Identity::generate(pseudo, ChaCha20Rng::seed_from_u64(graine_identite))
            .unwrap_or_else(|e| unreachable!("pseudo de scénario invalide ({pseudo:?}) : {e}"));
        Self {
            node: Node::new(identity, graine_routage),
            rng_source: ChaCha20Rng::seed_from_u64(graine_rng),
            liens: BTreeMap::new(),
        }
    }

    /// `PeerId` de ce nœud (à passer à [`Node::add_contact`] d'un autre
    /// [`NoeudClient`] pour un appairage préalable — hors bande, comme le
    /// QR code réel : voir la doc de module d'`api.rs`, « Ce que cette
    /// façade N'est PAS »).
    #[must_use]
    pub fn peer_id(&self) -> PeerId {
        self.node.peer_id()
    }

    /// Identité publique à partager (équivalent du QR code).
    #[must_use]
    pub fn public_identity(&self) -> PublicIdentity {
        self.node.public_identity()
    }

    /// Enregistre `contact` comme correspondant connu — condition
    /// nécessaire à [`Node::send_message`] tant qu'aucune session n'est
    /// établie (voir sa doc).
    pub fn add_contact(&mut self, contact: PublicIdentity) {
        self.node.add_contact(contact);
    }

    /// Ce que l'application locale a reçu, dans l'ordre (miroir de
    /// [`Simulation::livres`](crate::harness::Simulation::livres), mais
    /// interrogeable sans dépendre du harness — utile pour vérifier un
    /// **statut**, pas seulement une livraison).
    #[must_use]
    pub fn node(&self) -> &Node {
        &self.node
    }

    fn rng(&mut self) -> ChaCha20Rng {
        ChaCha20Rng::seed_from_u64(self.rng_source.next_u64())
    }

    fn maintenant(ctx: &Contexte<'_>) -> RoutingNow {
        let t = ctx.maintenant_ms();
        RoutingNow::new(t, t)
    }

    /// Vide `Node::take_outgoing` vers le lien associé à chaque
    /// destinataire. Un paquet dont le lien n'est plus ouvert (pair
    /// déconnecté entre-temps) reste dans l'outbox de `Node` — rejoué de
    /// lui-même à la prochaine reconnexion, comportement déjà du ressort de
    /// `Node`, rien à refaire ici.
    fn vider_sortie(&mut self, ctx: &mut Contexte<'_>) {
        for (dest, bytes) in self.node.take_outgoing() {
            if let Some(lien) = self.liens.iter().find_map(|(l, e)| match e {
                EtatLien::Associe(p) if *p == dest => Some(*l),
                _ => None,
            }) {
                let _ = ctx.transport().send(lien, &bytes);
            }
        }
    }
}

impl Comportement for NoeudClient {
    fn pas(&mut self, ctx: &mut Contexte<'_>) {
        let maintenant = Self::maintenant(ctx);
        for ev in ctx.transport().poll() {
            match ev {
                TransportEvent::PeerConnected { peer_link_id, .. } => {
                    self.liens.insert(peer_link_id, EtatLien::EnAttente);
                    if let Ok(announce) = self.node.announce_packet(maintenant) {
                        let _ = ctx.transport().send(peer_link_id, &announce);
                    }
                }
                TransportEvent::PeerDisconnected { peer_link_id, .. } => {
                    if let Some(EtatLien::Associe(peer_id)) = self.liens.remove(&peer_link_id) {
                        self.node.on_peer_disconnected(peer_id);
                    }
                }
                TransportEvent::FrameReceived {
                    peer_link_id,
                    bytes,
                } => match self.liens.get(&peer_link_id).copied() {
                    Some(EtatLien::Associe(peer_id)) => {
                        self.node.on_bytes_received(peer_id, &bytes, maintenant);
                    }
                    Some(EtatLien::EnAttente) => {
                        // Pas un ANNOUNCE authentique : trame perdue plutôt
                        // qu'une association hasardeuse — un vrai voisin
                        // réémettra le sien (comportement identique à
                        // `relay.rs`/`session.rs`).
                        if let Ok(identite) = api::parse_announce(&bytes) {
                            let peer_id = identite.peer_id();
                            self.liens.insert(peer_link_id, EtatLien::Associe(peer_id));
                            let rng = self.rng();
                            self.node.on_peer_connected(peer_id, maintenant, rng);
                        }
                    }
                    None => {} // lien déjà fermé : trame ignorée
                },
            }
        }

        for evenement in self.node.poll_events(maintenant) {
            if let NodeEvent::MessageReceived(message) = evenement {
                ctx.livrer(message.body.as_bytes());
            }
        }

        self.vider_sortie(ctx);
    }

    /// `charge` : voir [`charge_adressee`]. Une charge trop courte (< 8
    /// octets) ou dont la partie texte n'est pas UTF-8 est silencieusement
    /// ignorée — erreur de construction du scénario, pas un cas à gérer en
    /// production (cette convention n'existe que côté `dengon-sim`).
    fn emettre(&mut self, charge: &[u8], ctx: &mut Contexte<'_>) {
        let Some((dest, texte)) = charge.split_at_checked(8) else {
            return;
        };
        let mut dest_peer_id = [0u8; 8];
        dest_peer_id.copy_from_slice(dest);
        let Ok(texte) = core::str::from_utf8(texte) else {
            return;
        };
        let maintenant = Self::maintenant(ctx);
        let rng = self.rng();
        let _ = self.node.send_message(dest_peer_id, texte, maintenant, rng);
        self.vider_sortie(ctx);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn charge_adressee_porte_le_destinataire_et_le_texte_intacts() {
        let dest: PeerId = [1, 2, 3, 4, 5, 6, 7, 8];
        let c = charge_adressee(dest, "bonjour");
        assert_eq!(&c[..8], &dest);
        assert_eq!(&c[8..], b"bonjour");
    }

    #[test]
    fn deux_noeuds_ont_des_identites_distinctes_a_graines_distinctes() {
        let a = NoeudClient::new("alice", 1, 10, 100);
        let b = NoeudClient::new("alice", 2, 10, 100);
        assert_ne!(
            a.peer_id(),
            b.peer_id(),
            "deux graines d'identité différentes doivent produire des peerID différents"
        );
    }

    #[test]
    fn meme_graine_meme_identite() {
        let a = NoeudClient::new("alice", 7, 10, 100);
        let b = NoeudClient::new("alice", 7, 99, 999);
        assert_eq!(
            a.peer_id(),
            b.peer_id(),
            "la graine d'identité seule détermine le peerID, pas les graines de routage/RNG"
        );
    }
}
