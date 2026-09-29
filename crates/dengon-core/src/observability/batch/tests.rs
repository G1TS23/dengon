use super::*;
use crate::crypto::VerifyingKey;
use crate::ledger::{GENESIS_HASH, SIG_LEN};
use alloc::vec;

/// Graine de `contracts/events/test-signing-key.json` (clé de test publique).
fn cle_de_test() -> SigningKey {
    let mut seed = [0u8; 32];
    seed[31] = 0x2a;
    SigningKey::from_seed(&seed)
}

fn entree(seq: u64, name: &str, payload: &str) -> Entry {
    Entry {
        seq,
        ts_ms: 1_725_800_000_000 + seq,
        event_name: String::from(name),
        payload_json: String::from(payload),
        prev_hash: GENESIS_HASH,
        entry_hash: GENESIS_HASH,
        sig: [0u8; SIG_LEN],
    }
}

#[test]
fn node_id_de_relais_conforme_au_schema() {
    let id = relay_node_id(&[0x3f, 0x2a, 0x9c, 0xff, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(id, "relay-3f2a9c");
}

#[test]
fn une_entree_du_journal_devient_une_enveloppe_recanonicalisee() {
    let e = entree(
        7,
        "relay.boot",
        r#"{"secure_boot":false, "fw_version":"0.1.0","reset_reason":"sw","flash_enc":false}"#,
    );
    let env = Envelope::from_ledger_entry(&e, "relay-3f2a9c", NodeKind::Relay).unwrap();
    assert_eq!(env.seq, 7);
    assert_eq!(env.ts_ms, e.ts_ms);
    assert_eq!(env.name, "relay.boot");
    // Même event_id que Envelope::new : seul node_id ‖ seq compte.
    let attendu = Envelope::new(
        "relay-3f2a9c",
        NodeKind::Relay,
        7,
        0,
        "relay.boot",
        Value::Bool(true),
    );
    assert_eq!(env.event_id, attendu.event_id);
    assert_eq!(
        env.payload.to_canonical_bytes(),
        br#"{"flash_enc":false,"fw_version":"0.1.0","reset_reason":"sw","secure_boot":false}"#
    );
}

#[test]
fn nom_hors_catalogue_ou_payload_invalide_refuses() {
    let hors = entree(1, "relay.inconnu", "{}");
    assert_eq!(
        Envelope::from_ledger_entry(&hors, "relay-000000", NodeKind::Relay),
        Err(BatchError::UnknownEvent)
    );
    for payload in ["[1]", "3", "{\"a\":1.5}", "{"] {
        let e = entree(1, "relay.health", payload);
        assert_eq!(
            Envelope::from_ledger_entry(&e, "relay-000000", NodeKind::Relay),
            Err(BatchError::BadPayload),
            "{payload}"
        );
    }
}

#[test]
fn batch_vide_refuse() {
    assert_eq!(
        signed_batch(&[], "relay-000000", &cle_de_test()),
        Err(BatchError::Empty)
    );
}

#[test]
fn le_sig_raccroche_egale_la_serialisation_complete() {
    let entries = [
        entree(1, "relay.wifi_up", r#"{"ssid":"ap","duration_s":0}"#),
        entree(2, "relay.wifi_down", r#"{"reason":"beacon_timeout"}"#),
    ];
    let corps =
        signed_batch_from_entries(&entries, "relay-abcdef", NodeKind::Relay, &cle_de_test())
            .unwrap();
    let texte = core::str::from_utf8(&corps).unwrap();
    let relu = canonical::parse(texte).unwrap();
    assert_eq!(relu.to_canonical_bytes(), corps);

    // Et la signature couvre bien `canonical_json(batch sans sig)`.
    let Value::Object(mut obj) = relu else {
        panic!("le batch doit être un objet")
    };
    let Some(Value::Str(sig_b64)) = obj.remove("sig") else {
        panic!("sig absent")
    };
    let sig: [u8; 64] = data_encoding::BASE64
        .decode(sig_b64.as_bytes())
        .unwrap()
        .try_into()
        .unwrap();
    let signe = Value::Object(obj).to_canonical_bytes();
    let pk: VerifyingKey = cle_de_test().verifying_key();
    assert!(pk.verify(&signe, &sig).is_ok());
}

#[test]
fn le_batch_id_couvre_les_evenements() {
    let a = [Envelope::from_ledger_entry(
        &entree(1, "relay.wifi_up", r#"{"ssid":"ap","duration_s":0}"#),
        "relay-abcdef",
        NodeKind::Relay,
    )
    .unwrap()];
    let mut b = a.clone();
    b[0].ts_ms += 1;
    assert_ne!(batch_id(&a), batch_id(&b));
    assert_eq!(batch_id(&a).len(), 64);
}

#[test]
fn trop_d_evenements_refuse() {
    let e = Envelope::new(
        "relay-000000",
        NodeKind::Relay,
        0,
        0,
        "relay.wifi_up",
        Value::Object(BTreeMap::new()),
    );
    let trop = vec![e; MAX_EVENTS + 1];
    assert_eq!(
        signed_batch(&trop, "relay-000000", &cle_de_test()),
        Err(BatchError::TooMany)
    );
}
