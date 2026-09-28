//! Tests du courrier : dépôt, offre / collecte, expiration, bornes, et test
//! négatif « le courrier ne peut pas lire ce qu'il transporte ».

use alloc::string::ToString;
use alloc::vec;

use proptest::prelude::*;

use super::*;
use crate::protocol::consts::PROTO_VERSION;
use crate::protocol::{encode, Flags, Header, Packet};

const T0: u64 = 1_700_000_000_000;
const TTL_MS: u64 = MSG_TTL_S as u64 * 1000;
const DAY: u16 = 19_675;

/// Paquet `SEALED_ENVELOPE` encodé : `tag ‖ day ‖ ciphertext`, signé (octets
/// de signature factices : la vérification est faite en amont du courrier).
fn envelope_packet(tag: RecipientTag, sent_ms: u64, ciphertext: &[u8]) -> Vec<u8> {
    let mut payload = tag.to_vec();
    payload.extend_from_slice(&DAY.to_be_bytes());
    payload.extend_from_slice(ciphertext);
    let packet = Packet {
        header: Header {
            version: PROTO_VERSION,
            packet_type: PacketType::SealedEnvelope,
            ttl: 7,
            flags: Flags::SIGNED | Flags::RELAY_OK,
            timestamp_ms: sent_ms,
            sender_id: [0x5E; 8],
            recipient_id: None,
            payload_len: u16::try_from(payload.len()).unwrap(),
        },
        payload,
        signature: Some([0xA5; 64]),
    };
    encode(&packet).unwrap()
}

fn id(n: u8) -> MsgId {
    [n; 32]
}

fn tag(n: u8) -> RecipientTag {
    [n; RECIPIENT_TAG_LEN]
}

fn deposit(c: &mut Courier, n: u8, tag_n: u8, now: u64) -> Result<DepositOutcome, CourierError> {
    c.deposit(id(n), envelope_packet(tag(tag_n), now, b"opaque"), now)
}

// --- Dépôt ------------------------------------------------------------------

#[test]
fn depot_puis_collecte_par_tag() {
    let mut c = Courier::default();
    let raw = envelope_packet(tag(1), T0, b"ciphertext");
    assert_eq!(
        c.deposit(id(1), raw.clone(), T0),
        Ok(DepositOutcome::Stored { evicted: None })
    );
    deposit(&mut c, 2, 2, T0).unwrap();
    deposit(&mut c, 3, 1, T0).unwrap();
    assert_eq!(c.len(), 3);

    let held = c.get(&id(1)).unwrap();
    assert_eq!(held.recipient_tag, tag(1));
    assert_eq!(held.epoch_day, DAY);
    assert_eq!(held.packet, raw, "réémis octet pour octet");
    assert_eq!(held.deposit_ms, T0);
    assert_eq!(held.expires_ms, T0 + TTL_MS);

    assert_eq!(c.offer(T0), vec![tag(1), tag(2)]);
    let ids: Vec<MsgId> = c.matching(&[tag(1)], T0).iter().map(|e| e.msg_id).collect();
    assert_eq!(ids, vec![id(1), id(3)]);
    assert!(c.matching(&[tag(9)], T0).is_empty());

    // Remise confirmée : l'enveloppe quitte le magasin.
    assert_eq!(c.confirm_handoff(&id(1)).unwrap().packet, raw);
    assert_eq!(c.confirm_handoff(&id(1)), None);
    assert_eq!(c.len(), 2);
}

#[test]
fn depot_en_double_ignore() {
    let mut c = Courier::default();
    deposit(&mut c, 1, 1, T0).unwrap();
    assert_eq!(deposit(&mut c, 1, 1, T0 + 5), Ok(DepositOutcome::Duplicate));
    assert_eq!(c.get(&id(1)).unwrap().deposit_ms, T0);
    assert_eq!(c.len(), 1);
}

#[test]
fn depot_refuse_ce_qui_n_est_pas_une_enveloppe_valide() {
    let mut c = Courier::default();
    assert!(matches!(
        c.deposit(id(1), vec![1, 2, 3], T0),
        Err(CourierError::Decode(_))
    ));

    // Autre type de paquet.
    let mut p = crate::protocol::decode(&envelope_packet(tag(1), T0, b"x")).unwrap();
    p.header.packet_type = PacketType::Announce;
    let raw = encode(&p).unwrap();
    assert_eq!(
        c.deposit(id(1), raw, T0),
        Err(CourierError::NotSealedEnvelope(PacketType::Announce))
    );

    // Payload réduit à l'en-tête, sans ciphertext.
    let mut p = crate::protocol::decode(&envelope_packet(tag(1), T0, b"x")).unwrap();
    p.payload.truncate(SEALED_HEADER_LEN);
    p.header.payload_len = u16::try_from(SEALED_HEADER_LEN).unwrap();
    assert_eq!(
        c.deposit(id(1), encode(&p).unwrap(), T0),
        Err(CourierError::MalformedEnvelope)
    );

    // Trop gros.
    let big = envelope_packet(tag(1), T0, &vec![0; ENVELOPE_MAX_BYTES]);
    assert_eq!(c.deposit(id(1), big, T0), Err(CourierError::TooLarge));
    assert!(c.is_empty());
}

#[test]
fn format_du_payload_scelle() {
    let mut payload = tag(7).to_vec();
    payload.extend_from_slice(&[0x01, 0x02, 0xFF]);
    let (h, ct) = parse_sealed_payload(&payload).unwrap();
    assert_eq!(h.recipient_tag, tag(7));
    assert_eq!(h.epoch_day, 0x0102);
    assert_eq!(ct, &[0xFF]);
    assert_eq!(
        parse_sealed_payload(&payload[..SEALED_HEADER_LEN]),
        Err(CourierError::MalformedEnvelope)
    );
}

// --- Expiration ---------------------------------------------------------------

#[test]
fn expiration_supprime_l_enveloppe_perimee() {
    let mut c = Courier::default();
    deposit(&mut c, 1, 1, T0).unwrap();
    deposit(&mut c, 2, 2, T0 + 1000).unwrap();

    // À l'échéance exacte : toujours là, toujours offerte.
    assert!(c.expire(T0 + TTL_MS).is_empty());
    assert_eq!(c.offer(T0 + TTL_MS), vec![tag(1), tag(2)]);

    // Juste après : ni offerte, ni collectable, puis supprimée.
    assert_eq!(c.offer(T0 + TTL_MS + 1), vec![tag(2)]);
    assert!(c.matching(&[tag(1)], T0 + TTL_MS + 1).is_empty());
    assert_eq!(c.expire(T0 + TTL_MS + 1), vec![id(1)]);
    assert_eq!(c.len(), 1);
}

#[test]
fn echeance_bornee_par_l_horodatage_d_origine() {
    let mut c = Courier::default();
    // Enveloppe émise il y a 20 h, déposée maintenant : il lui reste 4 h,
    // pas 24 h — changer de courrier ne prolonge pas sa vie.
    let sent = T0 - 20 * 3_600_000;
    c.deposit(id(1), envelope_packet(tag(1), sent, b"x"), T0)
        .unwrap();
    assert_eq!(c.get(&id(1)).unwrap().expires_ms, sent + TTL_MS);

    // Horloge d'émetteur en avance : bornée par l'instant du dépôt.
    let future = T0 + 3_600_000;
    c.deposit(id(2), envelope_packet(tag(2), future, b"x"), T0)
        .unwrap();
    assert_eq!(c.get(&id(2)).unwrap().expires_ms, T0 + TTL_MS);

    // Déjà périmée à l'arrivée : refusée.
    let old = T0 - TTL_MS - 1;
    assert_eq!(
        c.deposit(id(3), envelope_packet(tag(3), old, b"x"), T0),
        Err(CourierError::Expired)
    );
}

#[test]
fn le_depot_purge_d_abord_les_perimees() {
    let mut c = Courier::new(CourierConfig {
        capacity: 1,
        ..CourierConfig::default()
    });
    deposit(&mut c, 1, 1, T0).unwrap();
    // Plein, mais la première a expiré : la place se libère.
    let later = T0 + TTL_MS + 1;
    assert_eq!(
        deposit(&mut c, 2, 2, later),
        Ok(DepositOutcome::Stored { evicted: None })
    );
    assert!(c.get(&id(1)).is_none());
}

// --- Bornes et politique d'éviction -----------------------------------------------

#[test]
fn magasin_plein_refuse_par_defaut() {
    let config = CourierConfig::default();
    assert_eq!(config.capacity, ENVELOPE_STORE_MAX);
    assert_eq!(config.policy, EvictionPolicy::RejectNew);
    let mut c = Courier::new(config);
    assert_eq!(c.config(), config);
    for n in 0..u8::try_from(ENVELOPE_STORE_MAX).unwrap() {
        deposit(&mut c, n, n, T0).unwrap();
    }
    assert_eq!(deposit(&mut c, 200, 200, T0), Err(CourierError::Full));
    assert_eq!(c.len(), ENVELOPE_STORE_MAX);
    assert!(
        c.get(&id(0)).is_some(),
        "les enveloppes acceptées sont protégées"
    );
}

#[test]
fn eviction_du_plus_ancien_si_configuree() {
    let mut c = Courier::new(CourierConfig {
        capacity: 2,
        policy: EvictionPolicy::EvictOldest,
        ..CourierConfig::default()
    });
    deposit(&mut c, 1, 1, T0).unwrap();
    deposit(&mut c, 2, 2, T0 + 1).unwrap();
    assert_eq!(
        deposit(&mut c, 3, 3, T0 + 2),
        Ok(DepositOutcome::Stored {
            evicted: Some(id(1))
        })
    );
    assert_eq!(c.offer(T0 + 2), vec![tag(2), tag(3)]);
}

#[test]
fn capacite_nulle() {
    for policy in [EvictionPolicy::RejectNew, EvictionPolicy::EvictOldest] {
        let mut c = Courier::new(CourierConfig {
            capacity: 0,
            policy,
            ..CourierConfig::default()
        });
        assert_eq!(deposit(&mut c, 1, 1, T0), Err(CourierError::Full));
        assert!(c.is_empty());
    }
}

#[test]
fn messages_d_erreur() {
    let decode_err = crate::protocol::decode(&[]).unwrap_err();
    for e in [
        CourierError::Decode(decode_err),
        CourierError::NotSealedEnvelope(PacketType::Ack),
        CourierError::MalformedEnvelope,
        CourierError::TooLarge,
        CourierError::Expired,
        CourierError::Full,
    ] {
        assert!(!e.to_string().is_empty());
    }
}

// --- Test négatif : le courrier ne peut pas lire ce qu'il transporte ---------

/// Noise `X` (US-204, PR #81) n'est pas encore sur `main` : l'expéditeur est
/// simulé ici par un AEAD XChaCha20-Poly1305 sous une clé que seul le
/// destinataire possède. Ce que le test établit ne dépend pas de l'AEAD :
/// le courrier ne reçoit aucune clé, stocke et rend le ciphertext intact, et
/// aucun octet du clair n'apparaît dans ce qu'il détient.
#[cfg(feature = "std")]
mod lecture_impossible {
    use super::*;
    use chacha20poly1305::aead::{Aead, KeyInit};
    use chacha20poly1305::{XChaCha20Poly1305, XNonce};

    const CLAIR: &[u8] = b"rendez-vous au refuge a 18h, apporte la radio";
    const NONCE: [u8; 24] = [7; 24];

    fn sceller(cle: &[u8; 32]) -> Vec<u8> {
        let aead = XChaCha20Poly1305::new(cle.into());
        let mut out = NONCE.to_vec();
        out.extend(aead.encrypt(XNonce::from_slice(&NONCE), CLAIR).unwrap());
        out
    }

    fn ouvrir(cle: &[u8; 32], ciphertext: &[u8]) -> Option<Vec<u8>> {
        let (nonce, ct) = ciphertext.split_at(24);
        XChaCha20Poly1305::new(cle.into())
            .decrypt(XNonce::from_slice(nonce), ct)
            .ok()
    }

    #[test]
    fn le_courrier_ne_peut_pas_dechiffrer_ce_qu_il_transporte() {
        let cle_destinataire = [0x11; 32];
        let cle_courrier = [0x22; 32];

        let mut c = Courier::default();
        let raw = envelope_packet(tag(1), T0, &sceller(&cle_destinataire));
        c.deposit(id(1), raw, T0).unwrap();

        let held = c.matching(&[tag(1)], T0)[0];
        // Aucun fragment du clair dans ce que détient le courrier.
        assert!(
            !held
                .packet
                .windows(8)
                .any(|w| CLAIR.windows(8).any(|c| c == w)),
            "le clair apparaît dans le paquet détenu"
        );
        let payload = crate::protocol::decode(&held.packet).unwrap().payload;
        let (_, ciphertext) = parse_sealed_payload(&payload).unwrap();

        // Avec sa propre clé, le courrier échoue ; le destinataire réussit.
        assert_eq!(ouvrir(&cle_courrier, ciphertext), None);
        assert_eq!(ouvrir(&cle_destinataire, ciphertext).unwrap(), CLAIR);
    }
}

// --- Property tests -----------------------------------------------------------

#[derive(Debug, Clone)]
enum Op {
    Deposit { n: u8, tag: u8, age_ms: u64 },
    Handoff(u8),
    Tick(u64),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (any::<u8>(), 0..6u8, 0..2 * TTL_MS)
            .prop_map(|(n, tag, age_ms)| Op::Deposit { n, tag, age_ms }),
        1 => any::<u8>().prop_map(Op::Handoff),
        1 => (0..TTL_MS / 4).prop_map(Op::Tick),
    ]
}

fn policy() -> impl Strategy<Value = EvictionPolicy> {
    prop_oneof![
        Just(EvictionPolicy::RejectNew),
        Just(EvictionPolicy::EvictOldest)
    ]
}

proptest! {
    /// Stockage borné, politique respectée, rien de périmé n'est jamais
    /// offert ni collecté.
    #[test]
    fn stockage_borne_et_politique_respectee(
        ops in proptest::collection::vec(op(), 0..120),
        capacity in 0..8usize,
        policy in policy(),
    ) {
        let mut c = Courier::new(CourierConfig { capacity, policy, ..CourierConfig::default() });
        let mut now = T0;
        for op in ops {
            match op {
                Op::Deposit { n, tag: t, age_ms } => {
                    let before: Vec<MsgId> = c.envelopes.keys().copied().collect();
                    let was_full = c.len() >= capacity;
                    let res = c.deposit(id(n), envelope_packet(tag(t), now - age_ms, b"x"), now);
                    if let Ok(DepositOutcome::Stored { evicted }) = res {
                        prop_assert_eq!(c.get(&id(n)).unwrap().deposit_ms, now);
                        if policy == EvictionPolicy::RejectNew {
                            prop_assert_eq!(evicted, None);
                        }
                    }
                    if policy == EvictionPolicy::RejectNew && was_full && res == Err(CourierError::Full) {
                        let after: Vec<MsgId> = c.envelopes.keys().copied().collect();
                        prop_assert!(after.iter().all(|k| before.contains(k)));
                    }
                }
                Op::Handoff(n) => { c.confirm_handoff(&id(n)); }
                Op::Tick(dt) => { now += dt; }
            }
            prop_assert!(c.len() <= capacity);
            for e in c.matching(&c.offer(now), now) {
                prop_assert!(now <= e.expires_ms);
            }
        }
    }

    /// Le courrier rend exactement les octets reçus.
    #[test]
    fn octets_preserves(ciphertext in proptest::collection::vec(any::<u8>(), 1..2000)) {
        let raw = envelope_packet(tag(3), T0, &ciphertext);
        let mut c = Courier::default();
        c.deposit(id(1), raw.clone(), T0).unwrap();
        prop_assert_eq!(&c.matching(&[tag(3)], T0)[0].packet, &raw);
    }

    #[test]
    fn depot_sans_panic(bytes in proptest::collection::vec(any::<u8>(), 0..300)) {
        let mut c = Courier::default();
        let _ = c.deposit(id(1), bytes, T0);
        prop_assert!(c.len() <= 1);
    }
}
