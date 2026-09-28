//! Génère le pont FFI Rust (scaffolding) à partir de `src/dengon.udl`
//! (US-106). Ne génère PAS les bindings Kotlin : ça, c'est
//! `uniffi-bindgen generate`, une étape séparée qui arrive avec le vrai FFI
//! (US-302). Ici, on ne produit que le code Rust côté scaffolding — la
//! preuve que le contrat `.udl` est syntaxiquement valide et cohérent avec
//! `src/lib.rs`.

fn main() {
    if let Err(err) = uniffi::generate_scaffolding("src/dengon.udl") {
        panic!("dengon.udl invalide : {err}");
    }
}
