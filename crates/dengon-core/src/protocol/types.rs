//! Types du format de trame L3 (`docs/synthese/05-protocole-et-trame.md` §3-4).
//!
//! Aucune (dé)sérialisation ici : c'est `protocol::codec` (US-201). Ces types
//! décrivent la **forme** d'un paquet décodé, pour que `sync::*` s'écrive
//! contre eux.

use super::consts::{
    HEADER_LEN_ADDRESSED, HEADER_LEN_BROADCAST, MSG_ID_LEN, PEER_ID_LEN, SIGNATURE_LEN,
};

/// Identifiant pseudonyme d'un nœud : `SHA-256(pub_static)[0..8]` (A-8).
pub type PeerId = [u8; PEER_ID_LEN];

/// Identifiant de contenu d'un paquet :
/// `SHA-256(sender_id ‖ timestamp_ms ‖ type ‖ payload)` (A-9). Clé du seen-set.
pub type MsgId = [u8; MSG_ID_LEN];

/// Signature Ed25519 en fin de paquet (présente si [`Flags::SIGNED`]).
pub type Signature = [u8; SIGNATURE_LEN];

/// Type de paquet L3 — octet 1 de l'en-tête. `docs/synthese/05` §4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PacketType {
    /// `0x01` — présence d'un nœud (clés publiques, pseudo, hauteur de journal).
    Announce = 0x01,
    /// `0x02` — un des 3 messages du handshake Noise `XX`.
    NoiseHs = 0x02,
    /// `0x03` — ciphertext Noise de transport ; en clair : un `AppFrame`.
    NoiseMsg = 0x03,
    /// `0x04` — enveloppe scellée Noise `X` (store-and-forward).
    SealedEnvelope = 0x04,
    /// `0x05` — accusé, dans une session Noise.
    Ack = 0x05,
    /// `0x06` — gossip GCS : filtre compact des `msgID` connus. **Cible v2**.
    GossipFilter = 0x06,
    /// `0x07` — gossip GCS : demande explicite de paquets. **Cible v2**.
    GossipPull = 0x07,
    /// `0x08` — gossip GCS : paquets bruts ré-encapsulés. **Cible v2**.
    GossipPush = 0x08,
    /// `0x09` — fragment d'un paquet L3 trop grand pour le MTU.
    Fragment = 0x09,
    /// `0x0A` — attestation de journal chaîné (racine + hauteur), diffusée.
    LogAttest = 0x0A,
    /// `0x0B` — « je porte ces enveloppes » (liste de `recipient_tag`).
    EnvelopeOffer = 0x0B,
    /// `0x0C` — « donne-les moi » (liste de `recipient_tag`).
    EnvelopeRequest = 0x0C,
    /// `0x0D` — liste des `msgID` détenus, échangée entre voisins.
    /// Remplace `GOSSIP_*` au MVP (A-13).
    Inventory = 0x0D,
}

impl PacketType {
    /// Décode l'octet `type`. `None` si la valeur n'est pas un type connu.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0x01 => Self::Announce,
            0x02 => Self::NoiseHs,
            0x03 => Self::NoiseMsg,
            0x04 => Self::SealedEnvelope,
            0x05 => Self::Ack,
            0x06 => Self::GossipFilter,
            0x07 => Self::GossipPull,
            0x08 => Self::GossipPush,
            0x09 => Self::Fragment,
            0x0A => Self::LogAttest,
            0x0B => Self::EnvelopeOffer,
            0x0C => Self::EnvelopeRequest,
            0x0D => Self::Inventory,
            _ => return None,
        })
    }

    /// L'octet `type` correspondant.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// `true` si le type est actif au MVP. Les `GOSSIP_*` (`0x06`–`0x08`) sont
    /// réservés à la cible v2 (A-13) — [`PacketType::Inventory`] les remplace.
    #[must_use]
    pub const fn is_mvp(self) -> bool {
        !matches!(
            self,
            Self::GossipFilter | Self::GossipPull | Self::GossipPush
        )
    }

    /// `true` si un paquet de ce type porte **toujours** une signature Ed25519
    /// (`docs/synthese/05` §4, colonne `SIGNED`). Les autres (`NOISE_*`, `ACK`)
    /// tiennent leur intégrité de la session Noise.
    #[must_use]
    pub const fn is_always_signed(self) -> bool {
        matches!(
            self,
            Self::Announce
                | Self::SealedEnvelope
                | Self::GossipFilter
                | Self::GossipPull
                | Self::GossipPush
                | Self::LogAttest
                | Self::EnvelopeOffer
                | Self::EnvelopeRequest
                | Self::Inventory
        )
    }

    /// `true` si un paquet de ce type est **adressé** (`recipient_id` présent,
    /// [`Flags::ADDRESSED`] posé). `docs/synthese/05` §4, colonne `ADDRESSED`.
    /// [`PacketType::Fragment`] hérite du paquet qu'il transporte.
    #[must_use]
    pub const fn is_addressed(self) -> bool {
        matches!(
            self,
            Self::NoiseHs
                | Self::NoiseMsg
                | Self::Ack
                | Self::GossipFilter
                | Self::GossipPull
                | Self::GossipPush
                | Self::EnvelopeOffer
                | Self::EnvelopeRequest
                | Self::Inventory
        )
    }
}

/// Bitfield `flags` — octet 3 de l'en-tête. `docs/synthese/05` §3.1.
///
/// Volontairement sans la crate `bitflags` : la surface est figée et minuscule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flags(u8);

impl Flags {
    /// `recipient_id` présent ; paquet destiné à un `peerID` précis.
    pub const ADDRESSED: Self = Self(1 << 0);
    /// Signature Ed25519 en fin de paquet.
    pub const SIGNED: Self = Self(1 << 1);
    /// Le payload est un fragment (l'en-tête L3 enveloppe le fragment).
    pub const FRAGMENT: Self = Self(1 << 2);
    /// L'émetteur autorise le relais (`0` = strictement local, 1 saut).
    pub const RELAY_OK: Self = Self(1 << 3);
    /// Payload complété par du PKCS#7 vers un [`super::consts::PAD_BUCKETS`].
    pub const PADDED: Self = Self(1 << 4);
    /// Bits 5-7 : réservés, `0` à l'émission.
    pub const RESERVED_MASK: u8 = 0b1110_0000;

    /// Aucun drapeau.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Depuis l'octet brut, en **ignorant** les bits réservés.
    #[must_use]
    pub const fn from_bits_truncate(bits: u8) -> Self {
        Self(bits & !Self::RESERVED_MASK)
    }

    /// L'octet brut.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// `true` si tous les bits de `other` sont posés.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Réunion de deux jeux de drapeaux.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// `true` si un bit réservé (5-7) est posé → paquet non conforme.
    #[must_use]
    pub const fn has_reserved(self) -> bool {
        self.0 & Self::RESERVED_MASK != 0
    }
}

impl core::ops::BitOr for Flags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

/// En-tête L3 **décodé** (sans le payload ni la signature).
///
/// La conversion octets ⇄ `Header` est `protocol::codec` (US-201).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Doit valoir [`super::consts::PROTO_VERSION`].
    pub version: u8,
    pub packet_type: PacketType,
    /// Sauts restants ; décrémenté à chaque relais.
    pub ttl: u8,
    pub flags: Flags,
    /// Horloge de l'émetteur, ms UTC. *Best-effort* (voir anti-rejeu, §6.4).
    pub timestamp_ms: u64,
    pub sender_id: PeerId,
    /// Présent **si et seulement si** [`Flags::ADDRESSED`].
    pub recipient_id: Option<PeerId>,
    /// Longueur du payload en octets (champ `payload_len`, big-endian).
    pub payload_len: u16,
}

impl Header {
    /// Longueur de l'en-tête sérialisé, hors payload et signature.
    #[must_use]
    pub const fn header_len(&self) -> usize {
        if self.recipient_id.is_some() {
            HEADER_LEN_ADDRESSED
        } else {
            HEADER_LEN_BROADCAST
        }
    }

    /// Longueur totale du paquet sérialisé : en-tête + payload (+ signature si
    /// [`Flags::SIGNED`]).
    #[must_use]
    pub const fn wire_len(&self) -> usize {
        let sig = if self.flags.contains(Flags::SIGNED) {
            SIGNATURE_LEN
        } else {
            0
        };
        self.header_len() + self.payload_len as usize + sig
    }

    /// Cohérence interne : la présence de `recipient_id` doit refléter
    /// [`Flags::ADDRESSED`], et aucun bit réservé ne doit être posé.
    #[must_use]
    pub const fn flags_are_consistent(&self) -> bool {
        !self.flags.has_reserved()
            && self.recipient_id.is_some() == self.flags.contains(Flags::ADDRESSED)
    }
}

/// Type d'une frame applicative (L4, à l'intérieur de Noise). `synthese/05` §4.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AppFrameKind {
    /// `1` — `msg_uuid(16) ‖ conv_seq(8) ‖ sent_ms(8) ‖ text_utf8`.
    Message = 1,
    /// `2` — un `AckFrame`.
    Ack = 2,
    /// `3` — accusé de lecture. **Réservé v2** (statut « Lu »).
    ReadReceipt = 3,
    /// `4` — profil (pseudo, avatar…). **Post-MVP**.
    Profile = 4,
}

impl AppFrameKind {
    /// Décode l'octet `kind`. `None` si inconnu.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            1 => Self::Message,
            2 => Self::Ack,
            3 => Self::ReadReceipt,
            4 => Self::Profile,
            _ => return None,
        })
    }

    /// L'octet `kind`.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// `true` si actif au MVP (`ReadReceipt` = v2, `Profile` = post-MVP).
    #[must_use]
    pub const fn is_mvp(self) -> bool {
        matches!(self, Self::Message | Self::Ack)
    }
}

/// Statut porté par un `AckFrame`. `docs/synthese/05` §4.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AckStatus {
    /// `2` — reçu par l'appareil destinataire.
    Delivered = 2,
    /// `3` — lu par l'utilisateur. **Réservé v2**.
    Read = 3,
}

impl AckStatus {
    /// Décode l'octet `status`. `None` si inconnu.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            2 => Self::Delivered,
            3 => Self::Read,
            _ => return None,
        })
    }

    /// L'octet `status`.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }
}

#[cfg(test)]
mod tests {
    use super::super::consts::{
        HEADER_LEN_ADDRESSED, HEADER_LEN_BROADCAST, PEER_ID_LEN, PROTO_VERSION, SIGNATURE_LEN,
        TTL_DEFAULT,
    };
    use super::*;

    const ALL_TYPES: [PacketType; 13] = [
        PacketType::Announce,
        PacketType::NoiseHs,
        PacketType::NoiseMsg,
        PacketType::SealedEnvelope,
        PacketType::Ack,
        PacketType::GossipFilter,
        PacketType::GossipPull,
        PacketType::GossipPush,
        PacketType::Fragment,
        PacketType::LogAttest,
        PacketType::EnvelopeOffer,
        PacketType::EnvelopeRequest,
        PacketType::Inventory,
    ];

    #[test]
    fn discriminants_contigus_0x01_a_0x0d() {
        for (i, t) in ALL_TYPES.iter().enumerate() {
            assert_eq!(t.to_u8(), (i as u8) + 1);
        }
        assert_eq!(PacketType::Inventory.to_u8(), 0x0D);
    }

    #[test]
    fn from_u8_est_l_inverse_de_to_u8() {
        for t in ALL_TYPES {
            assert_eq!(PacketType::from_u8(t.to_u8()), Some(t));
        }
        assert_eq!(PacketType::from_u8(0x00), None);
        assert_eq!(PacketType::from_u8(0x0E), None);
        assert_eq!(PacketType::from_u8(0xFF), None);
    }

    #[test]
    fn perimetre_mvp() {
        let v2: [PacketType; 3] = [
            PacketType::GossipFilter,
            PacketType::GossipPull,
            PacketType::GossipPush,
        ];
        for t in ALL_TYPES {
            assert_eq!(t.is_mvp(), !v2.contains(&t), "{t:?}");
        }
    }

    #[test]
    fn flags_bits() {
        assert_eq!(Flags::ADDRESSED.bits(), 0b0000_0001);
        assert_eq!(Flags::SIGNED.bits(), 0b0000_0010);
        assert_eq!(Flags::FRAGMENT.bits(), 0b0000_0100);
        assert_eq!(Flags::RELAY_OK.bits(), 0b0000_1000);
        assert_eq!(Flags::PADDED.bits(), 0b0001_0000);
        assert_eq!(Flags::RESERVED_MASK, 0b1110_0000);
    }

    #[test]
    fn flags_operations() {
        let f = Flags::ADDRESSED | Flags::SIGNED;
        assert!(f.contains(Flags::ADDRESSED));
        assert!(f.contains(Flags::SIGNED));
        assert!(!f.contains(Flags::FRAGMENT));
        assert!(!f.has_reserved());

        assert_eq!(
            Flags::from_bits_truncate(0b1110_0011),
            Flags::ADDRESSED | Flags::SIGNED
        );
        assert!(Flags(0b0010_0000).has_reserved());
    }

    #[test]
    fn header_len_selon_addressed() {
        let base = Header {
            version: PROTO_VERSION,
            packet_type: PacketType::NoiseMsg,
            ttl: TTL_DEFAULT,
            flags: Flags::ADDRESSED | Flags::RELAY_OK,
            timestamp_ms: 1,
            sender_id: [0; PEER_ID_LEN],
            recipient_id: Some([9; PEER_ID_LEN]),
            payload_len: 10,
        };
        assert_eq!(base.header_len(), HEADER_LEN_ADDRESSED);
        assert_eq!(base.wire_len(), HEADER_LEN_ADDRESSED + 10);
        assert!(base.flags_are_consistent());

        let broadcast = Header {
            packet_type: PacketType::Announce,
            flags: Flags::SIGNED,
            recipient_id: None,
            payload_len: 5,
            ..base
        };
        assert_eq!(broadcast.header_len(), HEADER_LEN_BROADCAST);
        assert_eq!(
            broadcast.wire_len(),
            HEADER_LEN_BROADCAST + 5 + SIGNATURE_LEN
        );
        assert!(broadcast.flags_are_consistent());
    }

    #[test]
    fn header_flags_incoherents_detectes() {
        let bad = Header {
            version: PROTO_VERSION,
            packet_type: PacketType::NoiseMsg,
            ttl: 7,
            flags: Flags::empty(), // ADDRESSED absent…
            timestamp_ms: 1,
            sender_id: [0; PEER_ID_LEN],
            recipient_id: Some([1; PEER_ID_LEN]), // …mais recipient_id présent
            payload_len: 0,
        };
        assert!(!bad.flags_are_consistent());
    }

    #[test]
    fn app_frame_kind_et_ack_status() {
        assert_eq!(AppFrameKind::from_u8(1), Some(AppFrameKind::Message));
        assert_eq!(AppFrameKind::from_u8(0), None);
        assert!(AppFrameKind::Message.is_mvp());
        assert!(!AppFrameKind::ReadReceipt.is_mvp());

        assert_eq!(AckStatus::from_u8(2), Some(AckStatus::Delivered));
        assert_eq!(AckStatus::from_u8(0), None);
        assert_eq!(AckStatus::from_u8(1), None);
        assert_eq!(AckStatus::Delivered.to_u8(), 2);
    }
}
