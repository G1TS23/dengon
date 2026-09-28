// Fichier de test intégré : un `unwrap`/`expect` qui panique = échec de test.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Les **vraies** fixtures golden de `contracts/events/fixtures/` relues par
//! `dengon-core::observability` — patte Rust du job `cross-vectors` (US-222).
//!
//! # Pourquoi ce fichier existe
//!
//! `src/observability/mod.rs` contient déjà trois tests golden, mais ils
//! comparent à des octets **écrits en dur dans le test**. Ils documentent
//! bien le format, seulement ils ne relient rien : modifier une fixture ne
//! les fait pas broncher. Ici, les 20 fichiers sont lus depuis le disque,
//! donc une fixture qui bouge sans que le Rust suive casse le build — et
//! réciproquement.
//!
//! # Ce qui est vérifié, par fixture et par événement
//!
//! 1. `event_id = hex(SHA-256(node_id ‖ seq_be_u64))` recalculé par
//!    `Envelope::new` == celui du fichier (`CANONICAL.md` §3) ;
//! 2. la forme canonique produite par **notre** sérialiseur
//!    (`observability::canonical`) == celle produite par `serde_json`
//!    (clés triées par `serde_json::Map` = `BTreeMap`, séparateurs compacts,
//!    non-ASCII littéral) — deux sérialiseurs écrits séparément, mêmes
//!    octets, c'est tout l'intérêt ;
//! 3. `batch_id = hex(SHA-256(canonical_json(events)))` recalculé sur les
//!    octets de **notre** sérialiseur == celui du fichier ;
//! 4. la signature Ed25519 du batch, produite côté **Python**, est acceptée
//!    par `crypto::VerifyingKey::verify` côté **Rust** (`CANONICAL.md` §2).
//!
//! Le pendant Python de ces mêmes fichiers est `contracts/tools/validate.py`,
//! et le dashboard les rejoue via `POST /ingest/batch`
//! (`dashboard/api/tests/test_cross_vectors.py`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use dengon_core::crypto::{Signature, VerifyingKey, PUBLIC_KEY_LEN};
use dengon_core::observability::{canonical::Value as DValue, Envelope, NodeKind};
use sha2::{Digest, Sha256};

fn racine_contrats() -> PathBuf {
    // crates/dengon-core -> crates -> racine du monorepo -> contracts/events
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/events")
        .canonicalize()
        .expect("contracts/events introuvable")
}

/// Les 20 fixtures, triées par nom pour que l'ordre des échecs soit stable.
fn fixtures() -> Vec<(String, serde_json::Value)> {
    let dir = racine_contrats().join("fixtures");
    let mut fichiers: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("contracts/events/fixtures introuvable")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    fichiers.sort();
    assert!(
        fichiers.len() >= 20,
        "au moins 20 fixtures golden attendues, {} trouvées",
        fichiers.len()
    );
    fichiers
        .into_iter()
        .map(|p| {
            let nom = p.file_name().unwrap().to_string_lossy().into_owned();
            let brut = std::fs::read_to_string(&p).unwrap();
            (
                nom,
                serde_json::from_str(&brut).expect("fixture JSON invalide"),
            )
        })
        .collect()
}

fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex invalide"))
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `serde_json::Value` -> `observability::canonical::Value`.
///
/// Refuse explicitement les flottants et `null` : `CANONICAL.md` §1 les
/// interdit dans le contrat, et notre `Value` ne sait même pas les
/// représenter. Une fixture qui en contiendrait ferait échouer **ici**,
/// avec un message qui nomme le champ — plutôt que plus loin, sur une
/// différence d'octets illisible.
fn vers_valeur_dengon(v: &serde_json::Value, chemin: &str) -> DValue {
    match v {
        serde_json::Value::Bool(b) => DValue::Bool(*b),
        serde_json::Value::Number(n) => DValue::Int(
            n.as_i64()
                .unwrap_or_else(|| panic!("{chemin}: nombre non entier ({n}) — CANONICAL.md §1")),
        ),
        serde_json::Value::String(s) => DValue::Str(s.clone()),
        serde_json::Value::Array(items) => DValue::Array(
            items
                .iter()
                .enumerate()
                .map(|(i, it)| vers_valeur_dengon(it, &format!("{chemin}[{i}]")))
                .collect(),
        ),
        serde_json::Value::Object(map) => {
            let mut out = BTreeMap::new();
            for (k, val) in map {
                out.insert(k.clone(), vers_valeur_dengon(val, &format!("{chemin}.{k}")));
            }
            DValue::Object(out)
        }
        serde_json::Value::Null => panic!("{chemin}: null interdit par le contrat"),
    }
}

fn enveloppe_depuis_fixture(ev: &serde_json::Value, chemin: &str) -> Envelope {
    let node_kind = match ev["node_kind"].as_str().unwrap() {
        "relay" => NodeKind::Relay,
        "client" => NodeKind::Client,
        autre => panic!("{chemin}: node_kind inconnu « {autre} »"),
    };
    // `Envelope::name` est `&'static str` (les noms viennent du catalogue,
    // constants à la compilation) ; ici ils sortent d'un fichier lu à
    // l'exécution. Fuiter la chaîne est sans conséquence dans un test — le
    // processus s'arrête à la fin — et évite de tordre l'API de production
    // pour les besoins d'un test.
    let name: &'static str = Box::leak(ev["name"].as_str().unwrap().to_owned().into_boxed_str());
    Envelope::new(
        ev["node_id"].as_str().unwrap(),
        node_kind,
        ev["seq"].as_u64().unwrap(),
        ev["ts_ms"].as_u64().unwrap(),
        name,
        vers_valeur_dengon(&ev["payload"], &format!("{chemin}.payload")),
    )
}

#[test]
fn chaque_event_id_des_fixtures_est_recalculable() {
    for (nom, batch) in fixtures() {
        for (i, ev) in batch["events"].as_array().unwrap().iter().enumerate() {
            let chemin = format!("{nom}[{i}]");
            let env = enveloppe_depuis_fixture(ev, &chemin);
            assert_eq!(
                env.event_id,
                ev["event_id"].as_str().unwrap(),
                "{chemin}: event_id recalculé != event_id de la fixture"
            );
        }
    }
}

#[test]
fn notre_json_canonique_est_octet_pour_octet_celui_des_fixtures() {
    for (nom, batch) in fixtures() {
        for (i, ev) in batch["events"].as_array().unwrap().iter().enumerate() {
            let chemin = format!("{nom}[{i}]");
            let a_nous = enveloppe_depuis_fixture(ev, &chemin).to_canonical_bytes();
            // Sérialiseur de référence indépendant : `serde_json::Map` est un
            // `BTreeMap` (clés triées), `to_vec` écrit sans espace et laisse
            // l'UTF-8 littéral — exactement `CANONICAL.md` §1.
            let de_reference = serde_json::to_vec(ev).unwrap();
            assert_eq!(
                String::from_utf8_lossy(&a_nous),
                String::from_utf8_lossy(&de_reference),
                "{chemin}: observability::canonical diverge de la fixture"
            );
        }
    }
}

#[test]
fn le_batch_id_des_fixtures_se_recalcule_depuis_nos_octets() {
    for (nom, batch) in fixtures() {
        // canonical_json(events) reconstruit à partir de NOS enveloppes, pas
        // du texte du fichier : c'est notre sérialiseur qui est mis à
        // l'épreuve, pas la capacité de serde_json à relire un fichier.
        let evenements: Vec<DValue> = batch["events"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, ev)| {
                vers_valeur_dengon(
                    &serde_json::from_slice::<serde_json::Value>(
                        &enveloppe_depuis_fixture(ev, &format!("{nom}[{i}]")).to_canonical_bytes(),
                    )
                    .unwrap(),
                    &format!("{nom}[{i}]"),
                )
            })
            .collect();
        let octets = DValue::Array(evenements).to_canonical_bytes();
        assert_eq!(
            hex_encode(&Sha256::digest(&octets)),
            batch["batch_id"].as_str().unwrap(),
            "{nom}: batch_id recalculé != batch_id de la fixture"
        );
    }
}

#[test]
fn rust_verifie_les_signatures_produites_par_python() {
    let cle: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(racine_contrats().join("test-signing-key.json")).unwrap(),
    )
    .unwrap();
    let pub_bytes: [u8; PUBLIC_KEY_LEN] = hex_decode(cle["public_hex"].as_str().unwrap())
        .try_into()
        .expect("clé publique de 32 octets");
    let verif = VerifyingKey::from_bytes(&pub_bytes).expect("clé publique valide");

    for (nom, batch) in fixtures() {
        // `CANONICAL.md` §2 : le message signé est le batch SANS `sig`.
        let mut sans_sig = batch.clone();
        sans_sig.as_object_mut().unwrap().remove("sig");
        let message = serde_json::to_vec(&sans_sig).unwrap();

        let sig_bytes = base64_decode(batch["sig"].as_str().unwrap());
        let sig: Signature = sig_bytes.try_into().expect("signature de 64 octets");

        verif
            .verify(&message, &sig)
            .unwrap_or_else(|e| panic!("{nom}: signature du batch refusée par le Rust ({e})"));
    }
}

#[test]
fn une_fixture_alteree_est_refusee() {
    // Sans ce test, rien ne prouve que le précédent détecte quoi que ce soit.
    let cle: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(racine_contrats().join("test-signing-key.json")).unwrap(),
    )
    .unwrap();
    let pub_bytes: [u8; PUBLIC_KEY_LEN] = hex_decode(cle["public_hex"].as_str().unwrap())
        .try_into()
        .unwrap();
    let verif = VerifyingKey::from_bytes(&pub_bytes).unwrap();

    let (nom, batch) = fixtures().into_iter().next().unwrap();
    let mut altere = batch.clone();
    {
        let obj = altere.as_object_mut().unwrap();
        obj.remove("sig");
        // Un seul champ change : `seq` du premier événement.
        obj["events"][0]["seq"] = serde_json::json!(999_999);
    }
    let sig: Signature = base64_decode(batch["sig"].as_str().unwrap())
        .try_into()
        .unwrap();
    assert!(
        verif
            .verify(&serde_json::to_vec(&altere).unwrap(), &sig)
            .is_err(),
        "{nom}: une fixture altérée passe la vérification de signature"
    );
}

/// base64 standard, avec padding. Écrit ici plutôt que tiré d'une crate :
/// `data-encoding` est une dépendance de `dengon-core`, pas une
/// dev-dependency, et ajouter une dépendance pour 15 lignes de décodage dans
/// un test n'en vaut pas le coût de surface.
fn base64_decode(s: &str) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut acc: u32 = 0;
    let mut bits = 0_u32;
    let mut out = Vec::new();
    for c in s.bytes().filter(|&c| c != b'=') {
        let v = ALPHABET
            .iter()
            .position(|&a| a == c)
            .unwrap_or_else(|| panic!("caractère base64 invalide : {c:?}"));
        acc = (acc << 6) | u32::try_from(v).unwrap();
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xFF).unwrap());
        }
    }
    out
}
