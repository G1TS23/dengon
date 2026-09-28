// Fichier de test intégré : un `unwrap`/`expect` qui panique = échec de test,
// c'est le comportement voulu. (`allow-*-in-tests` du clippy.toml ne couvre que
// les modules `#[cfg(test)]`, pas les crates de `tests/`.)
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Vecteurs de conformité `crypto` v0 (US-204) : `vectors/crypto_v0.json`.
//!
//! Le fichier fixe, pour des clés et des graines connues : le padding, des
//! `recipient_tag`, un handshake Noise `XX` complet suivi de deux messages de
//! transport, et une enveloppe Noise `X`. L'aléa est un `ChaCha20Rng`
//! (`rand_chacha` 0.3) graine fixe. Une implémentation non-Rust (job
//! `cross-vectors`, US-222) retrouve la clé éphémère avec **n'importe quel
//! ChaCha20** : ce sont les 32 premiers octets du flux (clé = graine, nonce 0,
//! compteur 0) — vérifié avec Python `cryptography`. Les éphémères ne sont pas
//! écrits en clair dans le fichier : ce serait du matériel de clé privée, que
//! le scan de secrets (GitGuardian) signale à juste titre.
//!
//! - `vecteurs_conformes` recalcule tout et compare au fichier (régression) ;
//! - `vecteurs_rejouables` rejoue le fichier dans l'autre sens (déchiffrement,
//!   ouverture, dépadding) sans se fier au recalcul ;
//! - `generer_vecteurs` (`#[ignore]`) réécrit le fichier :
//!   `cargo test -p dengon-core --test crypto_vectors -- --ignored generer_vecteurs`.

use dengon_core::crypto::{
    open, pad, recipient_tag, seal, unpad, Handshake, StaticKeypair, PAD_BUCKETS,
};
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};
use serde_json::{json, Value};

const VECTORS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/crypto_v0.json");

const ALICE_SECRET: [u8; 32] = [0xA1; 32];
const BOB_SECRET: [u8; 32] = [0xB0; 32];
const SEED_INITIATOR: [u8; 32] = [0x01; 32];
const SEED_RESPONDER: [u8; 32] = [0x02; 32];
const SEED_SEAL: [u8; 32] = [0x03; 32];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "hex de longueur impaire");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex invalide"))
        .collect()
}

fn arr32(v: &Value) -> [u8; 32] {
    unhex(v.as_str().unwrap()).try_into().unwrap()
}

/// Premier secret éphémère tiré par `snow` : 32 octets du RNG (= flux
/// ChaCha20 standard, clé = graine, nonce 0).
fn first_ephemeral(seed: [u8; 32]) -> [u8; 32] {
    let mut e = [0u8; 32];
    ChaCha20Rng::from_seed(seed).fill_bytes(&mut e);
    e
}

/// Calcule l'ensemble des vecteurs.
fn compute() -> Value {
    let alice = StaticKeypair::from_secret(ALICE_SECRET);
    let bob = StaticKeypair::from_secret(BOB_SECRET);

    let padding: Vec<Value> = [0usize, 5, 200, 254, 255, 1023, 2046]
        .iter()
        .map(|&len| {
            let data: Vec<u8> = (0..len).map(|i| u8::try_from(i % 251).unwrap()).collect();
            let padded = pad(&data).unwrap();
            json!({ "input": hex(&data), "padded_len": padded.len(), "padded": hex(&padded) })
        })
        .collect();

    let tags: Vec<Value> = [
        (ALICE_SECRET, 0u16),
        (ALICE_SECRET, 20_000),
        (BOB_SECRET, 20_000),
    ]
    .iter()
    .map(|(secret, day)| {
        let pk = StaticKeypair::from_secret(*secret).public();
        json!({ "pub_static": hex(&pk), "epoch_day": day, "tag": hex(&recipient_tag(&pk, *day)) })
    })
    .collect();

    // Noise XX : Alice initie, Bob répond.
    let mut ia = Handshake::initiator(&alice, ChaCha20Rng::from_seed(SEED_INITIATOR)).unwrap();
    let mut rb = Handshake::responder(&bob, ChaCha20Rng::from_seed(SEED_RESPONDER)).unwrap();
    let p1: &[u8] = b"";
    let p2: &[u8] = b"hello from bob";
    let p3: &[u8] = b"";
    let m1 = ia.write_message(p1).unwrap();
    rb.read_message(&m1).unwrap();
    let m2 = rb.write_message(p2).unwrap();
    ia.read_message(&m2).unwrap();
    let m3 = ia.write_message(p3).unwrap();
    rb.read_message(&m3).unwrap();
    let mut sa = ia.into_session().unwrap();
    let mut sb = rb.into_session().unwrap();
    let t_ab: &[u8] = b"salut Bob";
    let t_ba: &[u8] = b"salut Alice, message un peu plus long";
    let c_ab = sa.encrypt(t_ab).unwrap();
    let c_ba = sb.encrypt(t_ba).unwrap();

    // Noise X : Alice scelle pour Bob.
    let sealed_pt: &[u8] = b"enveloppe pour Bob";
    let env = seal(
        &alice,
        &bob.public(),
        sealed_pt,
        ChaCha20Rng::from_seed(SEED_SEAL),
    )
    .unwrap();

    json!({
        "note": "Vecteurs de conformite v0 de dengon-core::crypto (US-204). Padding = len(u16 BE) || donnees || zeros jusqu'au bucket (ecart vs PKCS#7, docs/suivi/03-ecarts-conception.md). recipient_tag = HMAC-SHA256(pub_static, 'dengon-tag' || u32_be(epoch_day))[0..16]. Transport XX sans etat : ciphertext = nonce(u64 BE) || ChaCha20-Poly1305(padded), nonce Noise = ce compteur (fenetre anti-rejeu de 64). RNG = ChaCha20Rng (rand_chacha 0.3) graine fixe : la cle ephemere = 32 premiers octets du flux ChaCha20 standard (cle = rng_seed, nonce 0, compteur 0) ; non consignee en clair (pas de materiel de cle prive dans le depot).",
        "spec": "docs/synthese/06-securite.md §3 ; docs/synthese/05-protocole-et-trame.md",
        "pad_buckets": PAD_BUCKETS,
        "padding": padding,
        "recipient_tag": tags,
        "keys": {
            "alice_secret": hex(&ALICE_SECRET), "alice_public": hex(&alice.public()),
            "bob_secret": hex(&BOB_SECRET), "bob_public": hex(&bob.public()),
        },
        "noise_xx": {
            "protocol": "Noise_XX_25519_ChaChaPoly_SHA256",
            "prologue": "",
            "initiator": "alice", "responder": "bob",
            "initiator_rng_seed": hex(&SEED_INITIATOR),
            "responder_rng_seed": hex(&SEED_RESPONDER),
            "handshake": [
                { "payload": hex(p1), "message": hex(&m1) },
                { "payload": hex(p2), "message": hex(&m2) },
                { "payload": hex(p3), "message": hex(&m3) },
            ],
            "transport": [
                { "from": "alice", "nonce": 0, "plaintext": hex(t_ab), "ciphertext": hex(&c_ab) },
                { "from": "bob", "nonce": 0, "plaintext": hex(t_ba), "ciphertext": hex(&c_ba) },
            ],
        },
        "noise_x": {
            "protocol": "Noise_X_25519_ChaChaPoly_SHA256",
            "prologue": "",
            "sender": "alice", "recipient": "bob",
            "rng_seed": hex(&SEED_SEAL),
            "plaintext": hex(sealed_pt),
            "message": hex(&env),
        },
    })
}

fn load() -> Value {
    let raw = std::fs::read_to_string(VECTORS_PATH)
        .expect("vecteurs absents : lancer generer_vecteurs (voir en-tête)");
    serde_json::from_str(&raw).unwrap()
}

#[test]
fn vecteurs_conformes() {
    assert_eq!(
        compute(),
        load(),
        "sortie crypto différente des vecteurs versionnés — régression, ou \
         vecteurs à régénérer volontairement (et à justifier dans le journal)"
    );
}

#[test]
fn vecteurs_rejouables() {
    let v = load();

    // Padding : taille ∈ buckets, dépadding exact.
    for p in v["padding"].as_array().unwrap() {
        let padded = unhex(p["padded"].as_str().unwrap());
        assert!(PAD_BUCKETS.contains(&padded.len()));
        assert_eq!(unpad(&padded).unwrap(), unhex(p["input"].as_str().unwrap()));
    }

    // recipient_tag.
    for t in v["recipient_tag"].as_array().unwrap() {
        let day = u16::try_from(t["epoch_day"].as_u64().unwrap()).unwrap();
        assert_eq!(
            hex(&recipient_tag(&arr32(&t["pub_static"]), day)),
            t["tag"].as_str().unwrap()
        );
    }

    let keys = &v["keys"];
    let bob = StaticKeypair::from_secret(arr32(&keys["bob_secret"]));
    assert_eq!(hex(&bob.public()), keys["bob_public"].as_str().unwrap());

    // Noise XX : le premier message est exactement `e` de l'initiateur, en
    // clair (payload interdit, handshake non paddé) — l'éphémère consigné est
    // donc vérifiable indépendamment de snow.
    let xx = &v["noise_xx"];
    let m1 = unhex(xx["handshake"][0]["message"].as_str().unwrap());
    assert_eq!(m1.len(), 32);
    let e_i = StaticKeypair::from_secret(first_ephemeral(arr32(&xx["initiator_rng_seed"])));
    assert_eq!(&m1[..32], &e_i.public()[..]);

    // Bob rejoue le handshake reçu d'Alice (lecture seule des messages 1 et 3).
    let mut rb = Handshake::responder(
        &bob,
        ChaCha20Rng::from_seed(arr32(&xx["responder_rng_seed"])),
    )
    .unwrap();
    rb.read_message(&m1).unwrap();
    let m2 = rb
        .write_message(&unhex(xx["handshake"][1]["payload"].as_str().unwrap()))
        .unwrap();
    assert_eq!(hex(&m2), xx["handshake"][1]["message"].as_str().unwrap());
    rb.read_message(&unhex(xx["handshake"][2]["message"].as_str().unwrap()))
        .unwrap();
    let mut sb = rb.into_session().unwrap();
    assert_eq!(
        hex(&sb.remote_static()),
        keys["alice_public"].as_str().unwrap()
    );
    let t0 = &xx["transport"][0];
    let c0 = unhex(t0["ciphertext"].as_str().unwrap());
    assert_eq!(c0[..8], t0["nonce"].as_u64().unwrap().to_be_bytes());
    assert_eq!(
        sb.decrypt(&unhex(t0["ciphertext"].as_str().unwrap()))
            .unwrap(),
        unhex(t0["plaintext"].as_str().unwrap())
    );

    // Noise X : Bob ouvre l'enveloppe et identifie Alice.
    let x = &v["noise_x"];
    let env = unhex(x["message"].as_str().unwrap());
    let e_x = StaticKeypair::from_secret(first_ephemeral(arr32(&x["rng_seed"])));
    assert_eq!(&env[..32], &e_x.public()[..]);
    let opened = open(&bob, &env).unwrap();
    assert_eq!(opened.plaintext, unhex(x["plaintext"].as_str().unwrap()));
    assert_eq!(
        hex(&opened.sender_static),
        keys["alice_public"].as_str().unwrap()
    );
}

#[test]
#[ignore = "réécrit tests/vectors/crypto_v0.json"]
fn generer_vecteurs() {
    let mut out = serde_json::to_string_pretty(&compute()).unwrap();
    out.push('\n');
    std::fs::write(VECTORS_PATH, out).unwrap();
}
