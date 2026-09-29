#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

//! L'API C du relais (US-308) exercée depuis Rust, avec les mêmes règles
//! d'appel que le firmware : handle opaque, sorties retirées une à une dans
//! un tampon fixe, curseur de journal persisté puis repassé au redémarrage.

use std::ptr;

use dengon_core::crypto::VerifyingKey;
use dengon_core::ledger::{verify_entries, verify_signatures, Anchor, Entry, Verdict};
use dengon_core_ffi::*;

fn now(ms: u64) -> Now {
    // Heure murale plausible (2025-09-01) + ms.
    Now {
        wall_ms: 1_756_684_800_000 + ms,
        mono_ms: ms,
    }
}

unsafe fn nouveau(n: u8, seq: u64, hash: Option<[u8; 32]>) -> *mut Relay {
    let dh = [n; 32];
    let seed = [n + 100; 32];
    let hash_ptr = hash.as_ref().map_or(ptr::null(), |h| h.as_ptr());
    let r = unsafe {
        dengon_relay_new(
            dh.as_ptr(),
            seed.as_ptr(),
            seq,
            hash_ptr,
            c"relais".as_ptr(),
            u64::from(n),
        )
    };
    assert!(!r.is_null());
    r
}

unsafe fn sorties(r: *mut Relay) -> Vec<(u64, Vec<u8>)> {
    let mut out = Vec::new();
    let mut buf = [0u8; 600];
    loop {
        let (mut link, mut len) = (0u64, 0usize);
        match unsafe {
            dengon_relay_pop_outgoing(r, &mut link, buf.as_mut_ptr(), buf.len(), &mut len)
        } {
            Status::Ok => out.push((link, buf[..len].to_vec())),
            Status::Empty => return out,
            s => panic!("pop_outgoing : {s:?}"),
        }
    }
}

unsafe fn journal(r: *mut Relay) -> Vec<u8> {
    let mut fichier = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        let mut len = 0usize;
        match unsafe { dengon_relay_pop_ledger(r, buf.as_mut_ptr(), buf.len(), &mut len) } {
            Status::Ok => fichier.extend_from_slice(&buf[..len]),
            Status::Empty => return fichier,
            s => panic!("pop_ledger : {s:?}"),
        }
    }
}

fn entrees(mut octets: &[u8]) -> Vec<Entry> {
    let mut out = Vec::new();
    while let Some((e, reste)) = Entry::from_bytes(octets) {
        out.push(e);
        octets = reste;
    }
    assert!(octets.is_empty(), "fin de fichier illisible");
    out
}

#[test]
fn deux_relais_se_lient_par_l_api_c() {
    unsafe {
        let (a, b) = (nouveau(1, 0, None), nouveau(2, 0, None));
        dengon_relay_link_up(a, 1, now(0));
        dengon_relay_link_up(b, 9, now(0));
        for _ in 0..3 {
            for (l, f) in sorties(a) {
                assert_eq!(l, 1);
                dengon_relay_on_frame(b, 9, f.as_ptr(), f.len(), now(1));
            }
            for (l, f) in sorties(b) {
                assert_eq!(l, 9);
                dengon_relay_on_frame(a, 1, f.as_ptr(), f.len(), now(1));
            }
        }
        let noms: Vec<String> = entrees(&journal(a))
            .into_iter()
            .map(|e| e.event_name)
            .collect();
        assert_eq!(noms, vec!["peer.announce_seen"]);

        let mut stats = RelayStats::default();
        assert_eq!(dengon_relay_stats(b, &mut stats), Status::Ok);
        assert_eq!(stats.unauthentic, 0);
        dengon_relay_link_down(a, 1);
        dengon_relay_free(a);
        dengon_relay_free(b);
    }
}

#[test]
fn tampon_trop_petit_ne_perd_pas_la_trame() {
    unsafe {
        let r = nouveau(1, 0, None);
        dengon_relay_link_up(r, 4, now(0));
        let (mut link, mut len) = (0u64, 0usize);
        let mut petit = [0u8; 8];
        assert_eq!(
            dengon_relay_pop_outgoing(r, &mut link, petit.as_mut_ptr(), petit.len(), &mut len),
            Status::BufferTooSmall
        );
        assert!(len > petit.len());
        let out = sorties(r);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].1.len(), len);
        dengon_relay_free(r);
    }
}

#[test]
fn journal_persiste_puis_repris_apres_redemarrage() {
    unsafe {
        let r = nouveau(3, 0, None);
        let mut cle = [0u8; 32];
        assert_eq!(dengon_relay_verifying_key(r, cle.as_mut_ptr()), Status::Ok);
        for i in 0..3 {
            assert!(dengon_relay_record_event(
                r,
                c"relay.boot".as_ptr(),
                c"{}".as_ptr(),
                now(i)
            ));
        }
        assert!(!dengon_relay_record_event(
            r,
            c"pas.au.catalogue".as_ptr(),
            c"{}".as_ptr(),
            now(3)
        ));
        let mut fichier = journal(r);
        let (mut seq, mut hash) = (0u64, [0u8; 32]);
        assert_eq!(
            dengon_relay_ledger_anchor(r, &mut seq, hash.as_mut_ptr()),
            Status::Ok
        );
        assert_eq!(seq, 3);
        dengon_relay_free(r);

        // `esp_restart()` : mêmes secrets, curseur relu de NVS.
        let r = nouveau(3, seq, Some(hash));
        assert!(dengon_relay_record_event(
            r,
            c"relay.boot".as_ptr(),
            c"{}".as_ptr(),
            now(10)
        ));
        fichier.extend(journal(r));
        dengon_relay_free(r);

        let tout = entrees(&fichier);
        assert_eq!(tout.len(), 4);
        assert_eq!(verify_entries(&tout, Anchor::GENESIS), Verdict::Ok);
        let cle = VerifyingKey::from_bytes(&cle).unwrap();
        assert_eq!(verify_signatures(&tout, &cle), None);
    }
}

#[test]
fn pointeurs_nuls_refuses_sans_crash() {
    unsafe {
        let s = [0u8; 32];
        assert!(
            dengon_relay_new(ptr::null(), s.as_ptr(), 0, ptr::null(), ptr::null(), 0).is_null()
        );
        dengon_relay_free(ptr::null_mut());
        dengon_relay_poll(ptr::null_mut(), now(0));
        dengon_relay_on_frame(ptr::null_mut(), 0, ptr::null(), 0, now(0));
        let mut len = 0usize;
        assert_eq!(
            dengon_relay_pop_ledger(ptr::null_mut(), ptr::null_mut(), 0, &mut len),
            Status::NullPointer
        );
        assert_eq!(dengon_noise_selftest(None), Status::NullPointer);

        // Trame vide ou hostile sur un vrai relais : rejetée, pas de panique.
        let r = nouveau(1, 0, None);
        dengon_relay_link_up(r, 1, now(0));
        dengon_relay_on_frame(r, 1, ptr::null(), 0, now(0));
        let hostile = [0xffu8; 64];
        dengon_relay_on_frame(r, 1, hostile.as_ptr(), hostile.len(), now(0));
        let mut stats = RelayStats::default();
        assert_eq!(dengon_relay_stats(r, &mut stats), Status::Ok);
        assert_eq!(stats.malformed, 2);
        dengon_relay_free(r);
    }
}

/// Aléa d'hôte (xorshift, pas cryptographique : suffit à prouver le câblage).
unsafe extern "C" fn alea_hote(buf: *mut u8, len: usize) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static ETAT: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);
    for i in 0..len {
        let mut x = ETAT.load(Ordering::Relaxed);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        ETAT.store(x, Ordering::Relaxed);
        unsafe { *buf.add(i) = x.to_le_bytes()[0] };
    }
}

unsafe extern "C" fn alea_constant(buf: *mut u8, len: usize) {
    unsafe { ptr::write_bytes(buf, 0x42, len) };
}

#[test]
fn autotest_noise_avec_l_alea_de_la_plateforme() {
    unsafe {
        assert_eq!(dengon_noise_selftest(Some(alea_hote)), Status::Ok);
        // Aléa constant : les deux paires de clés sont identiques, refusé.
        assert_eq!(dengon_noise_selftest(Some(alea_constant)), Status::Crypto);
    }
}
