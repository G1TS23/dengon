//! `dengon-node` — nœud dengon headless : bancs de test, PC fixe, bootstrap
//! du maillage.
//!
//! Invocation visée par la conception (`docs/synthese/04-architecture.md` §5) :
//!
//! ```text
//! dengon-node run --name alice --db ./alice.db
//! ```
//!
//! L'analyse d'arguments (clap), la boucle d'événements et le transport
//! `btleplug` arrivent avec P1.11 ; `tokio` ne sera ajouté qu'à ce moment-là.
//!
//! # État
//!
//! Squelette livré par l'US-104 : le binaire se contente d'afficher sa version.

fn main() {
    println!(
        "dengon-node {} — squelette (protocole v{})",
        env!("CARGO_PKG_VERSION"),
        dengon_core::PROTOCOL_VERSION
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn les_deux_dependances_sont_reellement_liees() {
        assert_eq!(dengon_core::PROTOCOL_VERSION, 1);
        assert!(!dengon_ble::VERSION.is_empty());
    }
}
