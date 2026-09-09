//! `dengon-ffi` — surface FFI de dengon exposée via UniFFI.
//!
//! Génère les bindings Kotlin consommés par l'application Android ; Swift est
//! reporté en v2 (`docs/synthese/04-architecture.md` §5). C'est le pont entre
//! le cœur Rust et `android/.../ble/AndroidTransport.kt`.
//!
//! La surface v0 (fichier UDL + bouchon Kotlin) est livrée par l'**US-106** ;
//! la dépendance à `uniffi` et le `build.rs` associé arrivent avec elle.
//!
//! # Note sur `unsafe`
//!
//! UniFFI génère du code `unsafe`. Le lint `unsafe_code = "deny"` défini dans
//! `[workspace.lints.rust]` devra être neutralisé ici par un
//! `#![allow(unsafe_code)]`. C'est précisément pour cela qu'il est en `deny`
//! et non en `forbid`, qui serait inviolable.
//!
//! # État
//!
//! Squelette livré par l'US-104.

/// Version de la surface FFI, destinée à être exposée aux plateformes hôtes.
pub fn version() -> String {
    format!(
        "{} (protocole v{})",
        env!("CARGO_PKG_VERSION"),
        dengon_core::PROTOCOL_VERSION
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn la_version_expose_le_numero_de_protocole() {
        assert!(super::version().contains("protocole v1"));
    }
}
