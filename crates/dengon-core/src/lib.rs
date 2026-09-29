//! `dengon-core` — cœur du protocole dengon.
//!
//! Cette crate est la **seule** implémentation du protocole (décision A-2) :
//! elle est partagée par l'application Android (via `dengon-ffi`), le nœud CLI
//! `dengon-node` et le firmware ESP32. Elle ne contient **ni I/O radio ni I/O
//! réseau** : elle produit et consomme des octets, transportés par un
//! `Transport` (voir la crate `dengon-ble`).
//!
//! Modules, d'après `docs/synthese/04-architecture.md` §2 : [`protocol`]
//! (livré — types & constantes US-108, codec US-201), [`crypto`] (livré),
//! [`identity`] (livré — clés, QR, code de vérification, coffre), [`ledger`]
//! (livré), `store` (livré, feature `std`), [`sync`] (livré : `sync::status`
//! US-211, `sync::routing` US-209, `sync::inventory` US-210, `sync::courier`
//! US-212), puis `observability` et la façade `api` (sprint 2).
//!
//! # Contrainte `no_std`
//!
//! [`protocol`], `sync`, `ledger` et `observability` doivent rester
//! compilables en `no_std` + `alloc` pour la cible ESP32 (le firmware doit
//! pouvoir émettre `relay.*`/`pkt.*`). La feature `std` est activée par
//! défaut ; la CI lance `cargo check -p dengon-core --no-default-features`
//! pour détecter toute dépendance à `std` qui se serait glissée par
//! inadvertance.
//!
//! # État
//!
//! Squelette livré par l'US-104. `protocol` livré par l'US-108. `ledger`
//! livré par l'US-206. `store` livré par l'US-207. `crypto` livré par
//! l'US-203 (Ed25519) et l'US-204 (Noise, `recipient_tag`, padding).
//! `sync::status` livré par l'US-211, `sync::routing` par l'US-209,
//! `sync::inventory` par l'US-210, `sync::courier` par l'US-212.
//! `observability` livré par l'US-208.

#![cfg_attr(not(feature = "std"), no_std)]

// `alloc` est nécessaire en no_std pour `Vec`/`String`/`BTreeMap` (utilisés
// par `ledger`, `sync`, `observability`, par le codec de `protocol` et par
// `crypto::noise`, car snow alloue) ; en mode `std`, `alloc` est déjà
// réexporté par la libstd, donc cette déclaration ne coûte rien et le code
// reste identique dans les deux configurations.
extern crate alloc;

#[cfg(feature = "std")]
pub mod api;
pub mod crypto;
pub mod identity;
pub mod ledger;
pub mod observability;
pub mod protocol;
pub mod relay;
#[cfg(feature = "std")]
pub mod store;
pub mod sync;

/// Version du protocole dengon implémentée par cette crate.
///
/// Alias historique de [`protocol::consts::PROTO_VERSION`], conservé parce que
/// les crates sœurs (`dengon-ble`, `dengon-node`, …) l'utilisent comme test de
/// liaison (US-104). La source de vérité est `protocol::consts`.
pub const PROTOCOL_VERSION: u8 = protocol::consts::PROTO_VERSION;

/// Version de la crate, telle que déclarée dans `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::{PROTOCOL_VERSION, VERSION};

    #[test]
    fn la_version_de_crate_est_renseignee() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn la_version_de_protocole_vaut_un() {
        assert_eq!(PROTOCOL_VERSION, 1);
        assert_eq!(PROTOCOL_VERSION, crate::protocol::consts::PROTO_VERSION);
    }
}
