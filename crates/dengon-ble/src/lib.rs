//! `dengon-ble` — couche transport BLE de dengon.
//!
//! Contient le trait `Transport`, unique couture entre le cœur et les
//! plateformes (`docs/synthese/04-architecture.md` §3), et son implémentation
//! desktop / CLI fondée sur `btleplug`.
//!
//! Le trait lui-même (`start`, `poll`, `send`, `broadcast`) et le transport
//! bouchon en mémoire sont livrés par l'**US-105** ; la dépendance à
//! `btleplug` sera ajoutée à ce moment-là.
//!
//! # État
//!
//! Squelette livré par l'US-104.

/// Version de la crate, telle que déclarée dans `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn le_coeur_est_reellement_lie() {
        // Vérifie que l'arête dengon-ble -> dengon-core du graphe de
        // dépendances est câblée, et pas seulement déclarée.
        assert_eq!(dengon_core::PROTOCOL_VERSION, 1);
    }
}
