//! Tests unitaires et property de `sync::inventory` (US-210).

use alloc::collections::BTreeSet;
use alloc::vec;
use alloc::vec::Vec;

use proptest::prelude::*;

use super::*;
use crate::protocol::consts::{PROTO_VERSION, TTL_DEFAULT};
use crate::protocol::PeerId;
use crate::sync::routing::{NoRelayReason, RejectReason};

const T0: u64 = 1_800_000_000_000;
const MINUTE: u64 = 60_000;
const A: u8 = 1;
const B: u8 = 2;

/// Instant où les deux horloges coïncident (cas nominal des tests).
fn a(t: u64) -> Now {
    Now::new(t, t)
}

fn id(n: u32) -> MsgId {
    let mut m = [0u8; 32];
    m[..4].copy_from_slice(&n.to_be_bytes());
    m
}

fn octets(n: u32) -> Vec<u8> {
    n.to_be_bytes().to_vec()
}

fn inv() -> Inventory<u8> {
    Inventory::new(InventoryConfig::new())
}

/// Un cache contenant `ids`, reçus à `t`.
fn avec(ids: impl IntoIterator<Item = u32>, t: u64) -> Inventory<u8> {
    let mut i = inv();
    for n in ids {
        assert!(i.remember(id(n), t, 6, octets(n), a(t)));
    }
    i
}

fn entete(packet_type: PacketType, ttl: u8, flags: Flags) -> Header {
    let dest: Option<PeerId> = flags.contains(Flags::ADDRESSED).then_some([9; 8]);
    Header {
        version: PROTO_VERSION,
        packet_type,
        ttl,
        flags,
        timestamp_ms: T0,
        sender_id: [0x11; 8],
        recipient_id: dest,
        payload_len: 0,
    }
}

// --- Payload ----------------------------------------------------------------

#[test]
fn payload_aller_retour() {
    for n in [0u32, 1, 3, 120] {
        let ids: Vec<MsgId> = (0..n).map(id).collect();
        let p = encode_payload(&ids);
        assert_eq!(p.as_ref().map(Vec::len), Ok(2 + ids.len() * MSG_ID_LEN));
        assert_eq!(p.and_then(|p| decode_payload(&p)), Ok(ids));
    }
}

#[test]
fn payload_count_big_endian() {
    let p = encode_payload(&[id(7), id(8)]);
    assert_eq!(p.map(|p| [p[0], p[1]]), Ok([0, 2]));
}

#[test]
fn payload_mal_forme_rejete() {
    assert_eq!(decode_payload(&[]), Err(PayloadError::Truncated));
    assert_eq!(decode_payload(&[0]), Err(PayloadError::Truncated));
    // count = 1 mais aucun msgID.
    assert_eq!(
        decode_payload(&[0, 1]),
        Err(PayloadError::LengthMismatch {
            expected: 34,
            got: 2
        })
    );
    // count = 0 mais des octets en trop.
    assert_eq!(
        decode_payload(&[0, 0, 5]),
        Err(PayloadError::LengthMismatch {
            expected: 2,
            got: 3
        })
    );
    assert_eq!(
        decode_payload(&[0xFF, 0xFF]),
        Err(PayloadError::TooMany(0xFFFF))
    );
}

#[test]
fn payload_trop_d_ids_refuse_a_l_encodage() {
    let ids = vec![id(0); INVENTORY_MAX_IDS + 1];
    assert_eq!(
        encode_payload(&ids),
        Err(PayloadError::TooMany(INVENTORY_MAX_IDS + 1))
    );
    // La borne tient dans payload_len.
    assert!(2 + INVENTORY_MAX_IDS * MSG_ID_LEN <= usize::from(u16::MAX));
    let pile = encode_payload(&vec![id(0); INVENTORY_MAX_IDS]);
    assert!(pile.is_ok());
}

#[test]
fn erreurs_de_payload_lisibles() {
    for e in [
        PayloadError::Truncated,
        PayloadError::TooMany(3000),
        PayloadError::LengthMismatch {
            expected: 34,
            got: 2,
        },
    ] {
        assert!(!alloc::format!("{e}").is_empty());
    }
}

// --- cacheable --------------------------------------------------------------

#[test]
fn cacheable_selon_la_decision() {
    let h = entete(PacketType::NoiseMsg, TTL_DEFAULT, Flags::RELAY_OK);
    assert_eq!(
        cacheable(&h, &Decision::RelayScheduled { at_ms: 0, ttl: 5 }),
        Some(5)
    );
    assert_eq!(
        cacheable(&h, &Decision::NoRelay(NoRelayReason::Late)),
        Some(TTL_DEFAULT - 1)
    );
    assert_eq!(cacheable(&h, &Decision::Deliver), None);
    assert_eq!(
        cacheable(&h, &Decision::Reject(RejectReason::Duplicate)),
        None
    );

    let env = entete(
        PacketType::SealedEnvelope,
        3,
        Flags::SIGNED | Flags::RELAY_OK,
    );
    assert_eq!(cacheable(&env, &Decision::Store), Some(2));
}

#[test]
fn cacheable_respecte_la_portee_voulue_par_l_emetteur() {
    let sans_relais = entete(PacketType::NoiseMsg, TTL_DEFAULT, Flags::empty());
    assert_eq!(
        cacheable(
            &sans_relais,
            &Decision::NoRelay(NoRelayReason::RelayNotAllowed)
        ),
        None
    );
    let epuise = entete(PacketType::SealedEnvelope, 1, Flags::RELAY_OK);
    assert_eq!(cacheable(&epuise, &Decision::Store), None);
}

#[test]
fn cacheable_seulement_les_types_store_and_forward() {
    let oui = [
        PacketType::SealedEnvelope,
        PacketType::NoiseMsg,
        PacketType::Ack,
    ];
    let non = [
        PacketType::Announce,
        PacketType::NoiseHs,
        PacketType::Fragment,
        PacketType::LogAttest,
        PacketType::EnvelopeOffer,
        PacketType::EnvelopeRequest,
        PacketType::Inventory,
        PacketType::GossipPush,
    ];
    let d = Decision::RelayScheduled { at_ms: 0, ttl: 4 };
    for t in oui {
        assert_eq!(
            cacheable(&entete(t, 5, Flags::RELAY_OK), &d),
            Some(4),
            "{t:?}"
        );
    }
    for t in non {
        assert_eq!(cacheable(&entete(t, 5, Flags::RELAY_OK), &d), None, "{t:?}");
    }
}

// --- Cache ------------------------------------------------------------------

#[test]
fn remember_ignore_un_doublon() {
    let mut i = inv();
    assert!(i.remember(id(1), T0, 6, octets(1), a(T0)));
    assert!(!i.remember(id(1), T0, 6, octets(1), a(T0 + 1)));
    assert_eq!(i.len(), 1);
    assert!(i.contains(&id(1)));
    assert!(!i.is_empty());
    assert_eq!(i.stats().remembered, 1);
}

#[test]
fn eviction_du_plus_ancien_a_la_capacite() {
    let mut i: Inventory<u8> = Inventory::new(InventoryConfig {
        cap: 3,
        ..InventoryConfig::new()
    });
    for n in 0..5 {
        assert!(i.remember(id(n), T0, 6, octets(n), a(T0 + u64::from(n))));
    }
    assert_eq!(i.ids(), vec![id(2), id(3), id(4)]);
    assert_eq!(i.stats().evicted, 2);
}

#[test]
fn capacite_nulle_ne_retient_rien() {
    let mut i: Inventory<u8> = Inventory::new(InventoryConfig {
        cap: 0,
        ..InventoryConfig::new()
    });
    assert!(!i.remember(id(1), T0, 6, octets(1), a(T0)));
    assert!(i.is_empty());
}

#[test]
fn meme_milliseconde_ordre_d_arrivee() {
    let i = avec([9, 3, 5], T0);
    assert_eq!(i.ids(), vec![id(9), id(3), id(5)]);
}

#[test]
fn expiration_apres_la_fenetre_de_six_heures() {
    let mut i = avec([1], T0);
    i.purge(a(T0 + INVENTORY_WINDOW_MS - 1));
    assert!(i.contains(&id(1)));
    i.purge(a(T0 + INVENTORY_WINDOW_MS));
    assert!(!i.contains(&id(1)));
    assert_eq!(i.stats().expired, 1);
}

#[test]
fn expiration_selon_l_horodatage_du_message() {
    let msg_ttl = InventoryConfig::new().msg_ttl_ms;
    // Horodaté 23 h 59 dans le passé : entre, puis expire à 24 h.
    let ts = T0 - msg_ttl + MINUTE;
    let mut i = inv();
    assert!(i.remember(id(1), ts, 6, octets(1), a(T0)));
    i.purge(a(T0 + MINUTE + 1));
    assert!(i.is_empty());
    // Déjà trop vieux : refusé d'emblée.
    assert!(!i.remember(id(2), T0 - msg_ttl - 1, 6, octets(2), a(T0)));
}

#[test]
fn recul_de_l_horloge_murale_ne_purge_rien() {
    let mut i = avec([1], T0);
    // L'heure murale recule d'une heure, la monotone avance d'une seconde.
    i.purge(Now::new(T0 - 3_600_000, T0 + 1_000));
    assert!(i.contains(&id(1)));
}

#[test]
fn forget_retire_du_cache_et_des_files() {
    let mut i = avec([1, 2], T0);
    let _ = i.link_up(B, a(T0));
    assert_eq!(i.on_inventory(B, &[], a(T0)), 2);
    assert!(i.forget(&id(1)));
    assert!(!i.forget(&id(1)));
    assert_eq!(i.queued(B), 1);
    let pushes = i.poll_push(a(T0));
    assert_eq!(pushes.len(), 1);
    assert_eq!(pushes[0].msg_id, id(2));
    assert_eq!(i.stats().forgotten, 1);
}

// --- Session ----------------------------------------------------------------

#[test]
fn link_up_annonce_les_plus_recents_d_abord() {
    let mut i: Inventory<u8> = Inventory::new(InventoryConfig {
        max_ids: 2,
        ..InventoryConfig::new()
    });
    for n in 0..4 {
        assert!(i.remember(id(n), T0, 6, octets(n), a(T0 + u64::from(n))));
    }
    assert_eq!(i.link_up(B, a(T0 + 10)), vec![id(3), id(2)]);
    assert_eq!(i.stats().inventories_sent, 1);
}

#[test]
fn on_inventory_ne_pousse_que_le_manquant() {
    let mut i = avec([1, 2, 3], T0);
    let _ = i.link_up(B, a(T0));
    assert_eq!(i.on_inventory(B, &[id(2), id(99)], a(T0)), 2);
    let ids: Vec<MsgId> = i.poll_push(a(T0)).into_iter().map(|p| p.msg_id).collect();
    assert_eq!(ids, vec![id(1), id(3)]);
}

#[test]
fn on_inventory_lien_inconnu_ignore() {
    let mut i = avec([1], T0);
    assert_eq!(i.on_inventory(B, &[], a(T0)), 0);
    assert_eq!(i.stats().inventories_received, 0);
    assert!(i.poll_push(a(T0)).is_empty());
}

#[test]
fn inventaire_repete_pas_de_double_file() {
    let mut i = avec([1, 2], T0);
    let _ = i.link_up(B, a(T0));
    assert_eq!(i.on_inventory(B, &[], a(T0)), 2);
    assert_eq!(i.on_inventory(B, &[], a(T0)), 0);
    assert_eq!(i.queued(B), 2);
}

#[test]
fn push_porte_les_octets_et_le_ttl() {
    let mut i = inv();
    assert!(i.remember(id(1), T0, 4, vec![0xAB; 10], a(T0)));
    let _ = i.link_up(B, a(T0));
    let _ = i.on_inventory(B, &[], a(T0));
    assert_eq!(
        i.poll_push(a(T0)),
        vec![PushOrder {
            target: B,
            msg_id: id(1),
            ttl: 4,
            bytes: vec![0xAB; 10],
        }]
    );
}

#[test]
fn cadence_bornee_par_minute_puis_reprise() {
    let mut i = avec(0..40, T0);
    let _ = i.link_up(B, a(T0));
    assert_eq!(i.on_inventory(B, &[], a(T0)), 40);

    let max = usize::from(PUSH_MAX_PER_MIN);
    assert_eq!(i.poll_push(a(T0)).len(), max);
    assert!(i.poll_push(a(T0 + MINUTE - 1)).is_empty());
    assert_eq!(i.next_deadline(), Some(T0 + MINUTE));
    assert_eq!(i.poll_push(a(T0 + MINUTE)).len(), max);
    assert_eq!(i.poll_push(a(T0 + 2 * MINUTE)).len(), 40 - 2 * max);
    assert_eq!(i.next_deadline(), None);
    assert_eq!(i.stats().pushes_emitted, 40);
}

#[test]
fn next_deadline_immediate_si_budget_restant() {
    let mut i = avec([1], T0);
    assert_eq!(i.next_deadline(), None);
    let _ = i.link_up(B, a(T0));
    let _ = i.on_inventory(B, &[], a(T0));
    assert_eq!(i.next_deadline(), Some(0));
}

#[test]
fn cadence_nulle_ne_pousse_jamais() {
    let mut i: Inventory<u8> = Inventory::new(InventoryConfig {
        push_max_per_min: 0,
        ..InventoryConfig::new()
    });
    assert!(i.remember(id(1), T0, 6, octets(1), a(T0)));
    let _ = i.link_up(B, a(T0));
    let _ = i.on_inventory(B, &[], a(T0));
    assert!(i.poll_push(a(T0 + 10 * MINUTE)).is_empty());
    assert_eq!(i.next_deadline(), Some(u64::MAX));
}

#[test]
fn cadence_par_lien_independante() {
    let mut i = avec(0..20, T0);
    for l in [A, B] {
        let _ = i.link_up(l, a(T0));
        let _ = i.on_inventory(l, &[], a(T0));
    }
    let pushes = i.poll_push(a(T0));
    let max = usize::from(PUSH_MAX_PER_MIN);
    assert_eq!(pushes.iter().filter(|p| p.target == A).count(), max);
    assert_eq!(pushes.iter().filter(|p| p.target == B).count(), max);
    // Ordre déterministe : lien A d'abord.
    assert!(pushes[..max].iter().all(|p| p.target == A));
}

#[test]
fn link_down_abandonne_la_file() {
    let mut i = avec([1, 2], T0);
    let _ = i.link_up(B, a(T0));
    let _ = i.on_inventory(B, &[], a(T0));
    i.link_down(B);
    assert_eq!(i.queued(B), 0);
    assert!(i.poll_push(a(T0)).is_empty());
    // Le cache, lui, reste.
    assert_eq!(i.len(), 2);
}

#[test]
fn paquet_expire_en_file_saute() {
    let mut i = avec([1], T0);
    assert!(i.remember(id(2), T0, 6, octets(2), a(T0 + INVENTORY_WINDOW_MS / 2)));
    let _ = i.link_up(B, a(T0));
    let _ = i.on_inventory(B, &[], a(T0));
    let pushes = i.poll_push(a(T0 + INVENTORY_WINDOW_MS));
    assert_eq!(
        pushes.into_iter().map(|p| p.msg_id).collect::<Vec<_>>(),
        vec![id(2)]
    );
    assert_eq!(i.queued(B), 0);
}

// --- Property ---------------------------------------------------------------

/// Deux nœuds se rencontrent : échange d'inventaires, puis push jusqu'à
/// épuisement des files. Rend les deux caches finaux et le plus grand
/// nombre de pushs observé par lien sur une fenêtre de 60 s.
fn reconcilier(ga: &BTreeSet<u32>, gb: &BTreeSet<u32>) -> (Vec<MsgId>, Vec<MsgId>, usize) {
    let mut na = avec(ga.iter().copied(), T0);
    let mut nb = avec(gb.iter().copied(), T0);
    let inv_a = na.link_up(B, a(T0));
    let inv_b = nb.link_up(A, a(T0));
    let _ = na.on_inventory(B, &inv_b, a(T0));
    let _ = nb.on_inventory(A, &inv_a, a(T0));

    let mut emis: Vec<u64> = Vec::new();
    let mut pire = 0;
    let mut t = T0;
    while na.queued(B) + nb.queued(A) > 0 {
        let pa = na.poll_push(a(t));
        let pb = nb.poll_push(a(t));
        emis.extend(core::iter::repeat_n(t, pa.len().max(pb.len())));
        for p in pa {
            let _ = nb.remember(p.msg_id, T0, p.ttl, p.bytes, a(t));
        }
        for p in pb {
            let _ = na.remember(p.msg_id, T0, p.ttl, p.bytes, a(t));
        }
        pire = pire.max(emis.iter().filter(|&&e| t - e < MINUTE).count());
        t += 1_000;
    }
    let mut ids_a = na.ids();
    let mut ids_b = nb.ids();
    ids_a.sort_unstable();
    ids_b.sort_unstable();
    (ids_a, ids_b, pire)
}

proptest! {
    #[test]
    fn reconciliation_convergente(
        ga in proptest::collection::btree_set(0u32..80, 0..50),
        gb in proptest::collection::btree_set(0u32..80, 0..50),
    ) {
        let (ids_a, ids_b, pire) = reconcilier(&ga, &gb);
        let union: Vec<MsgId> = ga.union(&gb).copied().map(id).collect();
        prop_assert_eq!(&ids_a, &union);
        prop_assert_eq!(&ids_b, &union);
        prop_assert!(pire <= usize::from(PUSH_MAX_PER_MIN));
    }

    #[test]
    fn jamais_de_push_d_un_id_annonce(
        mien in proptest::collection::btree_set(0u32..60, 0..40),
        sien in proptest::collection::btree_set(0u32..60, 0..40),
    ) {
        let mut i = avec(mien.iter().copied(), T0);
        let _ = i.link_up(B, a(T0));
        let annonce: Vec<MsgId> = sien.iter().copied().map(id).collect();
        let n = i.on_inventory(B, &annonce, a(T0));
        prop_assert_eq!(n, mien.difference(&sien).count());
        let mut t = T0;
        while i.queued(B) > 0 {
            for p in i.poll_push(a(t)) {
                prop_assert!(!annonce.contains(&p.msg_id));
            }
            t += MINUTE;
        }
    }

    #[test]
    fn payload_aller_retour_quelconque(ns in proptest::collection::vec(any::<u32>(), 0..200)) {
        let ids: Vec<MsgId> = ns.into_iter().map(id).collect();
        let p = encode_payload(&ids);
        prop_assert_eq!(p.and_then(|p| decode_payload(&p)), Ok(ids));
    }

    #[test]
    fn decode_ne_panique_jamais(octets in proptest::collection::vec(any::<u8>(), 0..300)) {
        let _ = decode_payload(&octets);
    }
}
