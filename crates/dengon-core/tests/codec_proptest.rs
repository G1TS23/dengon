// Fichier de test intégré : un `unwrap` qui panique = échec de test.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Property tests du codec (US-201, `docs/synthese/10` §4.2).
//!
//! - `decode(encode(p)) == p` pour tout paquet valide généré ;
//! - aucune entrée (octets aléatoires, préfixe, octet muté) ne fait paniquer
//!   `decode` / `decode_app_frame` — c'est le « fuzz léger » demandé par l'US.

use dengon_core::protocol::consts::{PEER_ID_LEN, PROTO_VERSION, SIGNATURE_LEN};
use dengon_core::protocol::{
    decode, decode_app_frame, encode, encode_app_frame, signed_len, AckFrame, AckStatus, AppFrame,
    Flags, Header, MessageFrame, Packet, PacketType,
};
use proptest::prelude::*;

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

/// Paquet **valide** : les drapeaux sont dérivés du type pour respecter
/// `synthese/05` §4 (le codec refuserait sinon, et le round-trip n'aurait
/// pas de sens).
fn arb_packet() -> impl Strategy<Value = Packet> {
    (
        prop::sample::select(&ALL_TYPES[..]),
        any::<u8>(),                // ttl
        any::<u64>(),               // timestamp_ms
        any::<[u8; PEER_ID_LEN]>(), // sender_id
        any::<[u8; PEER_ID_LEN]>(), // recipient_id si adressé
        any::<bool>(),              // adressé (si le type hérite)
        any::<bool>(),              // signé (si le type ne l'impose pas)
        any::<bool>(),              // RELAY_OK
        any::<bool>(),              // PADDED
        prop::collection::vec(any::<u8>(), 0..1200),
        prop::collection::vec(any::<u8>(), SIGNATURE_LEN),
    )
        .prop_map(
            |(pt, ttl, ts, sender, recipient, addr, sig, relay, padded, payload, signature)| {
                let addressed = pt.is_addressed().unwrap_or(addr);
                let signed = pt.is_always_signed() || sig;
                let mut flags = Flags::empty();
                for (on, bit) in [
                    (addressed, Flags::ADDRESSED),
                    (signed, Flags::SIGNED),
                    (pt == PacketType::Fragment, Flags::FRAGMENT),
                    (relay, Flags::RELAY_OK),
                    (padded, Flags::PADDED),
                ] {
                    if on {
                        flags = flags | bit;
                    }
                }
                Packet {
                    header: Header {
                        version: PROTO_VERSION,
                        packet_type: pt,
                        ttl,
                        flags,
                        timestamp_ms: ts,
                        sender_id: sender,
                        recipient_id: addressed.then_some(recipient),
                        payload_len: u16::try_from(payload.len()).unwrap(),
                    },
                    payload,
                    signature: signed.then(|| signature.try_into().unwrap()),
                }
            },
        )
}

fn arb_app_frame() -> impl Strategy<Value = AppFrame> {
    prop_oneof![
        (any::<[u8; 16]>(), any::<u64>(), any::<u64>(), ".{0,200}").prop_map(
            |(msg_uuid, conv_seq, sent_ms, text)| AppFrame::Message(MessageFrame {
                msg_uuid,
                conv_seq,
                sent_ms,
                text,
            })
        ),
        (
            any::<[u8; 16]>(),
            prop::sample::select(&[AckStatus::Delivered, AckStatus::Read][..]),
            any::<u64>()
        )
            .prop_map(|(msg_uuid, status, at_ms)| AppFrame::Ack(AckFrame {
                msg_uuid,
                status,
                at_ms,
            })),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn decode_encode_est_l_identite(p in arb_packet()) {
        let raw = encode(&p).unwrap();
        prop_assert_eq!(raw.len(), p.header.wire_len());
        let back = decode(&raw).unwrap();
        prop_assert_eq!(&back, &p);
        // La signature suit exactement les octets signés.
        if let Some(sig) = &p.signature {
            prop_assert_eq!(&raw[signed_len(&p.header)..], &sig[..]);
        }
    }

    #[test]
    fn tout_prefixe_strict_est_refuse(p in arb_packet(), cut in any::<prop::sample::Index>()) {
        let raw = encode(&p).unwrap();
        let k = cut.index(raw.len());
        prop_assert!(decode(&raw[..k]).is_err());
    }

    #[test]
    fn un_octet_mute_ne_fait_jamais_paniquer(
        p in arb_packet(),
        at in any::<prop::sample::Index>(),
        xor in 1u8..,
    ) {
        let mut raw = encode(&p).unwrap();
        let i = at.index(raw.len());
        raw[i] ^= xor;
        let _ = decode(&raw);
    }

    #[test]
    fn octets_arbitraires_ne_font_jamais_paniquer(raw in prop::collection::vec(any::<u8>(), 0..400)) {
        let _ = decode(&raw);
        let _ = decode_app_frame(&raw);
    }

    #[test]
    fn un_decodage_reussi_se_reencode(raw in prop::collection::vec(any::<u8>(), 0..200)) {
        // Si des octets arbitraires décodent, le paquet obtenu est valide :
        // il se ré-encode, identique aux octets reçus aux bits réservés près.
        if let Ok(p) = decode(&raw) {
            let mut again = encode(&p).unwrap();
            again[3] |= raw[3] & Flags::RESERVED_MASK;
            prop_assert_eq!(again, raw);
        }
    }

    #[test]
    fn app_frame_aller_retour(f in arb_app_frame()) {
        prop_assert_eq!(decode_app_frame(&encode_app_frame(&f)).unwrap(), f);
    }
}
