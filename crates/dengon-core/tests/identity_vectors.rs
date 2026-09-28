// Fichier de test intégré : un `unwrap`/`expect` qui panique = échec de test,
// c'est le comportement voulu. (`allow-*-in-tests` du clippy.toml ne couvre que
// les modules `#[cfg(test)]`, pas les crates de `tests/`.)
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Vecteurs de conformité `identity` v0 (US-205) : `contracts/packet/identity_v0.json`.
//!
//! Pour deux identités dérivées d'un `ChaCha20Rng` (`rand_chacha` 0.3) à
//! graine fixe, le fichier fixe : clés publiques, `peerID` (hex et base32),
//! empreinte, chaîne QR, et le code de vérification de 60 chiffres entre les
//! deux. Une implémentation non-Rust (job `cross-vectors`) retrouve les
//! secrets avec **n'importe quel ChaCha20** (clé = graine, nonce 0,
//! compteur 0) : octets 0..32 du flux = secret X25519, octets 32..64 = graine
//! Ed25519. Les secrets ne sont pas écrits en clair (scan de secrets).
//!
//! - `vecteurs_conformes` recalcule tout et compare au fichier (régression) ;
//! - `vecteurs_rejouables` décode les QR du fichier et recalcule le code ;
//! - `appairage_a_et_b` : le scénario de l'écran d'appairage (US-215) ;
//! - `generer_vecteurs` (`#[ignore]`) réécrit le fichier :
//!   `cargo test -p dengon-core --test identity_vectors -- --ignored generer_vecteurs`.

use dengon_core::identity::{
    load_or_create, peer_id_base32, verification_code, Identity, MemoryVault, PublicIdentity,
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use serde_json::{json, Value};

const VECTORS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../contracts/packet/identity_v0.json"
);

const SEED_ALICE: [u8; 32] = [0x0A; 32];
const SEED_BOB: [u8; 32] = [0x0B; 32];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn identity(pseudo: &str, seed: [u8; 32]) -> Identity {
    Identity::generate(pseudo, ChaCha20Rng::from_seed(seed)).unwrap()
}

fn describe(id: &Identity, seed: [u8; 32]) -> Value {
    let p = id.public();
    json!({
        "rng_seed": hex(&seed),
        "pseudo": p.pseudo(),
        "pub_static": hex(&p.pub_static()),
        "pub_sign": hex(&p.pub_sign().to_bytes()),
        "peer_id": hex(&p.peer_id()),
        "peer_id_base32": peer_id_base32(&p.peer_id()),
        "fingerprint": hex(&p.fingerprint()),
        "qr": p.to_qr(),
    })
}

fn compute() -> Value {
    let alice = identity("alice", SEED_ALICE);
    let bob = identity("Bob 🐝", SEED_BOB);
    let code = verification_code(&alice.public(), &bob.public());
    json!({
        "alice": describe(&alice, SEED_ALICE),
        "bob": describe(&bob, SEED_BOB),
        "safety_number": {
            "between": ["alice", "bob"],
            "display": code.to_string(),
            "digits": code.digits(),
        },
    })
}

fn load() -> Value {
    let raw = std::fs::read_to_string(VECTORS_PATH)
        .expect("contracts/packet/identity_v0.json manquant — lancer generer_vecteurs");
    serde_json::from_str(&raw).unwrap()
}

#[test]
fn vecteurs_conformes() {
    assert_eq!(
        compute(),
        load(),
        "sortie identity différente des vecteurs versionnés — régression, ou \
         vecteurs à régénérer volontairement (et à justifier dans le journal)"
    );
}

#[test]
fn vecteurs_rejouables() {
    let v = load();
    let a = PublicIdentity::from_qr(v["alice"]["qr"].as_str().unwrap()).unwrap();
    let b = PublicIdentity::from_qr(v["bob"]["qr"].as_str().unwrap()).unwrap();
    assert_eq!(a.pseudo(), v["alice"]["pseudo"]);
    assert_eq!(hex(&b.pub_static()), v["bob"]["pub_static"]);
    assert_eq!(hex(&b.peer_id()), v["bob"]["peer_id"]);
    assert_eq!(hex(&a.fingerprint()), v["alice"]["fingerprint"]);
    assert_eq!(
        verification_code(&b, &a).to_string(),
        v["safety_number"]["display"]
    );
}

/// Scénario 1 du DoD, côté cœur : A et B créent leur identité (coffre),
/// scannent le QR de l'autre, et affichent le même code de 60 chiffres.
#[test]
fn appairage_a_et_b() {
    let key = [0x42; 32];
    let (mut coffre_a, mut coffre_b) = (MemoryVault::new(), MemoryVault::new());
    let a = load_or_create(
        &mut coffre_a,
        &key,
        "alice",
        ChaCha20Rng::from_seed([1; 32]),
    )
    .unwrap();
    let b = load_or_create(&mut coffre_b, &key, "bob", ChaCha20Rng::from_seed([2; 32])).unwrap();

    // Chacun scanne le QR affiché par l'autre.
    let b_vu_par_a = PublicIdentity::from_qr(&b.public().to_qr()).unwrap();
    let a_vu_par_b = PublicIdentity::from_qr(&a.public().to_qr()).unwrap();

    let code_chez_a = verification_code(&a.public(), &b_vu_par_a);
    let code_chez_b = verification_code(&b.public(), &a_vu_par_b);
    assert_eq!(code_chez_a, code_chez_b);
    assert_eq!(code_chez_a.digits().len(), 60);

    // Un MITM qui substitue sa propre carte change le code affiché chez A.
    let mallory = identity("bob", [3; 32]);
    assert_ne!(
        verification_code(&a.public(), &mallory.public()),
        code_chez_b
    );
}

#[test]
#[ignore = "réécrit contracts/packet/identity_v0.json"]
fn generer_vecteurs() {
    let mut out = serde_json::to_string_pretty(&compute()).unwrap();
    out.push('\n');
    std::fs::write(VECTORS_PATH, out).unwrap();
}
