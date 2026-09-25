// Fichier de test intégré : un `unwrap`/`expect` qui panique = échec de test,
// c'est le comportement voulu. (`allow-*-in-tests` du clippy.toml ne couvre que
// les modules `#[cfg(test)]`, pas les crates de `tests/`.)
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Contrôle des vecteurs de conformité v0 (`tests/vectors_v0.json`).
//!
//! US-108 ne livre **pas** de décodeur (`protocol::codec` = US-201). Ce test
//! vérifie donc la **cohérence structurelle** des vecteurs : chaque octet
//! `accept` doit correspondre à ses `expect` en s'appuyant sur
//! `dengon_core::protocol::{consts, types}`, et chaque octet `reject` doit
//! violer au moins une règle du format. US-201 branchera le vrai décodeur sur
//! ce même fichier ; le job `cross-vectors` (US-222) le partagera avec le
//! firmware et le dashboard.

use dengon_core::protocol::{
    consts::{HEADER_LEN_ADDRESSED, HEADER_LEN_BROADCAST, PROTO_VERSION, SIGNATURE_LEN},
    Flags, Header, PacketType,
};
use serde_json::Value;

const VECTORS_JSON: &str = include_str!("vectors_v0.json");

fn hex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "hex de longueur impaire");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex invalide"))
        .collect()
}

fn header_len(addressed: bool) -> usize {
    if addressed {
        HEADER_LEN_ADDRESSED
    } else {
        HEADER_LEN_BROADCAST
    }
}

/// Détecteur de rejet de référence (règles du format vérifiables sans décodeur).
fn is_rejected(raw: &[u8]) -> bool {
    if raw.len() < HEADER_LEN_BROADCAST {
        return true; // en-tête tronqué
    }
    if raw[0] != PROTO_VERSION {
        return true; // mauvaise version
    }
    if PacketType::from_u8(raw[1]).is_none() {
        return true; // type inconnu
    }
    // Pas de rejet sur bit réservé : synthese/05:80 les dit ignorés à la
    // réception (retour de revue #63, point de Paul — l'ancienne règle
    // contredisait la spec et aurait jeté tout le trafic v1.1 le jour où un
    // bit 5-7 serait attribué). `from_bits_truncate` les masque simplement.
    let flags = Flags::from_bits_truncate(raw[3]);
    let addressed = flags.contains(Flags::ADDRESSED);
    let hdr = header_len(addressed);
    // hdr inclut DÉJÀ les 2 octets de payload_len (voir
    // consts::tailles_den_tete_coherentes : 4 + 8 + PEER_ID_LEN + 2) : lire
    // raw[hdr-2..hdr] demande raw.len() >= hdr, pas hdr + 2. L'ancien garde
    // à `hdr + 2` rejetait à tort tout paquet valide avec payload_len ∈
    // {0, 1}, et rendait les vecteurs reject "bad-version"/"unknown-type"
    // (exactement `hdr` octets) attrapables par CETTE règle de longueur
    // plutôt que par celle qui les nomme (retour de revue #63, point de
    // Paul — vérifié en isolant chaque contrôle).
    if raw.len() < hdr {
        return true; // tronqué avant payload_len
    }
    let plen = u16::from_be_bytes([raw[hdr - 2], raw[hdr - 1]]) as usize;
    let expected = hdr
        + plen
        + if flags.contains(Flags::SIGNED) {
            SIGNATURE_LEN
        } else {
            0
        };
    raw.len() != expected // payload_len incohérent, signature absente/en trop
}

#[test]
fn accept_vectors_are_structurally_consistent() {
    let doc: Value = serde_json::from_str(VECTORS_JSON).expect("JSON invalide");
    let accept = doc["accept"].as_array().expect("champ accept");
    // 8, pas 7 : "reserved-flag-set" a rejoint accept (retour de revue #63,
    // point de Paul — un bit réservé posé est ignoré à la réception, pas un
    // motif de rejet, synthese/05:80).
    assert!(accept.len() >= 8, "au moins 8 vecteurs accept attendus");

    for v in accept {
        let name = v["name"].as_str().unwrap();
        let raw = hex(v["hex"].as_str().unwrap());
        let e = &v["expect"];

        assert_eq!(raw[0], PROTO_VERSION, "{name}: version");
        let pt = PacketType::from_u8(raw[1])
            .unwrap_or_else(|| panic!("{name}: type 0x{:02x} inconnu", raw[1]));
        assert_eq!(
            u64::from(pt.to_u8()),
            e["type"].as_u64().unwrap(),
            "{name}: type"
        );
        assert_eq!(
            pt.to_u8().to_string(),
            raw[1].to_string(),
            "{name}: type octet"
        );
        assert_eq!(u64::from(raw[2]), e["ttl"].as_u64().unwrap(), "{name}: ttl");

        // Le bit réservé brut n'est plus vérifié ici (retour de revue #63,
        // point de Paul) : un accept vector peut légitimement le porter
        // (voir "noise-msg-addressed-reserved-bit-ignored") puisque
        // synthese/05:80 dit ces bits ignorés, pas rejetés — c'est
        // `from_bits_truncate` juste en dessous qui les masque, et les
        // assertions sur `flags`/`addressed`/`signed`/`fragment` valident
        // que le masquage est correct.
        let flags = Flags::from_bits_truncate(raw[3]);
        assert_eq!(
            u64::from(flags.bits()),
            e["flags"].as_u64().unwrap(),
            "{name}: flags"
        );

        let addressed = flags.contains(Flags::ADDRESSED);
        let signed = flags.contains(Flags::SIGNED);
        let fragment = flags.contains(Flags::FRAGMENT);
        assert_eq!(
            addressed,
            e["addressed"].as_bool().unwrap(),
            "{name}: ADDRESSED"
        );
        assert_eq!(signed, e["signed"].as_bool().unwrap(), "{name}: SIGNED");
        assert_eq!(
            fragment,
            e["fragment"].as_bool().unwrap(),
            "{name}: FRAGMENT"
        );

        // Cohérence type ⇄ drapeaux. `is_addressed()` renvoie `None` pour
        // Fragment (hérite du paquet transporté, synthese/05:123) — plus
        // besoin de le court-circuiter ici avec un `if pt != Fragment`
        // (retour de revue #63, point de Paul).
        if let Some(expected_addressed) = pt.is_addressed() {
            assert_eq!(
                addressed, expected_addressed,
                "{name}: is_addressed() vs drapeau"
            );
        }
        if pt.is_always_signed() {
            assert!(signed, "{name}: {pt:?} doit être signé");
        }
        assert!(pt.is_mvp(), "{name}: type hors périmètre MVP");

        let hdr = header_len(addressed);
        assert!(raw.len() >= hdr, "{name}: en-tête tronqué");
        assert_eq!(
            &raw[12..20],
            hex(e["sender_id"].as_str().unwrap()).as_slice(),
            "{name}: sender_id"
        );
        match e["recipient_id"].as_str() {
            Some(r) => assert_eq!(&raw[20..28], hex(r).as_slice(), "{name}: recipient_id"),
            None => assert!(!addressed, "{name}: recipient_id null alors que ADDRESSED"),
        }

        let plen = u16::from_be_bytes([raw[hdr - 2], raw[hdr - 1]]);
        assert_eq!(
            u64::from(plen),
            e["payload_len"].as_u64().unwrap(),
            "{name}: payload_len"
        );
        let payload = &raw[hdr..hdr + plen as usize];
        assert_eq!(
            payload,
            hex(e["payload"].as_str().unwrap()).as_slice(),
            "{name}: payload"
        );

        let trailer = raw.len() - hdr - plen as usize;
        assert_eq!(
            trailer,
            if signed { SIGNATURE_LEN } else { 0 },
            "{name}: taille de la signature"
        );

        let header = Header {
            version: raw[0],
            packet_type: pt,
            ttl: raw[2],
            flags,
            timestamp_ms: u64::from_be_bytes(raw[4..12].try_into().unwrap()),
            sender_id: raw[12..20].try_into().unwrap(),
            recipient_id: if addressed {
                Some(raw[20..28].try_into().unwrap())
            } else {
                None
            },
            payload_len: plen,
        };
        assert!(header.flags_are_consistent(), "{name}: Header incohérent");
        assert_eq!(header.header_len(), hdr, "{name}: header_len()");
        assert_eq!(header.wire_len(), raw.len(), "{name}: wire_len()");

        assert!(
            !is_rejected(&raw),
            "{name}: un vecteur accept ne doit pas être rejeté"
        );
    }
}

#[test]
fn reject_vectors_each_violate_a_rule() {
    let doc: Value = serde_json::from_str(VECTORS_JSON).expect("JSON invalide");
    let reject = doc["reject"].as_array().expect("champ reject");
    assert!(reject.len() >= 5, "au moins 5 vecteurs reject attendus");

    for v in reject {
        let name = v["name"].as_str().unwrap();
        let raw = hex(v["hex"].as_str().unwrap());
        assert!(
            is_rejected(&raw),
            "{name}: devrait être rejeté, passe tous les contrôles v0 ({})",
            v["reject"].as_str().unwrap_or("?")
        );
    }
}

#[test]
fn accept_vectors_with_ttl_above_1_have_relay_ok() {
    // Retour de revue #63 round 2, point d'OswinFreyr : un paquet avec
    // `ttl > 1` mais sans `RELAY_OK` ne serait jamais relayé (synthese/05
    // §6.1 — règle de relais `RELAY_OK && ttl > 1`), et mourrait donc au
    // premier saut. Ce test empêche qu'un futur vecteur reproduise
    // l'incohérence trouvée sur "ack-addressed" (flags=1 avec ttl=7).
    let doc: Value = serde_json::from_str(VECTORS_JSON).expect("JSON invalide");
    let accept = doc["accept"].as_array().expect("champ accept");

    for v in accept {
        let name = v["name"].as_str().unwrap();
        let raw = hex(v["hex"].as_str().unwrap());
        let ttl = raw[2];
        let flags = Flags::from_bits_truncate(raw[3]);
        if ttl > 1 {
            assert!(
                flags.contains(Flags::RELAY_OK),
                "{name}: ttl={ttl} > 1 mais RELAY_OK absent — ce paquet mourrait au premier saut"
            );
        }
    }
}

#[test]
fn inventory_a_le_type_0x0d() {
    // AC US-108 : numéro attribué à INVENTORY.
    assert_eq!(PacketType::Inventory.to_u8(), 0x0D);
    let doc: Value = serde_json::from_str(VECTORS_JSON).unwrap();
    let inv = doc["accept"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "inventory-addressed-signed")
        .expect("vecteur inventory présent");
    assert_eq!(hex(inv["hex"].as_str().unwrap())[1], 0x0D);
}
