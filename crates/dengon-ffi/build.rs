//! Génère le pont FFI Rust (scaffolding) à partir de `src/dengon.udl`
//! (US-106). Ne génère PAS les bindings Kotlin : ça, c'est le binaire
//! `uniffi-bindgen` de cette crate (feature `bindgen`, US-302), lancé par
//! `android/scripts/build-ffi.sh`. Ici, on ne produit que le code Rust côté
//! scaffolding — la preuve que le contrat `.udl` est syntaxiquement valide et
//! cohérent avec `src/lib.rs`.

fn main() {
    if let Err(err) = uniffi::generate_scaffolding("src/dengon.udl") {
        panic!("dengon.udl invalide : {err}");
    }
}
