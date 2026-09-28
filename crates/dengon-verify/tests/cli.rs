//! Tests de bout en bout du binaire `dengon-verify` (US-305) : **un journal
//! fabriqué exprès par verdict**, passé au vrai exécutable, qui doit rendre
//! le bon verdict JSON **et** le bon code de sortie (contrat avec le
//! dashboard, US-310).
//!
//! Les journaux sont aussi écrits dans `tests/fixtures/` (déterministes :
//! signeur nul ou graine fixe, horodatages fixes) pour que le dashboard et la
//! démo de soutenance puissent les réutiliser. `fixtures_a_jour` vérifie que
//! les fichiers commités correspondent au générateur ; pour les régénérer :
//! `DENGON_REGEN_FIXTURES=1 cargo test -p dengon-verify --test cli`.

// Même convention que les autres tests d'intégration du workspace : clippy
// n'étend `allow-unwrap-in-tests` qu'aux `#[test]`, pas à leurs fonctions
// utilitaires.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use dengon_core::crypto::SigningKey;
use dengon_core::ledger::{Entry, Ledger, NullSigner, Signature, Signer};

const T0: u64 = 1_725_800_000_000;
const SEED: [u8; 32] = [0x2A; 32];

struct Ed25519(SigningKey);

impl Signer for Ed25519 {
    fn sign(&mut self, message: &[u8]) -> Signature {
        self.0.sign(message)
    }
}

fn build<S: Signer>(signer: S, n: u64) -> Vec<Entry> {
    let mut l = Ledger::new(signer);
    for i in 0..n {
        let payload = format!("{{\"hop\":{i}}}");
        l.append("pkt.relayed", &payload, T0 + i * 1000);
    }
    l.entries().to_vec()
}

/// Les journaux de démonstration, un par verdict (+ signé).
fn fixtures() -> Vec<(&'static str, Vec<Entry>)> {
    let ok = build(NullSigner, 5);

    // broken : contenu de l'entrée 2 modifié après coup.
    let mut broken = ok.clone();
    broken[2].payload_json = "{\"hop\":99}".to_owned();

    // fork : une seconde entrée revendique la position 2.
    let mut fork = ok.clone();
    let mut rival = ok[2].clone();
    rival.payload_json = "{\"hop\":42}".to_owned();
    fork.push(rival);

    // gap : l'entrée 2 a disparu.
    let mut gap = ok.clone();
    gap.remove(2);

    let signed = build(Ed25519(SigningKey::from_seed(&SEED)), 5);

    vec![
        ("ok", ok),
        ("broken", broken),
        ("fork", fork),
        ("gap", gap),
        ("signed", signed),
    ]
}

fn fixture(name: &str) -> Vec<Entry> {
    fixtures()
        .into_iter()
        .find(|(n, _)| *n == name)
        .map(|(_, e)| e)
        .unwrap()
}

fn export(entries: &[Entry]) -> Vec<u8> {
    entries.iter().flat_map(Entry::to_bytes).collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Lance le binaire avec `args`, l'export étant passé sur l'entrée standard.
fn run_stdin(args: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dengon-verify"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

fn run_file(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dengon-verify"))
        .args(args)
        .output()
        .unwrap()
}

fn assert_verdict(out: &Output, verdict: &str, code: i32) {
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(code), "stdout={stdout}");
    assert!(
        stdout.starts_with(&format!("{{\"verdict\":\"{verdict}\"")),
        "stdout={stdout}"
    );
}

// --- Un cas par verdict ---------------------------------------------------------

#[test]
fn verdict_ok() {
    let out = run_stdin(&[], &export(&fixture("ok")));
    assert_verdict(&out, "ok", 0);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "{\"verdict\":\"ok\",\"entries\":5,\"first_seq\":0,\"last_seq\":4,\"signatures\":\"unchecked\"}"
    );
}

#[test]
fn verdict_broken() {
    assert_verdict(&run_stdin(&["-"], &export(&fixture("broken"))), "broken", 1);
}

#[test]
fn verdict_fork() {
    assert_verdict(&run_stdin(&[], &export(&fixture("fork"))), "fork", 2);
}

#[test]
fn verdict_gap() {
    assert_verdict(&run_stdin(&[], &export(&fixture("gap"))), "gap", 3);
}

// --- Signatures -------------------------------------------------------------------

#[test]
fn signatures_verifiees_avec_la_bonne_cle() {
    let key = hex(&SigningKey::from_seed(&SEED).verifying_key().to_bytes());
    let out = run_stdin(&["--pubkey", &key], &export(&fixture("signed")));
    assert_verdict(&out, "ok", 0);
    assert!(String::from_utf8_lossy(&out.stdout).contains("\"signatures\":\"verified\""));

    // Clé d'un autre nœud : les signatures ne correspondent pas.
    let other = hex(&SigningKey::from_seed(&[1; 32]).verifying_key().to_bytes());
    let out = run_stdin(&["--pubkey", &other], &export(&fixture("signed")));
    assert_verdict(&out, "broken", 1);
    assert!(String::from_utf8_lossy(&out.stdout).contains("\"signatures\":\"invalid\""));
}

// --- Tranche ancrée -----------------------------------------------------------------

#[test]
fn tranche_ancree() {
    let ok = fixture("ok");
    let slice = export(&ok[3..]);
    let prev = hex(&ok[2].entry_hash);
    // Sans ancre : la tranche ne commence pas à 0.
    assert_verdict(&run_stdin(&[], &slice), "gap", 3);
    // Ancrée sur l'entrée 2 : intègre.
    let out = run_stdin(&["--from-seq", "3", "--prev-hash", &prev], &slice);
    assert_verdict(&out, "ok", 0);
    assert!(String::from_utf8_lossy(&out.stdout).contains("\"first_seq\":3,\"last_seq\":4"));
    // Mauvais prev_hash.
    let wrong = hex(&ok[1].entry_hash);
    let out = run_stdin(&["--from-seq", "3", "--prev-hash", &wrong], &slice);
    assert_verdict(&out, "broken", 1);
}

// --- Erreurs : codes sysexits, rien sur stdout ----------------------------------------

#[test]
fn erreurs_d_appel_et_d_entree() {
    let out = run_file(&["--inconnue"]);
    assert_eq!(out.status.code(), Some(64));
    assert!(out.stdout.is_empty());
    assert!(!out.stderr.is_empty());

    let bytes = export(&fixture("ok"));
    let out = run_stdin(&[], &bytes[..bytes.len() - 3]);
    assert_eq!(out.status.code(), Some(65));
    assert!(out.stdout.is_empty());

    let out = run_file(&["/chemin/qui/n/existe/pas.bin"]);
    assert_eq!(out.status.code(), Some(66));
    assert!(out.stdout.is_empty());
}

// --- Fixtures commitées -----------------------------------------------------------------

#[test]
fn fixtures_a_jour() {
    let dir = fixtures_dir();
    let regen = std::env::var_os("DENGON_REGEN_FIXTURES").is_some();
    for (name, entries) in fixtures() {
        let path = dir.join(format!("{name}.bin"));
        let bytes = export(&entries);
        if regen {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, &bytes).unwrap();
        }
        let committed = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("{}: {e} (régénérer, voir l'en-tête)", path.display()));
        assert_eq!(
            committed, bytes,
            "{name}.bin périmé (régénérer, voir l'en-tête)"
        );
    }
}

#[test]
fn fixtures_lues_depuis_un_fichier() {
    for (name, code) in [("ok", 0), ("broken", 1), ("fork", 2), ("gap", 3)] {
        let path = fixtures_dir().join(format!("{name}.bin"));
        let out = run_file(&[path.to_str().unwrap()]);
        assert_verdict(&out, name, code);
    }
}
