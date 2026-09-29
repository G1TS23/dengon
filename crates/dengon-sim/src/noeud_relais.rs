//! [`NoeudRelais`] — le **vrai** `dengon-core::relay::Relay`, câblé en
//! [`Comportement`] du simulateur (US-304, 2/2).
//!
//! Complète [`crate::noeud_client::NoeudClient`] : celui-ci ne relaie
//! jamais (`api::Node` n'appelle jamais `Router::poll_due`, voir sa doc de
//! module), donc `multihop`/`partition_merge` — qui ont besoin d'un
//! **maillage**, pas de deux correspondants directs — utilisent des
//! [`NoeudRelais`], pas des [`NoeudClient`]. C'est exactement la même
//! distinction que dans le vrai système : téléphone (`api::Node`) contre
//! relais dédié (`relay::Relay`, celui du firmware ESP32).
//!
//! # Horloge murale : `Relay` en exige une **réaliste**
//!
//! `Relay` refuse toute décision de routage tant que son horloge murale
//! n'est pas connue (`Relay::clock`, `WALL_CLOCK_MIN_MS` — 2024-01-01) : un
//! ESP32 sans Wi-Fi n'a pas d'horloge temps réel, il l'apprend du premier
//! `ANNOUNCE` reçu dont l'horodatage dépasse ce seuil. L'horloge **virtuelle**
//! du simulateur part de zéro (`ReseauPartage::avancer`), bien en dessous —
//! sans compensation, aucun relais ne prendrait jamais de décision.
//! [`NoeudRelais::maintenant`] décale donc l'horloge murale de
//! `WALL_CLOCK_MIN_MS` (l'horloge monotone, elle, reste l'horloge virtuelle
//! du simulateur telle quelle) : équivalent à un relais qui a déjà une
//! horloge fiable dès le démarrage — le cas normal, pas celui, dégradé, que
//! l'apprentissage par `ANNOUNCE` couvre.
//!
//! # Injection d'un paquet : convention locale, comme `NoeudClient`
//!
//! `Relay` n'a pas d'application : rien à composer, rien à décrypter.
//! [`Comportement::emettre`] diffuse ici `charge` **telle quelle**, comme si
//! un tiers (un téléphone non modélisé, ou un autre relais) venait de la
//! déposer sur ce lien — voir [`paquet_diffuse`] pour construire un paquet
//! signé valide, propagé sous les mêmes règles TTL/`RELAY_OK` que n'importe
//! quel paquet réel.
//!
//! # Observer un relais : `ctx.livrer` détourné, pas une nouvelle API
//!
//! [`Simulation`](crate::harness::Simulation) ne rend aucun accès à un
//! [`Comportement`] une fois ajouté (`ajouter_noeud` en prend possession) —
//! seul `sim.livres(i)` est observable de l'extérieur. `NoeudRelais::pas`
//! y pousse deux marqueurs distincts, un par événement précis plutôt qu'un
//! booléen vague : [`MARQUEUR_RELAYE`] chaque fois que
//! `RelayStats::relayed` augmente (le paquet est **reparti** vers un
//! voisin), [`MARQUEUR_CACHE`] chaque fois qu'un paquet reçu est
//! nouvellement mis en cache d'inventaire (le paquet est **arrivé**, qu'il
//! reparte ensuite ou non — utile pour le dernier maillon d'une chaîne, qui
//! reçoit sans avoir personne à qui relayer).

use dengon_ble::{LinkId, Transport, TransportEvent};
use dengon_core::crypto::SigningKey;
use dengon_core::ledger::Anchor;
use dengon_core::protocol::codec::{self, Packet};
use dengon_core::protocol::consts::PROTO_VERSION;
use dengon_core::protocol::{Flags, Header, PacketType, PeerId};
use dengon_core::relay::{Relay, RelayConfig, RelaySecrets, RelayStats, WALL_CLOCK_MIN_MS};
use dengon_core::sync::routing::Now;
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};

use crate::harness::{Comportement, Contexte};

/// Poussé dans `sim.livres(i)` par [`NoeudRelais`] à chaque paquet
/// effectivement relayé (voir la doc de module).
pub const MARQUEUR_RELAYE: &[u8] = b"pkt.relayed";
/// Poussé dans `sim.livres(i)` par [`NoeudRelais`] à chaque paquet
/// nouvellement mis en cache d'inventaire (voir la doc de module).
pub const MARQUEUR_CACHE: &[u8] = b"pkt.cache";

/// Construit un paquet signé, diffusé (non adressé), `RELAY_OK`, propagé
/// sous les mêmes règles que n'importe quel paquet réel — à passer à
/// [`crate::harness::Simulation::emettre`] pour l'injecter au premier
/// maillon d'une chaîne de [`NoeudRelais`].
///
/// `graine_signature` fixe une paire de clés Ed25519 jetable (pas de
/// lien avec un vrai nœud du scénario — un « tiers » qui dépose ce paquet
/// et disparaît, comme un téléphone hors de portée avant/après). Prendre
/// `type_ = PacketType::SealedEnvelope` pour que [`MARQUEUR_CACHE`]
/// s'observe : seuls `SealedEnvelope`/`NoiseMsg`/`Ack` entrent dans le
/// cache d'inventaire (`sync::inventory::cacheable`) — `Announce` et
/// `LogAttest`, eux, ne sont **jamais** mis en cache, quel que soit leur
/// TTL/`RELAY_OK` (piège rencontré en écrivant ce module : un paquet
/// `LogAttest` bien relayé ne laissait aucune trace `MARQUEUR_CACHE`).
///
/// # Panics
///
/// Si `payload` dépasse `u16::MAX` octets (jamais le cas pour un scénario
/// de test).
#[must_use]
pub fn paquet_diffuse(
    graine_signature: u64,
    sender_id: PeerId,
    type_: PacketType,
    ttl: u8,
    wall_ms: u64,
    payload: Vec<u8>,
) -> Vec<u8> {
    let mut graine = [0u8; 32];
    ChaCha20Rng::seed_from_u64(graine_signature).fill_bytes(&mut graine);
    let signing_key = SigningKey::from_seed(&graine);
    let Ok(payload_len) = u16::try_from(payload.len()) else {
        unreachable!("payload de test < 64 Kio")
    };
    let mut packet = Packet {
        header: Header {
            version: PROTO_VERSION,
            packet_type: type_,
            ttl,
            flags: Flags::SIGNED | Flags::RELAY_OK,
            timestamp_ms: wall_ms,
            sender_id,
            recipient_id: None,
            payload_len,
        },
        payload,
        signature: None,
    };
    let Ok(entree_signature) = codec::signing_input(&packet) else {
        unreachable!("un en-tête construit ci-dessus est toujours signable")
    };
    packet.signature = Some(signing_key.sign(&entree_signature));
    codec::encode(&packet).unwrap_or_else(|e| unreachable!("paquet de test mal formé : {e}"))
}

/// Le vrai relais `dengon-core::relay::Relay`, piloté par le harness.
#[derive(Debug)]
pub struct NoeudRelais {
    relay: Relay<LinkId>,
}

impl NoeudRelais {
    /// Un relais d'identité dérivée de `graine_secrets`, dont le journal
    /// repart de zéro (`Anchor::GENESIS` — pas de reprise après
    /// redémarrage à modéliser ici).
    #[must_use]
    pub fn new(pseudo: &str, graine_secrets: u64, graine_routage: u64) -> Self {
        let mut source = ChaCha20Rng::seed_from_u64(graine_secrets);
        let mut dh_secret = [0u8; 32];
        source.fill_bytes(&mut dh_secret);
        let mut sign_seed = [0u8; 32];
        source.fill_bytes(&mut sign_seed);
        let secrets = RelaySecrets {
            dh_secret,
            sign_seed,
        };
        Self {
            relay: Relay::new(
                &secrets,
                Anchor::GENESIS,
                RelayConfig::new(pseudo, graine_routage),
            ),
        }
    }

    /// `PeerId` de ce relais.
    #[must_use]
    pub fn peer_id(&self) -> PeerId {
        self.relay.peer_id()
    }

    fn maintenant(ctx: &Contexte<'_>) -> Now {
        let t = ctx.maintenant_ms();
        // Voir la doc de module : décale l'horloge murale au-delà de
        // `WALL_CLOCK_MIN_MS`, l'horloge monotone reste celle du simulateur.
        Now::new(WALL_CLOCK_MIN_MS + t, t)
    }
}

impl Comportement for NoeudRelais {
    fn pas(&mut self, ctx: &mut Contexte<'_>) {
        let maintenant = Self::maintenant(ctx);
        for ev in ctx.transport().poll() {
            match ev {
                TransportEvent::PeerConnected { peer_link_id, .. } => {
                    self.relay.link_up(peer_link_id, maintenant);
                }
                TransportEvent::PeerDisconnected { peer_link_id, .. } => {
                    self.relay.link_down(peer_link_id);
                }
                TransportEvent::FrameReceived {
                    peer_link_id,
                    bytes,
                } => {
                    let cache_avant = self.relay.cache_len();
                    self.relay.on_frame(peer_link_id, &bytes, maintenant);
                    if self.relay.cache_len() > cache_avant {
                        ctx.livrer(MARQUEUR_CACHE);
                    }
                }
            }
        }

        let RelayStats { relayed, .. } = self.relay.stats();
        self.relay.poll(maintenant);
        let nouveaux_relais = self.relay.stats().relayed.saturating_sub(relayed);
        for _ in 0..nouveaux_relais {
            ctx.livrer(MARQUEUR_RELAYE);
        }

        for (lien, bytes) in self.relay.take_outgoing() {
            let _ = ctx.transport().send(lien, &bytes);
        }
    }

    /// `charge` : un paquet complet déjà encodé (voir [`paquet_diffuse`]),
    /// diffusé tel quel sur tous les liens actuellement ouverts — ce nœud
    /// ne l'interprète pas lui-même, il joue le rôle du tiers qui vient de
    /// le déposer.
    fn emettre(&mut self, charge: &[u8], ctx: &mut Contexte<'_>) {
        let _ = ctx.transport().broadcast(charge);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn deux_relais_ont_des_identites_distinctes_a_graines_distinctes() {
        let a = NoeudRelais::new("relais-a", 1, 10);
        let b = NoeudRelais::new("relais-b", 2, 10);
        assert_ne!(a.peer_id(), b.peer_id());
    }

    #[test]
    fn meme_graine_de_secrets_meme_identite() {
        let a = NoeudRelais::new("relais-a", 7, 10);
        let b = NoeudRelais::new("relais-a", 7, 99);
        assert_eq!(
            a.peer_id(),
            b.peer_id(),
            "la graine de secrets seule détermine le peerID, pas la graine de routage"
        );
    }

    #[test]
    fn paquet_diffuse_se_decode_et_se_verifie() {
        let bytes = paquet_diffuse(
            42,
            [1; 8],
            PacketType::SealedEnvelope,
            5,
            WALL_CLOCK_MIN_MS,
            b"charge de test".to_vec(),
        );
        let paquet = codec::decode(&bytes).expect("paquet valide");
        assert_eq!(paquet.header.packet_type, PacketType::SealedEnvelope);
        assert_eq!(paquet.header.ttl, 5);
        assert_eq!(paquet.header.sender_id, [1; 8]);
        assert!(paquet
            .header
            .flags
            .contains(Flags::SIGNED | Flags::RELAY_OK));
        assert_eq!(paquet.payload, b"charge de test");
        assert!(paquet.signature.is_some());
    }

    #[test]
    fn meme_graine_de_signature_meme_paquet() {
        let a = paquet_diffuse(1, [1; 8], PacketType::SealedEnvelope, 3, 100, b"x".to_vec());
        let b = paquet_diffuse(1, [1; 8], PacketType::SealedEnvelope, 3, 100, b"x".to_vec());
        assert_eq!(
            a, b,
            "la signature Ed25519 doit être déterministe à graine fixe"
        );
    }
}
