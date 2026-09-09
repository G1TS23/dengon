//! `dengon-core` — cœur du protocole dengon.
//!
//! Cette crate est la **seule** implémentation du protocole (décision A-2) :
//! elle est partagée par l'application Android (via `dengon-ffi`), le nœud CLI
//! `dengon-node` et le firmware ESP32. Elle ne contient **ni I/O radio ni I/O
//! réseau** : elle produit et consomme des octets, transportés par un
//! `Transport` (voir la crate `dengon-ble`).
//!
//! Modules à venir, d'après `docs/synthese/04-architecture.md` §2 :
//! `protocol`, `crypto`, `identity`, `store`, `sync`, `ledger`,
//! `observability` et la façade `api`.
//!
//! # Contrainte `no_std`
//!
//! `protocol`, `sync` et `ledger` doivent rester compilables en `no_std` +
//! `alloc` pour la cible ESP32. La feature `std` est activée par défaut ; la
//! CI lance `cargo check -p dengon-core --no-default-features` pour détecter
//! toute dépendance à `std` qui se serait glissée par inadvertance.
//!
//! # État
//!
//! Squelette livré par l'US-104. Aucun module réel n'est implémenté.

#![cfg_attr(not(feature = "std"), no_std)]

/// Version du protocole dengon implémentée par cette crate.
///
/// Correspond au champ `version` présent dans chaque paquet
/// (`docs/synthese/05-protocole-et-trame.md`), qui permet la négociation de
/// version à l'ANNOUNCE.
pub const PROTOCOL_VERSION: u8 = 1;

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
    }
}
