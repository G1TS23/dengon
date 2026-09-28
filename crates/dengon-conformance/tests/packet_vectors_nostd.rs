// Fichier de test intégré : un `unwrap`/`expect` qui panique = échec de test.
// (`allow-*-in-tests` du clippy.toml ne couvre que les modules `#[cfg(test)]`.)
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Vecteurs `contracts/packet/vectors_v0.json` rejoués contre `dengon-core`
//! compilé **sans `std`** — la patte « firmware » du job `cross-vectors`
//! (US-222).
//!
//! `crates/dengon-core/tests/protocol_vectors.rs` fait le même travail, en
//! plus détaillé, mais contre le build `std`. Ce qui est vérifié **ici** et
//! nulle part ailleurs, c'est que le décodeur donne les mêmes décisions dans
//! la configuration que le firmware ESP32 embarquera (`no_std` + `alloc`,
//! ni `rusqlite` ni `getrandom`).
//!
//! Le firmware réel ne consomme pas encore `dengon-core` : le pont
//! `dengon_core_ffi` est l'US-307. C'est donc un **proxy**, consigné comme
//! écart dans `docs/suivi/03-ecarts-conception.md`.
//!
//! ⚠ Ces tests ne valent que lancés sur le paquet **seul**
//! (`cargo test -p dengon-conformance`) : dans un `cargo test --workspace`,
//! Cargo unifie les features et `dengon-core` récupère `std`. Voir la note
//! de `src/lib.rs`, et l'étape « le graphe sans std ne contient vraiment pas
//! rusqlite » du workflow `cross-vectors`, qui est le vrai garde-fou.

use dengon_core::protocol::{decode, encode, Flags};
use serde_json::Value;

const VECTORS_JSON: &str = include_str!("../../../contracts/packet/vectors_v0.json");

fn hex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "hex de longueur impaire");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex invalide"))
        .collect()
}

fn vecteurs() -> Value {
    serde_json::from_str(VECTORS_JSON).expect("contracts/packet/vectors_v0.json invalide")
}

#[test]
fn decode_accepte_les_vecteurs_accept_sans_std() {
    let doc = vecteurs();
    let accept = doc["accept"].as_array().expect("champ accept");
    assert!(accept.len() >= 8, "au moins 8 vecteurs accept attendus");

    for v in accept {
        let name = v["name"].as_str().unwrap();
        let raw = hex(v["hex"].as_str().unwrap());
        let e = &v["expect"];
        let p = decode(&raw).unwrap_or_else(|err| panic!("{name}: refusé sans std ({err})"));
        let h = &p.header;

        assert_eq!(
            u64::from(h.version),
            e["version"].as_u64().unwrap(),
            "{name}: version"
        );
        assert_eq!(
            u64::from(h.packet_type.to_u8()),
            e["type"].as_u64().unwrap(),
            "{name}: type"
        );
        assert_eq!(
            format!("{:?}", h.packet_type),
            e["type_name"].as_str().unwrap(),
            "{name}: type_name"
        );
        assert_eq!(u64::from(h.ttl), e["ttl"].as_u64().unwrap(), "{name}: ttl");
        assert_eq!(
            u64::from(h.flags.bits()),
            e["flags"].as_u64().unwrap(),
            "{name}: flags (bits réservés masqués)"
        );
        assert_eq!(
            h.flags.contains(Flags::ADDRESSED),
            e["addressed"].as_bool().unwrap(),
            "{name}: addressed"
        );
        assert_eq!(
            p.signature.is_some(),
            e["signed"].as_bool().unwrap(),
            "{name}: signed"
        );
        assert_eq!(
            h.flags.contains(Flags::FRAGMENT),
            e["fragment"].as_bool().unwrap(),
            "{name}: fragment"
        );
        assert_eq!(
            h.sender_id.as_slice(),
            hex(e["sender_id"].as_str().unwrap()).as_slice(),
            "{name}: sender_id"
        );
        assert_eq!(
            h.recipient_id.map(|r| r.to_vec()),
            e["recipient_id"].as_str().map(hex),
            "{name}: recipient_id"
        );
        assert_eq!(
            u64::from(h.payload_len),
            e["payload_len"].as_u64().unwrap(),
            "{name}: payload_len"
        );
        assert_eq!(
            p.payload,
            hex(e["payload"].as_str().unwrap()),
            "{name}: payload"
        );
    }
}

#[test]
fn encode_reproduit_les_vecteurs_accept_sans_std() {
    for v in vecteurs()["accept"].as_array().expect("champ accept") {
        let name = v["name"].as_str().unwrap();
        let raw = hex(v["hex"].as_str().unwrap());
        let again = encode(&decode(&raw).unwrap()).unwrap();
        // Seule différence admise : un bit réservé reçu est masqué au
        // décodage, donc absent du ré-encodage (synthese/05:80).
        let mut attendu = raw.clone();
        attendu[3] &= !Flags::RESERVED_MASK;
        assert_eq!(again, attendu, "{name}: ré-encodage sans std");
    }
}

#[test]
fn decode_refuse_les_vecteurs_reject_sans_std() {
    let doc = vecteurs();
    let reject = doc["reject"].as_array().expect("champ reject");
    assert!(reject.len() >= 5, "au moins 5 vecteurs reject attendus");

    for v in reject {
        let name = v["name"].as_str().unwrap();
        let raw = hex(v["hex"].as_str().unwrap());
        assert!(
            decode(&raw).is_err(),
            "{name}: accepté sans std alors qu'il viole « {} »",
            v["reject"].as_str().unwrap_or("?")
        );
    }
}
