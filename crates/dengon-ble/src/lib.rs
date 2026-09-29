//! `dengon-ble` — couche transport BLE de dengon.
//!
//! Contient le trait [`Transport`], **unique couture** entre le cœur et les
//! plateformes ([`docs/synthese/04-architecture.md`] §3), son bouchon en
//! mémoire [`MockTransport`], et la [suite de conformité](conformance) que
//! toute implémentation doit passer.
//!
//! # Pourquoi une seule couture
//!
//! Seules la radio et l'interface sont spécifiques à une plateforme. Le
//! protocole, la crypto, le stockage et le journal vivent dans `dengon-core` et
//! sont partagés. C'est ce qui rend l'ouverture iOS peu coûteuse (décision
//! A-11) : un shell SwiftUI plus une implémentation `Transport`
//! `CoreBluetooth`, sans toucher au cœur.
//!
//! | Implémentation | Où | Techno |
//! |---|---|---|
//! | Bouchon | [`MockTransport`], ici | mémoire — livré par US-105 |
//! | Desktop / CLI | [`CentralTransport`] + `BtleplugRadio` (feature `btleplug`) | `btleplug`, rôle **central** seulement — US-303 |
//! | Android | `AndroidTransport.kt` + `dengon-ffi` | `BluetoothGattServer` — US-213 |
//! | ESP32 | `firmware/dengon-relay/src/transport_nimble.c` | NimBLE — US-220 |
//!
//! # État
//!
//! Le contrat est **gelé** (US-105, area `contract`) : le modifier demande un
//! point d'équipe, parce que quatre implémentations et `dengon-sim` s'appuient
//! dessus. Le transport desktop (US-303) vit derrière la feature `btleplug`.
//!
//! [`docs/synthese/04-architecture.md`]: ../../../docs/synthese/04-architecture.md

#[cfg(feature = "btleplug")]
mod btleplug_radio;
pub mod central;
pub mod conformance;
mod mock;
mod transport;

#[cfg(feature = "btleplug")]
pub use btleplug_radio::{BtleplugRadio, BtleplugTransport};
pub use central::{CentralRadio, CentralTransport, RadioEvent, RadioHandle};
pub use mock::MockTransport;
pub use transport::{
    DisconnectReason, LinkId, Result, Transport, TransportConfig, TransportError, TransportEvent,
};

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
