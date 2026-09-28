//! Tests de l'outbox : transitions, rejeu après redémarrage, stockage en
//! échec, format d'enregistrement.

use alloc::string::ToString;
use alloc::vec;

use proptest::prelude::*;

use super::*;
use crate::sync::status::MSG_UUID_LEN;

const T0: u64 = 1_700_000_000_000;
const TTL_MS: u64 = MSG_TTL_S as u64 * 1000;
const BOB: PeerId = [0xB0; 8];
const RELAY: PeerId = [0x7E; 8];

fn uuid(n: u8) -> MsgUuid {
    [n; MSG_UUID_LEN]
}

fn new_msg(n: u8) -> NewMessage {
    NewMessage {
        msg_uuid: uuid(n),
        dest_peer_id: BOB,
        kind: DeliveryKind::Envelope,
        packet: vec![n; 40],
    }
}

fn outbox() -> Outbox<MemoryStore> {
    Outbox::open(MemoryStore::new()).unwrap()
}

/// Simule un redémarrage : l'outbox est détruite, seul le stockage survit.
fn restart(ob: Outbox<MemoryStore>) -> Outbox<MemoryStore> {
    Outbox::open(ob.into_store()).unwrap()
}

// --- Transitions -----------------------------------------------------------

#[test]
fn enqueue_cree_un_message_queued_persiste() {
    let mut ob = outbox();
    let change = ob.enqueue(new_msg(1), T0).unwrap().unwrap();
    assert_eq!(change.from, None);
    assert_eq!(change.to, Status::Queued);
    assert_eq!(change.event_name(), "msg.queued");

    let rec = ob.get(&uuid(1)).unwrap();
    assert_eq!(rec.status, Status::Queued);
    assert_eq!(rec.expires_ms, T0 + TTL_MS);
    assert_eq!(rec.first_sent_ms, None);
    assert_eq!(ob.len(), 1);
    assert!(!ob.is_empty());
    assert_eq!(ob.iter().count(), 1);
    assert_eq!(ob.into_store().len(), 1);
}

#[test]
fn enqueue_est_idempotent() {
    let mut ob = outbox();
    ob.enqueue(new_msg(1), T0).unwrap();
    let mut autre = new_msg(1);
    autre.packet = vec![0xFF];
    assert_eq!(ob.enqueue(autre, T0 + 5).unwrap(), None);
    assert_eq!(ob.get(&uuid(1)).unwrap().packet, vec![1; 40]);
}

#[test]
fn enqueue_refuse_un_paquet_trop_grand() {
    let mut ob = outbox();
    let mut msg = new_msg(1);
    msg.packet = vec![0; PACKET_MAX_BYTES + 1];
    assert_eq!(
        ob.enqueue(msg, T0),
        Err(OutboxError::Record(RecordError::PacketTooLarge))
    );
    assert!(ob.is_empty());
    assert!(ob.into_store().is_empty());
}

#[test]
fn premiere_remise_passe_in_flight_puis_re_remise_sans_changement() {
    let mut ob = outbox();
    ob.enqueue(new_msg(1), T0).unwrap();

    let change = ob
        .mark_handed_off(&uuid(1), &RELAY, T0 + 10)
        .unwrap()
        .unwrap();
    assert_eq!(change.from, Some(Status::Queued));
    assert_eq!(change.to, Status::InFlight);
    assert_eq!(change.event_name(), "msg.handed_off");

    assert_eq!(ob.mark_handed_off(&uuid(1), &RELAY, T0 + 20).unwrap(), None);
    assert_eq!(ob.mark_handed_off(&uuid(1), &BOB, T0 + 30).unwrap(), None);

    let rec = ob.get(&uuid(1)).unwrap();
    assert_eq!(rec.status, Status::InFlight);
    assert_eq!(rec.status_ms, T0 + 10);
    assert_eq!(rec.first_sent_ms, Some(T0 + 10));
    assert_eq!(rec.last_sent_ms, Some(T0 + 30));
    assert_eq!(rec.attempts_for(&RELAY), 2);
    assert_eq!(rec.attempts_for(&BOB), 1);
}

#[test]
fn operations_sur_message_inconnu_sans_effet() {
    let mut ob = outbox();
    assert_eq!(ob.mark_handed_off(&uuid(9), &RELAY, T0).unwrap(), None);
    assert_eq!(
        ob.apply_ack(&uuid(9), AckStatus::Delivered, T0).unwrap(),
        None
    );
    assert_eq!(ob.cancel(&uuid(9), T0).unwrap(), None);
    assert!(ob.is_empty());
}

#[test]
fn ack_delivered_retire_le_message_et_les_doublons_sont_ignores() {
    let mut ob = outbox();
    ob.enqueue(new_msg(1), T0).unwrap();
    ob.mark_handed_off(&uuid(1), &RELAY, T0 + 1).unwrap();

    assert_eq!(
        ob.apply_ack(&uuid(1), AckStatus::Read, T0 + 2).unwrap(),
        None
    );
    let change = ob
        .apply_ack(&uuid(1), AckStatus::Delivered, T0 + 3)
        .unwrap()
        .unwrap();
    assert_eq!(change.from, Some(Status::InFlight));
    assert_eq!(change.to, Status::Delivered);
    assert!(ob.get(&uuid(1)).is_none());

    // Ack en double : ignoré.
    assert_eq!(
        ob.apply_ack(&uuid(1), AckStatus::Delivered, T0 + 4)
            .unwrap(),
        None
    );
    assert!(ob.into_store().is_empty());
}

#[test]
fn annulation_avant_hand_off_seulement() {
    let mut ob = outbox();
    ob.enqueue(new_msg(1), T0).unwrap();
    ob.enqueue(new_msg(2), T0).unwrap();
    ob.mark_handed_off(&uuid(2), &RELAY, T0 + 1).unwrap();

    let change = ob.cancel(&uuid(1), T0 + 2).unwrap().unwrap();
    assert_eq!(change.to, Status::Cancelled);
    assert!(ob.get(&uuid(1)).is_none());

    assert_eq!(ob.cancel(&uuid(2), T0 + 2).unwrap(), None);
    assert_eq!(ob.get(&uuid(2)).unwrap().status, Status::InFlight);
}

#[test]
fn expiration_apres_l_echeance_stricte() {
    let mut ob = outbox();
    ob.enqueue(new_msg(1), T0).unwrap();
    ob.enqueue(new_msg(2), T0 + 1000).unwrap();
    ob.mark_handed_off(&uuid(1), &RELAY, T0 + 1).unwrap();

    // `now == expires_ms` : pas encore expiré.
    assert!(ob.expire_due(T0 + TTL_MS).unwrap().is_empty());
    assert_eq!(ob.len(), 2);

    let changes = ob.expire_due(T0 + TTL_MS + 1).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].msg_uuid, uuid(1));
    assert_eq!(changes[0].from, Some(Status::InFlight));
    assert_eq!(changes[0].to, Status::Expired);
    assert_eq!(changes[0].event_name(), "msg.expired");

    // Un message jamais remis expire aussi (QUEUED -> EXPIRED).
    let changes = ob.expire_due(T0 + 1000 + TTL_MS + 1).unwrap();
    assert_eq!(changes[0].from, Some(Status::Queued));
    assert!(ob.is_empty());
}

// --- Rejeu -------------------------------------------------------------

#[test]
fn rejeu_plafonne_a_resend_max_par_pair() {
    let mut ob = outbox();
    ob.enqueue(new_msg(1), T0).unwrap();
    for i in 0..u64::from(RESEND_MAX) {
        assert_eq!(ob.replay_candidates(&RELAY, T0 + i).len(), 1);
        ob.mark_handed_off(&uuid(1), &RELAY, T0 + i).unwrap();
    }
    assert!(ob.replay_candidates(&RELAY, T0 + 100).is_empty());
    // Un autre pair reste un candidat.
    assert_eq!(ob.replay_candidates(&BOB, T0 + 100).len(), 1);
}

#[test]
fn rejeu_exclut_les_messages_expires() {
    let mut ob = outbox();
    ob.enqueue(new_msg(1), T0).unwrap();
    assert_eq!(ob.replay_candidates(&RELAY, T0 + TTL_MS).len(), 1);
    assert!(ob.replay_candidates(&RELAY, T0 + TTL_MS + 1).is_empty());
}

#[test]
fn table_des_pairs_bornee() {
    let mut ob = outbox();
    ob.enqueue(new_msg(1), T0).unwrap();
    for i in 0..=ATTEMPT_PEERS_MAX {
        let peer = [u8::try_from(i).unwrap(); 8];
        ob.mark_handed_off(&uuid(1), &peer, T0).unwrap();
    }
    let rec = ob.get(&uuid(1)).unwrap();
    assert_eq!(rec.attempts.len(), ATTEMPT_PEERS_MAX);
    // Pair déjà compté : toujours candidat ; pair nouveau : plus de place.
    assert!(rec.is_replayable_to(&[0; 8], T0));
    assert!(!rec.is_replayable_to(&[0xEE; 8], T0));
}

#[test]
fn compteur_de_remises_saturant() {
    let mut rec = OutboxRecord::decode(&sample_record().encode().unwrap()).unwrap();
    rec.attempts = vec![(RELAY, u8::MAX)];
    rec.count_attempt(&RELAY);
    assert_eq!(rec.attempts_for(&RELAY), u8::MAX);
}

#[test]
fn rejeu_apres_redemarrage() {
    let mut ob = outbox();
    ob.enqueue(new_msg(1), T0).unwrap(); // restera QUEUED
    ob.enqueue(new_msg(2), T0).unwrap(); // passera IN_FLIGHT
    ob.enqueue(new_msg(3), T0).unwrap(); // sera DELIVERED
    ob.mark_handed_off(&uuid(2), &RELAY, T0 + 5).unwrap();
    ob.mark_handed_off(&uuid(3), &RELAY, T0 + 5).unwrap();
    ob.apply_ack(&uuid(3), AckStatus::Delivered, T0 + 6)
        .unwrap();
    let avant: Vec<OutboxRecord> = ob.iter().cloned().collect();

    let ob = restart(ob);

    let apres: Vec<OutboxRecord> = ob.iter().cloned().collect();
    assert_eq!(apres, avant, "l'état non confirmé survit tel quel");
    let a_renvoyer: Vec<MsgUuid> = ob
        .replay_candidates(&BOB, T0 + 60_000)
        .iter()
        .map(|r| r.msg_uuid)
        .collect();
    assert_eq!(a_renvoyer, vec![uuid(1), uuid(2)]);
    assert_eq!(ob.get(&uuid(2)).unwrap().attempts_for(&RELAY), 1);
}

#[test]
fn ouverture_purge_les_enregistrements_terminaux() {
    let mut store = MemoryStore::new();
    let mut rec = sample_record();
    rec.status = Status::Delivered;
    store.save(&rec.msg_uuid, &rec.encode().unwrap()).unwrap();
    let vivant = sample_record_with(uuid(2));
    store
        .save(&vivant.msg_uuid, &vivant.encode().unwrap())
        .unwrap();

    let ob = Outbox::open(store).unwrap();
    assert_eq!(ob.len(), 1);
    assert_eq!(ob.into_store().len(), 1);
}

#[test]
fn ouverture_refuse_un_enregistrement_corrompu() {
    let mut store = MemoryStore::new();
    store.save(&uuid(1), &[RECORD_VERSION, 0, 1]).unwrap();
    assert_eq!(
        Outbox::open(store).unwrap_err(),
        OutboxError::Record(RecordError::Truncated)
    );
}

// --- Stockage en échec -------------------------------------------------

/// Stockage dont chaque opération peut être rendue défaillante.
#[derive(Debug, Default)]
struct FlakyStore {
    inner: MemoryStore,
    fail_save: bool,
    fail_remove: bool,
    fail_load: bool,
}

impl OutboxStore for FlakyStore {
    type Error = &'static str;

    fn save(&mut self, msg_uuid: &MsgUuid, record: &[u8]) -> Result<(), Self::Error> {
        if self.fail_save {
            return Err("save");
        }
        let Ok(()) = self.inner.save(msg_uuid, record);
        Ok(())
    }

    fn remove(&mut self, msg_uuid: &MsgUuid) -> Result<(), Self::Error> {
        if self.fail_remove {
            return Err("remove");
        }
        let Ok(()) = self.inner.remove(msg_uuid);
        Ok(())
    }

    fn load_all(&self) -> Result<Vec<Vec<u8>>, Self::Error> {
        if self.fail_load {
            return Err("load");
        }
        let Ok(all) = self.inner.load_all();
        Ok(all)
    }
}

#[test]
fn echec_d_ecriture_laisse_l_etat_inchange() {
    let mut ob = Outbox::open(FlakyStore::default()).unwrap();
    ob.enqueue(new_msg(1), T0).unwrap();
    ob.store.fail_save = true;
    ob.store.fail_remove = true;

    assert_eq!(ob.enqueue(new_msg(2), T0), Err(OutboxError::Store("save")));
    assert!(ob.get(&uuid(2)).is_none());

    assert_eq!(
        ob.mark_handed_off(&uuid(1), &RELAY, T0 + 1),
        Err(OutboxError::Store("save"))
    );
    let rec = ob.get(&uuid(1)).unwrap();
    assert_eq!(rec.status, Status::Queued);
    assert_eq!(rec.attempts_for(&RELAY), 0);

    assert_eq!(
        ob.cancel(&uuid(1), T0 + 2),
        Err(OutboxError::Store("remove"))
    );
    assert_eq!(ob.get(&uuid(1)).unwrap().status, Status::Queued);

    assert_eq!(
        ob.expire_due(T0 + TTL_MS + 1),
        Err(OutboxError::Store("remove"))
    );
    assert_eq!(ob.len(), 1);
}

#[test]
fn echecs_a_l_ouverture() {
    let store = FlakyStore {
        fail_load: true,
        ..FlakyStore::default()
    };
    assert_eq!(Outbox::open(store).unwrap_err(), OutboxError::Store("load"));

    let mut store = FlakyStore::default();
    let mut rec = sample_record();
    rec.status = Status::Expired;
    store.save(&rec.msg_uuid, &rec.encode().unwrap()).unwrap();
    store.fail_remove = true;
    assert_eq!(
        Outbox::open(store).unwrap_err(),
        OutboxError::Store("remove")
    );
}

#[test]
fn messages_d_erreur() {
    let e: OutboxError<&str> = OutboxError::Store("disque plein");
    assert_eq!(e.to_string(), "stockage de l'outbox : disque plein");
    let e: OutboxError<&str> = OutboxError::Record(RecordError::Truncated);
    assert_eq!(
        e.to_string(),
        "enregistrement d'outbox : enregistrement d'outbox tronqué"
    );
    for err in [
        RecordError::Truncated,
        RecordError::TrailingBytes,
        RecordError::UnknownVersion(9),
        RecordError::UnknownKind(9),
        RecordError::UnknownStatus(9),
        RecordError::InvalidFlag(9),
        RecordError::TooManyPeers,
        RecordError::PacketTooLarge,
    ] {
        assert!(!err.to_string().is_empty());
    }
}

// --- Format d'enregistrement ------------------------------------------------

fn sample_record_with(msg_uuid: MsgUuid) -> OutboxRecord {
    OutboxRecord {
        msg_uuid,
        dest_peer_id: BOB,
        kind: DeliveryKind::Session,
        packet: vec![0xAB; 12],
        status: Status::InFlight,
        status_ms: T0 + 1,
        first_sent_ms: Some(T0 + 1),
        last_sent_ms: None,
        expires_ms: T0 + TTL_MS,
        attempts: vec![(RELAY, 3)],
    }
}

fn sample_record() -> OutboxRecord {
    sample_record_with(uuid(1))
}

/// Offsets du format v1 (voir `OutboxRecord::encode`).
const OFF_KIND: usize = 1 + 16 + 8;
const OFF_STATUS: usize = OFF_KIND + 1;
const OFF_FIRST_SENT: usize = OFF_STATUS + 1 + 8;
const OFF_N_ATTEMPTS: usize = OFF_FIRST_SENT + 9 + 9 + 8;
const OFF_PACKET_LEN: usize = OFF_N_ATTEMPTS + 1 + 9;

#[test]
fn format_v1_taille_et_aller_retour() {
    let rec = sample_record();
    let bytes = rec.encode().unwrap();
    assert_eq!(bytes.len(), OFF_PACKET_LEN + 4 + 12);
    assert_eq!(bytes[0], RECORD_VERSION);
    assert_eq!(bytes[OFF_KIND], 0);
    assert_eq!(bytes[OFF_STATUS], Status::InFlight.to_u8());
    assert_eq!(OutboxRecord::decode(&bytes).unwrap(), rec);
}

#[test]
fn decodage_rejette_les_incoherences() {
    let good = sample_record().encode().unwrap();
    let with = |off: usize, v: u8| {
        let mut b = good.clone();
        b[off] = v;
        OutboxRecord::decode(&b)
    };
    assert_eq!(with(0, 2), Err(RecordError::UnknownVersion(2)));
    assert_eq!(with(OFF_KIND, 7), Err(RecordError::UnknownKind(7)));
    assert_eq!(with(OFF_STATUS, 7), Err(RecordError::UnknownStatus(7)));
    assert_eq!(with(OFF_FIRST_SENT, 2), Err(RecordError::InvalidFlag(2)));
    assert_eq!(
        with(OFF_N_ATTEMPTS, u8::try_from(ATTEMPT_PEERS_MAX + 1).unwrap()),
        Err(RecordError::TooManyPeers)
    );
    assert_eq!(with(OFF_PACKET_LEN, 0xFF), Err(RecordError::PacketTooLarge));

    let mut long = good.clone();
    long.push(0);
    assert_eq!(OutboxRecord::decode(&long), Err(RecordError::TrailingBytes));
    assert_eq!(
        OutboxRecord::decode(&good[..good.len() - 1]),
        Err(RecordError::Truncated)
    );
    assert_eq!(OutboxRecord::decode(&[]), Err(RecordError::Truncated));
}

#[test]
fn encodage_rejette_les_depassements() {
    let mut rec = sample_record();
    rec.attempts = vec![(RELAY, 1); ATTEMPT_PEERS_MAX + 1];
    assert_eq!(rec.encode(), Err(RecordError::TooManyPeers));
    let mut rec = sample_record();
    rec.packet = vec![0; PACKET_MAX_BYTES + 1];
    assert_eq!(rec.encode(), Err(RecordError::PacketTooLarge));
}

fn status() -> impl Strategy<Value = Status> {
    proptest::sample::select(Status::ALL.to_vec())
}

prop_compose! {
    fn record()(
        msg_uuid in any::<MsgUuid>(),
        dest_peer_id in any::<PeerId>(),
        envelope in any::<bool>(),
        packet in proptest::collection::vec(any::<u8>(), 0..300),
        status in status(),
        status_ms in any::<u64>(),
        first_sent_ms in any::<Option<u64>>(),
        last_sent_ms in any::<Option<u64>>(),
        expires_ms in any::<u64>(),
        attempts in proptest::collection::vec(any::<(PeerId, u8)>(), 0..=ATTEMPT_PEERS_MAX),
    ) -> OutboxRecord {
        OutboxRecord {
            msg_uuid,
            dest_peer_id,
            kind: if envelope { DeliveryKind::Envelope } else { DeliveryKind::Session },
            packet,
            status,
            status_ms,
            first_sent_ms,
            last_sent_ms,
            expires_ms,
            attempts,
        }
    }
}

/// Une opération sur l'outbox, pour le property test de monotonie.
#[derive(Debug, Clone)]
enum Op {
    HandOff(u8, u8),
    Ack(u8, bool),
    Cancel(u8),
    Expire(u64),
    Restart,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..4u8, 0..3u8).prop_map(|(m, p)| Op::HandOff(m, p)),
        (0..4u8, any::<bool>()).prop_map(|(m, read)| Op::Ack(m, read)),
        (0..4u8).prop_map(Op::Cancel),
        (0..2 * TTL_MS).prop_map(Op::Expire),
        Just(Op::Restart),
    ]
}

proptest! {
    #[test]
    fn aller_retour_de_l_encodage(rec in record()) {
        let bytes = rec.encode().unwrap();
        prop_assert_eq!(OutboxRecord::decode(&bytes).unwrap(), rec);
    }

    #[test]
    fn decodage_sans_panic(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
        let _ = OutboxRecord::decode(&bytes);
    }

    /// Monotonie au niveau de l'outbox, redémarrages compris : le statut
    /// observé d'un message ne redescend jamais, et un message terminé ne
    /// réapparaît pas.
    #[test]
    fn aucune_regression_d_etat_dans_l_outbox(ops in proptest::collection::vec(op(), 0..48)) {
        let mut ob = outbox();
        let mut vu: BTreeMap<MsgUuid, Status> = BTreeMap::new();
        for m in 0..4 {
            let change = ob.enqueue(new_msg(m), T0).unwrap().unwrap();
            vu.insert(change.msg_uuid, change.to);
        }
        let mut now = T0;
        for op in ops {
            now += 1;
            let changes = match op {
                Op::HandOff(m, p) => ob.mark_handed_off(&uuid(m), &[p; 8], now).unwrap().into_iter().collect(),
                Op::Ack(m, read) => {
                    let s = if read { AckStatus::Read } else { AckStatus::Delivered };
                    ob.apply_ack(&uuid(m), s, now).unwrap().into_iter().collect()
                }
                Op::Cancel(m) => ob.cancel(&uuid(m), now).unwrap().into_iter().collect(),
                Op::Expire(dt) => ob.expire_due(T0 + dt).unwrap(),
                Op::Restart => { ob = restart(ob); Vec::new() }
            };
            for c in changes {
                let avant = vu[&c.msg_uuid];
                prop_assert_eq!(c.from, Some(avant));
                prop_assert!(c.to.rank() > avant.rank(), "{:?} -> {:?}", avant, c.to);
                vu.insert(c.msg_uuid, c.to);
            }
            for (id, s) in &vu {
                match ob.get(id) {
                    Some(rec) => prop_assert_eq!(rec.status, *s),
                    None => prop_assert!(s.is_terminal(), "{:?} disparu en {:?}", id, s),
                }
            }
        }
    }
}
