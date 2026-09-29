#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

//! Vecteurs `contracts/packet/vectors_v0.json` rejoués à travers la frontière
//! C de `dengon-core-ffi` (US-307), en appelant [`dengon_decode_reencode`]
//! exactement comme le fera `tests/c/host_test.c` — mêmes règles d'appel,
//! mêmes tampons, mais depuis Rust pour un retour rapide sous
//! `cargo test`. Le programme C, lui, est la preuve **littérale** de
//! l'exigence « un programme C hôte lie la bibliothèque » : ce fichier n'en
//! est pas un substitut, c'est un filet supplémentaire, plus rapide à
//! exécuter en boucle pendant le développement.
//!
//! Ce test est un fichier d'intégration (pas `#[cfg(test)] mod tests` dans
//! `src/lib.rs`) précisément pour pouvoir utiliser `std` (`serde_json`, ce
//! fichier) alors que la bibliothèque elle-même reste `#![no_std]` — même
//! raison que `dengon-conformance/tests/packet_vectors_nostd.rs`.

use dengon_core_ffi::{dengon_decode_reencode, dengon_protocol_version, Status};
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

/// Appelle `dengon_decode_reencode` avec un tampon de sortie large.
fn decode_reencode(raw: &[u8]) -> (Status, Vec<u8>) {
    let mut out = vec![0u8; 4096];
    let mut out_len: usize = 0;
    // SAFETY : `raw` est un slice Rust valide, `out` a bien `out.len()`
    // octets, `out_len` est une variable locale valide.
    let status = unsafe {
        dengon_decode_reencode(
            raw.as_ptr(),
            raw.len(),
            out.as_mut_ptr(),
            out.len(),
            &mut out_len,
        )
    };
    out.truncate(out_len);
    (status, out)
}

#[test]
fn la_version_de_protocole_est_exposee() {
    assert_eq!(dengon_protocol_version(), dengon_core::PROTOCOL_VERSION);
}

#[test]
fn encode_reproduit_les_vecteurs_accept_via_la_frontiere_c() {
    for v in vecteurs()["accept"].as_array().expect("champ accept") {
        let name = v["name"].as_str().unwrap();
        let raw = hex(v["hex"].as_str().unwrap());
        let (status, again) = decode_reencode(&raw);
        assert_eq!(status, Status::Ok, "{name}: refusé via la frontière C");
        // Seule différence admise : un bit réservé reçu est masqué au
        // décodage (synthese/05:80), comme côté dengon-conformance.
        let mut attendu = raw.clone();
        attendu[3] &= !dengon_core::protocol::Flags::RESERVED_MASK;
        assert_eq!(again, attendu, "{name}: ré-encodage via la frontière C");
    }
}

#[test]
fn decode_refuse_les_vecteurs_reject_via_la_frontiere_c() {
    let doc = vecteurs();
    let reject = doc["reject"].as_array().expect("champ reject");
    assert!(reject.len() >= 5, "au moins 5 vecteurs reject attendus");

    for v in reject {
        let name = v["name"].as_str().unwrap();
        let raw = hex(v["hex"].as_str().unwrap());
        let (status, _) = decode_reencode(&raw);
        assert_eq!(
            status,
            Status::Decode,
            "{name}: accepté via la frontière C alors qu'il viole « {} »",
            v["reject"].as_str().unwrap_or("?")
        );
    }
}

#[test]
fn tampon_de_sortie_trop_petit_est_signale() {
    let raw = hex(vecteurs()["accept"].as_array().unwrap()[0]["hex"]
        .as_str()
        .unwrap());
    let mut out = [0u8; 1];
    let mut out_len: usize = 0;
    // SAFETY : mêmes garanties que `decode_reencode`, tampon volontairement
    // trop petit pour exercer `Status::BufferTooSmall`.
    let status = unsafe {
        dengon_decode_reencode(
            raw.as_ptr(),
            raw.len(),
            out.as_mut_ptr(),
            out.len(),
            &mut out_len,
        )
    };
    assert_eq!(status, Status::BufferTooSmall);
}

#[test]
fn pointeur_de_sortie_nul_est_signale() {
    let raw = hex(vecteurs()["accept"].as_array().unwrap()[0]["hex"]
        .as_str()
        .unwrap());
    // SAFETY : `output_len` nul est précisément ce que ce test vérifie ;
    // la fonction doit le détecter avant tout déréférencement.
    let status = unsafe {
        dengon_decode_reencode(
            raw.as_ptr(),
            raw.len(),
            core::ptr::null_mut(),
            0,
            core::ptr::null_mut(),
        )
    };
    assert_eq!(status, Status::NullPointer);
}

#[test]
fn entree_vide_est_geree_sans_pointeur_non_nul() {
    // `input_len == 0` avec `input` nul est un appel valide (pas de lecture),
    // et doit être refusé par `decode` comme trop court plutôt que
    // provoquer un déréférencement.
    let mut out = [0u8; 8];
    let mut out_len: usize = 0;
    // SAFETY : `input` nul avec `input_len == 0` est explicitement autorisé
    // par le contrat de `dengon_decode_reencode`.
    let status = unsafe {
        dengon_decode_reencode(
            core::ptr::null(),
            0,
            out.as_mut_ptr(),
            out.len(),
            &mut out_len,
        )
    };
    assert_eq!(status, Status::Decode);
}
