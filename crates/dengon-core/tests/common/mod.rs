//! Outils partagés par les tests de bout en bout de `sync::*`
//! (`routing_mock.rs`, `inventory_mock.rs`).
//!
//! **Codec provisoire.** Cette branche part d'avant `protocol::codec`
//! (US-201) : [`encoder`] / [`decoder`] lisent et écrivent l'en-tête L3
//! **réel** (`synthese/05` §3, big-endian) à la main, sans signature. Le TTL
//! est à l'octet 2 : relais et push le réécrivent en place, sans réencoder.
//! À remplacer par `codec::{encode, decode}` (déjà sur `main`) au rebase.

// Chaque fichier de test compile ce module à part et n'en utilise qu'une
// partie.
#![allow(dead_code)]

use dengon_core::protocol::{Flags, Header, MsgId, PacketType, PeerId, PROTO_VERSION};
use sha2::{Digest, Sha256};

/// Horloge de départ des scénarios (ms UTC).
pub const T0: u64 = 1_800_000_000_000;
/// Octet du TTL dans l'en-tête L3 (`version, type, ttl, flags, …`).
pub const OCTET_TTL: usize = 2;

pub fn encoder(h: &Header, payload: &[u8]) -> Vec<u8> {
    let mut b = vec![h.version, h.packet_type.to_u8(), h.ttl, h.flags.bits()];
    b.extend_from_slice(&h.timestamp_ms.to_be_bytes());
    b.extend_from_slice(&h.sender_id);
    if let Some(r) = h.recipient_id {
        b.extend_from_slice(&r);
    }
    b.extend_from_slice(
        &u16::try_from(payload.len())
            .unwrap_or(u16::MAX)
            .to_be_bytes(),
    );
    b.extend_from_slice(payload);
    b
}

/// En-tête, `msgID` et payload d'un paquet encodé par [`encoder`].
pub fn decoder_complet(b: &[u8]) -> Option<(Header, MsgId, &[u8])> {
    let flags = Flags::from_bits_truncate(*b.get(3)?);
    let mut pos = 12;
    let lire8 = |pos: usize| -> Option<[u8; 8]> { b.get(pos..pos + 8)?.try_into().ok() };
    let timestamp_ms = u64::from_be_bytes(lire8(4)?);
    let sender_id = lire8(pos)?;
    pos += 8;
    let recipient_id = if flags.contains(Flags::ADDRESSED) {
        let r = lire8(pos)?;
        pos += 8;
        Some(r)
    } else {
        None
    };
    let payload_len = u16::from_be_bytes(b.get(pos..pos + 2)?.try_into().ok()?);
    pos += 2;
    let payload = b.get(pos..pos + usize::from(payload_len))?;
    let h = Header {
        version: b[0],
        packet_type: PacketType::from_u8(b[1])?,
        ttl: b[2],
        flags,
        timestamp_ms,
        sender_id,
        recipient_id,
        payload_len,
    };
    // msgID = SHA-256(sender_id ‖ timestamp_ms ‖ type ‖ payload) (A-9) :
    // le TTL n'y entre pas, donc un relais garde le même msgID.
    let mut hasher = Sha256::new();
    hasher.update(sender_id);
    hasher.update(timestamp_ms.to_be_bytes());
    hasher.update([h.packet_type.to_u8()]);
    hasher.update(payload);
    Some((h, hasher.finalize().into(), payload))
}

pub fn decoder(b: &[u8]) -> Option<(Header, MsgId)> {
    decoder_complet(b).map(|(h, id, _)| (h, id))
}

/// Un paquet d'un type donné.
pub fn paquet_type(
    packet_type: PacketType,
    flags: Flags,
    sender: PeerId,
    dest: Option<PeerId>,
    ttl: u8,
    ts: u64,
    payload: &[u8],
) -> Vec<u8> {
    let mut flags = flags;
    if dest.is_some() {
        flags = flags | Flags::ADDRESSED;
    }
    let h = Header {
        version: PROTO_VERSION,
        packet_type,
        ttl,
        flags,
        timestamp_ms: ts,
        sender_id: sender,
        recipient_id: dest,
        payload_len: 0,
    };
    encoder(&h, payload)
}

/// Un `NOISE_MSG` relayable.
pub fn paquet(sender: PeerId, dest: Option<PeerId>, ttl: u8, ts: u64, payload: &[u8]) -> Vec<u8> {
    paquet_type(
        PacketType::NoiseMsg,
        Flags::RELAY_OK,
        sender,
        dest,
        ttl,
        ts,
        payload,
    )
}

pub fn peer(n: u8) -> PeerId {
    [n; 8]
}
