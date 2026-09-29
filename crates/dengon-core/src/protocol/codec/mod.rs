//! Sérialisation octet par octet du format de trame L3 (US-201).
//!
//! Fait foi : `docs/powl/03-network-protocol.md` §3-4, résumé dans
//! `docs/synthese/05-protocole-et-trame.md` §3-4. Les **types** décodés
//! viennent de [`super::types`] (US-108) ; ce module ne fait que la
//! conversion octets ⇄ [`Packet`].
//!
//! ```text
//!  offset  taille  champ
//!    0       1     version
//!    1       1     type
//!    2       1     ttl
//!    3       1     flags
//!    4       8     timestamp_ms          (big-endian)
//!   12       8     sender_id
//!   20      [8]    recipient_id          (SSI ADDRESSED)
//!  20|28     2     payload_len           (big-endian)
//!  22|30     N     payload
//!   +N     [64]    signature             (SSI SIGNED)
//! ```
//!
//! # Règles appliquées au décodage
//!
//! Au-delà de la forme (longueurs, version, type connu), [`decode`] applique
//! les colonnes `ADDRESSED` / `SIGNED` du tableau `synthese/05` §4 (voir
//! [`FrameRule`]). La règle `SIGNED` est une règle de **sécurité** : le
//! pipeline de réception (`synthese/05` §6.1) ne vérifie la signature que
//! « si SIGNED » ; sans ce contrôle, retirer le bit `SIGNED` d'un `ANNOUNCE`
//! forgé (et tronquer la signature) suffirait à contourner la vérification.
//!
//! Les bits réservés (5-7) sont **ignorés** à la réception
//! (`synthese/05:80`) : [`decode`] les masque via
//! [`Flags::from_bits_truncate`]. À l'émission ils doivent valoir 0 —
//! [`encode`] refuse un en-tête qui en porte.
//!
//! # Signature
//!
//! La signature Ed25519 couvre l'en-tête et le payload (octets
//! `[0 .. début_signature]`) **avec l'octet `ttl` (offset [`TTL_OFFSET`]) mis
//! à 0** : chaque relais décrémente le TTL (`synthese/05` §6.1), une signature
//! qui le couvrirait ne se vérifierait plus après un saut (retour de revue
//! #80, point de Paul — même choix que bitchat). Le TTL n'est donc **pas**
//! protégé : un relais peut le remonter, borné par la dédup du seen-set.
//!
//! - Émission : [`signing_input`] donne les octets à signer d'un [`Packet`].
//! - Réception : [`received_signing_input`] les donne à partir des octets
//!   **reçus**, pas d'un ré-encodage : un bit réservé posé par un pair plus
//!   récent est masqué par [`decode`], un ré-encodage ne serait donc plus
//!   identique à ce que l'émetteur a signé.
//!
//! Les frames applicatives chiffrées (L4) sont dans [`app`], le payload
//! `ANNOUNCE` dans [`announce`].

pub mod announce;
pub mod app;

use alloc::vec::Vec;
use core::fmt;

use super::consts::{
    HEADER_LEN_ADDRESSED, HEADER_LEN_BROADCAST, PEER_ID_LEN, PROTO_VERSION, SIGNATURE_LEN,
};
use super::types::{Flags, Header, PacketType, PeerId, Signature};

/// Taille maximale d'un payload : `payload_len` tient sur 2 octets.
pub const PAYLOAD_MAX: usize = u16::MAX as usize;

/// Position de l'octet `ttl` dans l'en-tête ; mis à 0 dans l'entrée de
/// signature (voir la section « Signature » du module).
pub const TTL_OFFSET: usize = 2;

/// Paquet L3 complet : en-tête décodé, payload opaque, signature éventuelle.
///
/// Invariants vérifiés par [`encode`] (et garantis par [`decode`]) :
/// `header.payload_len == payload.len()`, `signature.is_some()` ⇔
/// [`Flags::SIGNED`], et les règles de [`FrameRule`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub header: Header,
    /// Payload brut ; sa structure dépend de `header.packet_type`
    /// (`synthese/05` §4) et n'est pas interprétée ici.
    pub payload: Vec<u8>,
    /// Présente **si et seulement si** [`Flags::SIGNED`].
    pub signature: Option<Signature>,
}

/// Cohérence type ⇄ drapeaux imposée par `synthese/05` §4, commune à
/// l'encodage et au décodage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameRule {
    /// Le type est toujours (ou jamais) adressé, et `ADDRESSED` dit l'inverse.
    Addressing {
        packet_type: PacketType,
        /// Valeur attendue de `ADDRESSED` pour ce type.
        expected: bool,
    },
    /// Type toujours signé ([`PacketType::is_always_signed`]) sans `SIGNED`.
    MissingSignature { packet_type: PacketType },
    /// `FRAGMENT` posé sur un type autre que [`PacketType::Fragment`], ou
    /// absent d'un [`PacketType::Fragment`].
    FragmentFlag { packet_type: PacketType },
}

impl fmt::Display for FrameRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Addressing {
                packet_type,
                expected,
            } => write!(
                f,
                "{packet_type:?} doit avoir ADDRESSED = {expected} (synthese/05 §4)"
            ),
            Self::MissingSignature { packet_type } => {
                write!(f, "{packet_type:?} doit être signé (synthese/05 §4)")
            }
            Self::FragmentFlag { packet_type } => write!(
                f,
                "drapeau FRAGMENT incohérent avec le type {packet_type:?}"
            ),
        }
    }
}

impl core::error::Error for FrameRule {}

/// Vérifie les colonnes `ADDRESSED` / `SIGNED` de `synthese/05` §4 et la
/// cohérence de `FRAGMENT`.
fn check_rules(packet_type: PacketType, flags: Flags) -> Result<(), FrameRule> {
    if let Some(expected) = packet_type.is_addressed() {
        if flags.contains(Flags::ADDRESSED) != expected {
            return Err(FrameRule::Addressing {
                packet_type,
                expected,
            });
        }
    }
    if packet_type.is_always_signed() && !flags.contains(Flags::SIGNED) {
        return Err(FrameRule::MissingSignature { packet_type });
    }
    let is_fragment = matches!(packet_type, PacketType::Fragment);
    if flags.contains(Flags::FRAGMENT) != is_fragment {
        return Err(FrameRule::FragmentFlag { packet_type });
    }
    Ok(())
}

/// Raison du refus d'un paquet par [`decode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Moins d'octets que l'en-tête n'en exige (22 ou 30).
    Truncated { needed: usize, got: usize },
    /// Octet `version` différent de [`PROTO_VERSION`].
    UnsupportedVersion(u8),
    /// Octet `type` hors `0x01..=0x0D`.
    UnknownType(u8),
    /// Longueur totale ≠ en-tête + `payload_len` (+ 64 si `SIGNED`) :
    /// payload tronqué, signature absente, ou octets en trop.
    LengthMismatch { expected: usize, got: usize },
    /// Violation d'une règle type ⇄ drapeaux.
    Rule(FrameRule),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { needed, got } => {
                write!(f, "en-tête tronqué : {got} octets, {needed} attendus")
            }
            Self::UnsupportedVersion(v) => {
                write!(f, "version {v} non supportée (attendu {PROTO_VERSION})")
            }
            Self::UnknownType(t) => write!(f, "type de paquet inconnu 0x{t:02x}"),
            Self::LengthMismatch { expected, got } => {
                write!(f, "longueur {got} octets, {expected} attendus")
            }
            Self::Rule(rule) => rule.fmt(f),
        }
    }
}

impl core::error::Error for DecodeError {}

impl From<FrameRule> for DecodeError {
    fn from(rule: FrameRule) -> Self {
        Self::Rule(rule)
    }
}

/// Raison du refus d'un [`Packet`] par [`encode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// `header.version` différent de [`PROTO_VERSION`].
    UnsupportedVersion(u8),
    /// Bit réservé (5-7) posé : ils valent 0 à l'émission (`synthese/05:80`).
    ReservedFlags(u8),
    /// Payload de plus de [`PAYLOAD_MAX`] octets.
    PayloadTooLarge(usize),
    /// `header.payload_len` ne correspond pas à `payload.len()`.
    PayloadLenMismatch { header: u16, actual: usize },
    /// `recipient_id` présent sans `ADDRESSED`, ou l'inverse.
    RecipientMismatch,
    /// `signature` présente sans `SIGNED`, ou l'inverse.
    SignatureMismatch,
    /// Violation d'une règle type ⇄ drapeaux.
    Rule(FrameRule),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(v) => {
                write!(f, "version {v} non supportée (attendu {PROTO_VERSION})")
            }
            Self::ReservedFlags(bits) => {
                write!(f, "bits réservés posés à l'émission (flags 0x{bits:02x})")
            }
            Self::PayloadTooLarge(len) => {
                write!(f, "payload de {len} octets > {PAYLOAD_MAX}")
            }
            Self::PayloadLenMismatch { header, actual } => write!(
                f,
                "payload_len = {header} mais le payload fait {actual} octets"
            ),
            Self::RecipientMismatch => {
                f.write_str("recipient_id présent SSI ADDRESSED : incohérent")
            }
            Self::SignatureMismatch => f.write_str("signature présente SSI SIGNED : incohérent"),
            Self::Rule(rule) => rule.fmt(f),
        }
    }
}

impl core::error::Error for EncodeError {}

impl From<FrameRule> for EncodeError {
    fn from(rule: FrameRule) -> Self {
        Self::Rule(rule)
    }
}

/// Nombre d'octets couverts par la signature : en-tête + payload.
const fn signed_len(header: &Header) -> usize {
    header.header_len() + header.payload_len as usize
}

/// Vérifie les invariants de [`Packet`] et les règles du format, sauf la
/// présence de la signature (inconnue au moment de calculer
/// [`signing_input`]).
fn check_encodable(packet: &Packet) -> Result<(), EncodeError> {
    let h = &packet.header;
    if h.version != PROTO_VERSION {
        return Err(EncodeError::UnsupportedVersion(h.version));
    }
    if h.flags.has_reserved() {
        return Err(EncodeError::ReservedFlags(h.flags.bits()));
    }
    if packet.payload.len() > PAYLOAD_MAX {
        return Err(EncodeError::PayloadTooLarge(packet.payload.len()));
    }
    if usize::from(h.payload_len) != packet.payload.len() {
        return Err(EncodeError::PayloadLenMismatch {
            header: h.payload_len,
            actual: packet.payload.len(),
        });
    }
    if !h.flags_are_consistent() {
        return Err(EncodeError::RecipientMismatch);
    }
    check_rules(h.packet_type, h.flags)?;
    Ok(())
}

/// Écrit l'en-tête puis le payload (tout sauf la signature).
fn write_header_and_payload(packet: &Packet, out: &mut Vec<u8>) {
    let h = &packet.header;
    out.push(h.version);
    out.push(h.packet_type.to_u8());
    out.push(h.ttl);
    out.push(h.flags.bits());
    out.extend_from_slice(&h.timestamp_ms.to_be_bytes());
    out.extend_from_slice(&h.sender_id);
    if let Some(recipient) = &h.recipient_id {
        out.extend_from_slice(recipient);
    }
    out.extend_from_slice(&h.payload_len.to_be_bytes());
    out.extend_from_slice(&packet.payload);
}

/// Sérialise `packet` à la fin de `out`. En cas d'erreur, `out` n'est pas
/// modifié.
///
/// # Errors
///
/// [`EncodeError`] si le paquet viole un invariant de [`Packet`] ou une
/// règle du format.
pub fn encode_into(packet: &Packet, out: &mut Vec<u8>) -> Result<(), EncodeError> {
    check_encodable(packet)?;
    if packet.signature.is_some() != packet.header.flags.contains(Flags::SIGNED) {
        return Err(EncodeError::SignatureMismatch);
    }
    out.reserve(packet.header.wire_len());
    write_header_and_payload(packet, out);
    if let Some(signature) = &packet.signature {
        out.extend_from_slice(signature);
    }
    Ok(())
}

/// Octets à signer d'un paquet à émettre : en-tête + payload, `ttl` à 0.
///
/// Le champ `packet.signature` est ignoré (c'est ce qu'on s'apprête à
/// calculer) ; le paquet doit en revanche porter [`Flags::SIGNED`].
/// Identique à [`received_signing_input`] sur les octets encodés, quel que
/// soit le TTL au moment de la réception.
///
/// # Errors
///
/// [`EncodeError::SignatureMismatch`] si `SIGNED` est absent, sinon les mêmes
/// erreurs que [`encode`].
pub fn signing_input(packet: &Packet) -> Result<Vec<u8>, EncodeError> {
    check_encodable(packet)?;
    if !packet.header.flags.contains(Flags::SIGNED) {
        return Err(EncodeError::SignatureMismatch);
    }
    let mut out = Vec::with_capacity(signed_len(&packet.header));
    write_header_and_payload(packet, &mut out);
    out[TTL_OFFSET] = 0;
    Ok(out)
}

/// Octets sur lesquels vérifier la signature d'un paquet **reçu** : les
/// octets bruts `[0 .. début_signature]`, `ttl` à 0.
///
/// `Ok(None)` si le paquet n'est pas signé.
///
/// # Errors
///
/// Les erreurs de [`decode`] : on ne calcule rien sur un paquet invalide.
pub fn received_signing_input(raw: &[u8]) -> Result<Option<Vec<u8>>, DecodeError> {
    let packet = decode(raw)?;
    if packet.signature.is_none() {
        return Ok(None);
    }
    // `decode` a vérifié raw.len() == signed_len + SIGNATURE_LEN : le `get`
    // ne peut pas échouer, mais on évite l'indexage qui panique.
    let mut out = raw
        .get(..signed_len(&packet.header))
        .ok_or(DecodeError::LengthMismatch {
            expected: packet.header.wire_len(),
            got: raw.len(),
        })?
        .to_vec();
    out[TTL_OFFSET] = 0;
    Ok(Some(out))
}

/// Sérialise `packet` dans un nouveau tampon de [`Header::wire_len`] octets.
///
/// # Errors
///
/// Voir [`encode_into`].
pub fn encode(packet: &Packet) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::new();
    encode_into(packet, &mut out)?;
    Ok(out)
}

/// Désérialise un paquet L3 complet (après réassemblage L2 éventuel).
///
/// Ne panique sur aucune entrée : toute trame malformée ou tronquée donne une
/// [`DecodeError`]. Ne vérifie **pas** la signature (c'est `crypto`), ni le
/// TTL, l'horodatage ou le périmètre MVP du type (c'est `sync::routing`).
///
/// # Errors
///
/// [`DecodeError`] si les octets ne forment pas exactement un paquet valide.
pub fn decode(raw: &[u8]) -> Result<Packet, DecodeError> {
    let mut r = Reader::new(raw);
    let fixed = r.take_array::<4>().ok_or(DecodeError::Truncated {
        needed: HEADER_LEN_BROADCAST,
        got: raw.len(),
    })?;
    let [version, type_byte, ttl, flags_byte] = fixed;
    if version != PROTO_VERSION {
        return Err(DecodeError::UnsupportedVersion(version));
    }
    let packet_type = PacketType::from_u8(type_byte).ok_or(DecodeError::UnknownType(type_byte))?;
    let flags = Flags::from_bits_truncate(flags_byte);
    let addressed = flags.contains(Flags::ADDRESSED);
    let header_len = if addressed {
        HEADER_LEN_ADDRESSED
    } else {
        HEADER_LEN_BROADCAST
    };
    let truncated = DecodeError::Truncated {
        needed: header_len,
        got: raw.len(),
    };
    if raw.len() < header_len {
        return Err(truncated);
    }

    // Longueurs vérifiées juste au-dessus : les `take_*` ci-dessous ne peuvent
    // plus échouer, mais on garde le `?` plutôt qu'un `unwrap`.
    let timestamp_ms = u64::from_be_bytes(r.take_array::<8>().ok_or(truncated)?);
    let sender_id: PeerId = r.take_array::<PEER_ID_LEN>().ok_or(truncated)?;
    let recipient_id = if addressed {
        Some(r.take_array::<PEER_ID_LEN>().ok_or(truncated)?)
    } else {
        None
    };
    let payload_len = u16::from_be_bytes(r.take_array::<2>().ok_or(truncated)?);

    let signed = flags.contains(Flags::SIGNED);
    let expected = header_len + usize::from(payload_len) + if signed { SIGNATURE_LEN } else { 0 };
    if raw.len() != expected {
        return Err(DecodeError::LengthMismatch {
            expected,
            got: raw.len(),
        });
    }
    check_rules(packet_type, flags)?;

    let length_mismatch = DecodeError::LengthMismatch {
        expected,
        got: raw.len(),
    };
    let payload = r
        .take(usize::from(payload_len))
        .ok_or(length_mismatch)?
        .to_vec();
    let signature = if signed {
        Some(r.take_array::<SIGNATURE_LEN>().ok_or(length_mismatch)?)
    } else {
        None
    };

    Ok(Packet {
        header: Header {
            version,
            packet_type,
            ttl,
            flags,
            timestamp_ms,
            sender_id,
            recipient_id,
            payload_len,
        },
        payload,
        signature,
    })
}

/// `msgID = SHA-256(sender_id ‖ timestamp_ms ‖ type ‖ payload)` (A-9,
/// `synthese/05` §3) : jamais transmis, recalculé par chaque nœud à partir
/// des octets reçus pour dédupliquer (`sync::routing`, `sync::courier`).
#[must_use]
pub fn msg_id(packet: &Packet) -> super::types::MsgId {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(packet.header.sender_id);
    hasher.update(packet.header.timestamp_ms.to_be_bytes());
    hasher.update([packet.header.packet_type.to_u8()]);
    hasher.update(&packet.payload);
    hasher.finalize().into()
}

/// Curseur de lecture qui ne panique jamais : toute lecture hors bornes
/// renvoie `None`.
#[derive(Debug)]
pub(crate) struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { rest: bytes }
    }

    /// Les `n` octets suivants, ou `None` s'il en reste moins.
    pub(crate) fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.rest.len() < n {
            return None;
        }
        let (head, tail) = self.rest.split_at(n);
        self.rest = tail;
        Some(head)
    }

    /// Les `N` octets suivants sous forme de tableau.
    pub(crate) fn take_array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    /// Tout ce qui reste (vide si la lecture est finie).
    pub(crate) fn take_rest(&mut self) -> &'a [u8] {
        core::mem::take(&mut self.rest)
    }
}

#[cfg(test)]
mod tests {
    use super::super::consts::TTL_DEFAULT;
    use super::*;
    use alloc::vec;

    fn noise_msg(payload: Vec<u8>) -> Packet {
        Packet {
            header: Header {
                version: PROTO_VERSION,
                packet_type: PacketType::NoiseMsg,
                ttl: TTL_DEFAULT,
                flags: Flags::ADDRESSED | Flags::RELAY_OK,
                timestamp_ms: 0x0000_0191_0000_0000,
                sender_id: [0xaa; PEER_ID_LEN],
                recipient_id: Some([0xbb; PEER_ID_LEN]),
                payload_len: u16::try_from(payload.len()).unwrap(),
            },
            payload,
            signature: None,
        }
    }

    fn announce() -> Packet {
        Packet {
            header: Header {
                version: PROTO_VERSION,
                packet_type: PacketType::Announce,
                ttl: 2,
                flags: Flags::SIGNED | Flags::RELAY_OK,
                timestamp_ms: 42,
                sender_id: [0xaa; PEER_ID_LEN],
                recipient_id: None,
                payload_len: 4,
            },
            payload: vec![1, 2, 3, 4],
            signature: Some([0xcc; SIGNATURE_LEN]),
        }
    }

    #[test]
    fn aller_retour_adresse_non_signe() {
        let p = noise_msg(vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0x11]);
        let raw = encode(&p).unwrap();
        assert_eq!(raw.len(), p.header.wire_len());
        assert_eq!(
            raw,
            hex("010307090000019100000000aaaaaaaaaaaaaaaabbbbbbbbbbbbbbbb0006deadbeef0011")
        );
        assert_eq!(decode(&raw).unwrap(), p);
    }

    #[test]
    fn aller_retour_broadcast_signe() {
        let p = announce();
        let raw = encode(&p).unwrap();
        assert_eq!(raw.len(), HEADER_LEN_BROADCAST + 4 + SIGNATURE_LEN);
        assert_eq!(&raw[HEADER_LEN_BROADCAST + 4..], &[0xcc; SIGNATURE_LEN]);
        assert_eq!(decode(&raw).unwrap(), p);
    }

    #[test]
    fn entree_de_signature_survit_au_relais() {
        // Retour de revue #80 (Paul) : Alice signe avec ttl=2, un relais
        // décrémente à 1 — l'entrée de signature doit rester identique.
        let p = announce();
        let signe = signing_input(&p).unwrap();
        assert_eq!(signe.len(), HEADER_LEN_BROADCAST + 4);
        assert_eq!(signe[TTL_OFFSET], 0);

        let mut raw = encode(&p).unwrap();
        assert_eq!(&signe[..TTL_OFFSET], &raw[..TTL_OFFSET]);
        assert_eq!(&signe[TTL_OFFSET + 1..], &raw[TTL_OFFSET + 1..signe.len()]);
        assert_eq!(received_signing_input(&raw).unwrap(), Some(signe.clone()));

        raw[TTL_OFFSET] -= 1; // relais
        assert_eq!(received_signing_input(&raw).unwrap(), Some(signe));
    }

    #[test]
    fn entree_de_signature_couvre_le_reste_de_l_en_tete() {
        let base = received_signing_input(&encode(&announce()).unwrap())
            .unwrap()
            .unwrap();
        let mut p = announce();
        p.header.timestamp_ms += 1;
        assert_ne!(signing_input(&p).unwrap(), base);

        // Bit réservé posé par un pair v1.1 : conservé dans l'entrée (c'est
        // ce que l'émetteur a signé), alors que `decode` le masque.
        let mut raw = encode(&announce()).unwrap();
        raw[3] |= 0x20;
        let recue = received_signing_input(&raw).unwrap().unwrap();
        assert_eq!(recue[3], raw[3]);
        assert_ne!(recue, base);
    }

    #[test]
    fn entree_de_signature_paquet_non_signe() {
        let p = noise_msg(vec![1]);
        assert_eq!(received_signing_input(&encode(&p).unwrap()), Ok(None));
        assert_eq!(signing_input(&p), Err(EncodeError::SignatureMismatch));
        assert_eq!(
            received_signing_input(&[]),
            Err(DecodeError::Truncated {
                needed: HEADER_LEN_BROADCAST,
                got: 0
            })
        );
        // Le champ signature est ignoré à l'émission : on le calcule.
        let mut a = announce();
        a.signature = None;
        assert_eq!(
            signing_input(&a).unwrap(),
            signing_input(&announce()).unwrap()
        );
    }

    #[test]
    fn payload_vide_accepte() {
        let p = noise_msg(Vec::new());
        assert_eq!(decode(&encode(&p).unwrap()).unwrap(), p);
    }

    #[test]
    fn encode_into_ajoute_sans_ecraser_et_n_ecrit_rien_en_erreur() {
        let mut out = vec![0x77];
        encode_into(&noise_msg(vec![1]), &mut out).unwrap();
        assert_eq!(out[0], 0x77);
        assert_eq!(out.len(), 1 + HEADER_LEN_ADDRESSED + 1);

        let mut bad = noise_msg(vec![1]);
        bad.header.payload_len = 2;
        let mut out = vec![0x77];
        assert!(encode_into(&bad, &mut out).is_err());
        assert_eq!(out, vec![0x77]);
    }

    #[test]
    fn decode_masque_les_bits_reserves() {
        let mut raw = encode(&noise_msg(Vec::new())).unwrap();
        raw[3] |= 0b1110_0000;
        let p = decode(&raw).unwrap();
        assert_eq!(p.header.flags, Flags::ADDRESSED | Flags::RELAY_OK);
        assert!(!p.header.flags.has_reserved());
    }

    #[test]
    fn decode_erreurs_de_forme() {
        let good = encode(&noise_msg(vec![1, 2])).unwrap();

        assert_eq!(
            decode(&[]),
            Err(DecodeError::Truncated {
                needed: HEADER_LEN_BROADCAST,
                got: 0
            })
        );
        assert_eq!(
            decode(&good[..HEADER_LEN_BROADCAST]),
            Err(DecodeError::Truncated {
                needed: HEADER_LEN_ADDRESSED,
                got: HEADER_LEN_BROADCAST
            })
        );

        let mut v = good.clone();
        v[0] = 2;
        assert_eq!(decode(&v), Err(DecodeError::UnsupportedVersion(2)));

        let mut v = good.clone();
        v[1] = 0x0e;
        assert_eq!(decode(&v), Err(DecodeError::UnknownType(0x0e)));

        let expected = good.len();
        assert_eq!(
            decode(&good[..expected - 1]),
            Err(DecodeError::LengthMismatch {
                expected,
                got: expected - 1
            })
        );
        let mut v = good;
        v.push(0);
        assert_eq!(
            decode(&v),
            Err(DecodeError::LengthMismatch {
                expected,
                got: expected + 1
            })
        );
    }

    #[test]
    fn decode_refuse_announce_sans_signed() {
        // Attaque visée : retirer SIGNED + la signature d'un ANNOUNCE pour
        // échapper au « signature OK (si SIGNED) » du pipeline de réception.
        let mut raw = encode(&announce()).unwrap();
        raw.truncate(raw.len() - SIGNATURE_LEN);
        raw[3] &= !Flags::SIGNED.bits();
        assert_eq!(
            decode(&raw),
            Err(DecodeError::Rule(FrameRule::MissingSignature {
                packet_type: PacketType::Announce
            }))
        );
    }

    #[test]
    fn decode_refuse_adressage_contraire_au_type() {
        // ANNOUNCE (jamais adressé) avec ADDRESSED + recipient_id.
        let mut p = announce();
        p.header.flags = p.header.flags | Flags::ADDRESSED;
        p.header.recipient_id = Some([1; PEER_ID_LEN]);
        let mut raw = Vec::new();
        raw.extend_from_slice(&[
            p.header.version,
            p.header.packet_type.to_u8(),
            p.header.ttl,
            p.header.flags.bits(),
        ]);
        raw.extend_from_slice(&p.header.timestamp_ms.to_be_bytes());
        raw.extend_from_slice(&p.header.sender_id);
        raw.extend_from_slice(&[1; PEER_ID_LEN]);
        raw.extend_from_slice(&4u16.to_be_bytes());
        raw.extend_from_slice(&p.payload);
        raw.extend_from_slice(&[0xcc; SIGNATURE_LEN]);
        let rule = FrameRule::Addressing {
            packet_type: PacketType::Announce,
            expected: false,
        };
        assert_eq!(decode(&raw), Err(DecodeError::Rule(rule)));
        assert_eq!(encode(&p), Err(EncodeError::Rule(rule)));
    }

    #[test]
    fn fragment_herite_de_l_adressage_mais_exige_le_drapeau() {
        let mut p = noise_msg(vec![0; 12]);
        p.header.packet_type = PacketType::Fragment;
        p.header.flags = p.header.flags | Flags::FRAGMENT;
        assert_eq!(decode(&encode(&p).unwrap()).unwrap(), p);

        // Broadcast : aussi valide (hérite).
        p.header.flags = Flags::FRAGMENT | Flags::RELAY_OK;
        p.header.recipient_id = None;
        assert_eq!(decode(&encode(&p).unwrap()).unwrap(), p);

        // Sans FRAGMENT : refusé.
        p.header.flags = Flags::RELAY_OK;
        let rule = FrameRule::FragmentFlag {
            packet_type: PacketType::Fragment,
        };
        assert_eq!(encode(&p), Err(EncodeError::Rule(rule)));

        // FRAGMENT sur un autre type : refusé.
        let mut q = noise_msg(Vec::new());
        q.header.flags = q.header.flags | Flags::FRAGMENT;
        let mut raw = encode(&noise_msg(Vec::new())).unwrap();
        raw[3] |= Flags::FRAGMENT.bits();
        let rule = FrameRule::FragmentFlag {
            packet_type: PacketType::NoiseMsg,
        };
        assert_eq!(decode(&raw), Err(DecodeError::Rule(rule)));
        assert_eq!(encode(&q), Err(EncodeError::Rule(rule)));
    }

    #[test]
    fn encode_refuse_les_paquets_incoherents() {
        let mut p = noise_msg(Vec::new());
        p.header.version = 2;
        assert_eq!(encode(&p), Err(EncodeError::UnsupportedVersion(2)));

        let mut p = noise_msg(Vec::new());
        p.header.flags = Flags::from_bits_raw(p.header.flags.bits() | 0x20);
        assert_eq!(encode(&p), Err(EncodeError::ReservedFlags(0x29)));

        let mut p = noise_msg(vec![0; 3]);
        p.header.payload_len = 2;
        assert_eq!(
            encode(&p),
            Err(EncodeError::PayloadLenMismatch {
                header: 2,
                actual: 3
            })
        );

        let mut p = noise_msg(Vec::new());
        p.payload = vec![0; PAYLOAD_MAX + 1];
        assert_eq!(
            encode(&p),
            Err(EncodeError::PayloadTooLarge(PAYLOAD_MAX + 1))
        );

        let mut p = noise_msg(Vec::new());
        p.header.recipient_id = None;
        assert_eq!(encode(&p), Err(EncodeError::RecipientMismatch));

        let mut p = noise_msg(Vec::new());
        p.signature = Some([0; SIGNATURE_LEN]);
        assert_eq!(encode(&p), Err(EncodeError::SignatureMismatch));

        let mut p = announce();
        p.signature = None;
        assert_eq!(encode(&p), Err(EncodeError::SignatureMismatch));

        let mut p = announce();
        p.header.flags = Flags::RELAY_OK;
        p.signature = None;
        assert_eq!(
            encode(&p),
            Err(EncodeError::Rule(FrameRule::MissingSignature {
                packet_type: PacketType::Announce
            }))
        );
    }

    #[test]
    fn messages_d_erreur_non_vides() {
        use alloc::string::ToString;
        let errors: [&dyn core::error::Error; 4] = [
            &DecodeError::UnknownType(0xff),
            &DecodeError::Rule(FrameRule::FragmentFlag {
                packet_type: PacketType::Ack,
            }),
            &EncodeError::SignatureMismatch,
            &EncodeError::ReservedFlags(0x20),
        ];
        for e in errors {
            assert!(!e.to_string().is_empty());
        }
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}
