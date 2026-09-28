//! Tests unitaires : arguments, lecture d'export, rapport JSON.

use dengon_core::ledger::{Ledger, NullSigner};

use super::*;

fn journal(n: u64) -> Vec<Entry> {
    let mut l = Ledger::new(NullSigner);
    for i in 0..n {
        l.append("pkt.relayed", "{\"hop\":1}", 1_700_000_000_000 + i);
    }
    l.entries().to_vec()
}

fn export(entries: &[Entry]) -> Vec<u8> {
    entries.iter().flat_map(Entry::to_bytes).collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn arguments_par_defaut() {
    let o = parse_args::<_, &str>([]).unwrap();
    assert_eq!(o.anchor, Anchor::GENESIS);
    assert_eq!(o.pubkey, None);
    assert_eq!(o.input, Input::Stdin);
    assert_eq!(parse_args(["-"]).unwrap().input, Input::Stdin);
    assert_eq!(
        parse_args(["j.bin"]).unwrap().input,
        Input::File("j.bin".to_owned())
    );
}

#[test]
fn arguments_complets() {
    let h = hex(&[0xAB; 32]);
    let o = parse_args(["--from-seq", "7", "--prev-hash", &h, "--pubkey", &h, "x"]).unwrap();
    assert_eq!(
        o.anchor,
        Anchor {
            first_seq: 7,
            prev_hash: [0xAB; 32]
        }
    );
    assert_eq!(o.pubkey, Some([0xAB; 32]));
}

#[test]
fn arguments_invalides() {
    let h = hex(&[0; 32]);
    for args in [
        vec!["--from-seq"],
        vec!["--from-seq", "x", "--prev-hash", &h],
        vec!["--from-seq", "1"],
        vec!["--prev-hash", &h],
        vec!["--prev-hash", "abcd"],
        vec!["--pubkey", &"zz".repeat(32)],
        vec!["--pubkey", &"é".repeat(32)],
        vec!["--verbose"],
        vec!["a", "b"],
        vec!["-", "-"],
    ] {
        let e = parse_args(&args).unwrap_err();
        assert_eq!(e.exit_code(), EXIT_USAGE, "{args:?}");
        assert!(e.to_string().contains(USAGE));
    }
}

#[test]
fn lecture_d_export() {
    let entries = journal(3);
    assert_eq!(parse_export(&export(&entries)).unwrap(), entries);
    assert_eq!(parse_export(&[]).unwrap(), Vec::new());
    let bytes = export(&entries);
    let e = parse_export(&bytes[..bytes.len() - 1]).unwrap_err();
    assert_eq!(e.exit_code(), EXIT_DATAERR);
    assert!(e.to_string().contains("entrée n°2"), "{e}");
}

#[test]
fn verdicts_et_codes() {
    for (v, code, name) in [
        (Verdict::Ok, 0, "ok"),
        (Verdict::Broken, 1, "broken"),
        (Verdict::Fork, 2, "fork"),
        (Verdict::Gap, 3, "gap"),
    ] {
        assert_eq!(exit_code(v), code);
        assert_eq!(verdict_name(v), name);
    }
    assert_eq!(Error::NoInput(String::new()).exit_code(), EXIT_NOINPUT);
}

#[test]
fn rapport_json() {
    let r = verify(&journal(3), Anchor::GENESIS, None).unwrap();
    assert_eq!(
        r.to_json(),
        "{\"verdict\":\"ok\",\"entries\":3,\"first_seq\":0,\"last_seq\":2,\"signatures\":\"unchecked\"}"
    );
    let r = verify(&[], Anchor::GENESIS, None).unwrap();
    assert_eq!(
        r.to_json(),
        "{\"verdict\":\"ok\",\"entries\":0,\"first_seq\":null,\"last_seq\":null,\"signatures\":\"unchecked\"}"
    );
}

#[test]
fn signatures_nulles_rejetees_avec_une_cle() {
    // Journal signé par `NullSigner` : avec une vraie clé, `broken`.
    let key = dengon_core::crypto::SigningKey::from_seed(&[1; 32])
        .verifying_key()
        .to_bytes();
    let r = verify(&journal(2), Anchor::GENESIS, Some(&key)).unwrap();
    assert_eq!(r.verdict, Verdict::Broken);
    assert_eq!(r.signatures, Some(false));
    assert!(r.to_json().contains("\"signatures\":\"invalid\""));
}

#[test]
fn cle_publique_invalide() {
    // y = 2 n'est l'ordonnée d'aucun point de la courbe (même cas que le
    // test `cle_publique_invalide_rejetee_au_decodage` de `crypto`).
    let mut bad = [0u8; 32];
    bad[0] = 2;
    let e = verify(&journal(1), Anchor::GENESIS, Some(&bad)).unwrap_err();
    assert_eq!(e.exit_code(), EXIT_USAGE);
}

#[test]
fn execution_depuis_stdin_et_fichier_absent() {
    let bytes = export(&journal(2));
    let r = run::<_, &str>([], bytes.as_slice()).unwrap();
    assert_eq!(r.verdict, Verdict::Ok);
    let e = run(["/nexiste/pas.bin"], std::io::empty()).unwrap_err();
    assert_eq!(e.exit_code(), EXIT_NOINPUT);
}
