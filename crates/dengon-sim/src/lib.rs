//! `dengon-sim` — simulateur multi-nœuds.
//!
//! N nœuds reliés par un `Transport` **en mémoire**, avec un modèle réseau
//! scriptable : latence, gigue, perte, partition
//! (`docs/synthese/10-benchmarks-mvp-tests.md` §4.3). Sert à démontrer le
//! maillage **sans matériel**, et à le tester en CI.
//!
//! # Modules
//!
//! - [`reseau`] — le réseau simulé (horloge virtuelle, topologie, partitions,
//!   files d'événements) et [`SimTransport`], qui implémente le contrat gelé
//!   `dengon_ble::Transport` et passe **la même suite de conformité** que
//!   `MockTransport` (`tests/conformite_sim.rs`).
//! - [`harness`] — [`Simulation`] : N nœuds, chacun avec un [`Comportement`]
//!   injecté, avancés pas à pas. [`Inondation`] est le comportement de
//!   démonstration (flood générique) ; [`noeud_client::NoeudClient`] est le
//!   **vrai** `dengon-core::api::Node` (US-304).
//! - [`scenario`] — scénarios `scenarios/*.ron` rejouables (avec
//!   [`Inondation`]), avec attendus. Les scénarios réels (US-304,
//!   `NoeudClient`) sont écrits en Rust : `tests/scenarios_reel.rs`.
//! - [`noeud_client`] — [`noeud_client::NoeudClient`], le nœud client réel.
//! - [`noeud_relais`] — [`noeud_relais::NoeudRelais`], le nœud relais réel
//!   (US-304, 2/2) : `multihop`/`partition_merge` en dépendent, `api::Node`
//!   ne relaie jamais.
//! - [`alea`] — SplitMix64, le seul hasard du simulateur.
//! - [`cli`] — la ligne de commande `dengon-sim`.
//!
//! # Déterminisme
//!
//! Aucune source de non-déterminisme : horloge virtuelle, aléa à graine fixe,
//! collections ordonnées (`BTreeMap`), nœuds servis par indice croissant. Deux
//! exécutions à même graine produisent la **même trace**, donc la même
//! [`empreinte`] — c'est ce que vérifient les tests et le job CI `sim`.

pub mod alea;
pub mod cli;
pub mod harness;
pub mod noeud_client;
pub mod noeud_relais;
pub mod reseau;
pub mod scenario;

pub use harness::{Comportement, Contexte, Inondation, Simulation};
pub use reseau::{
    empreinte, EntreeTrace, Horodate, NoeudId, ParametresLien, ReseauPartage, SimTransport,
};
pub use scenario::{ErreurScenario, Rapport, Scenario};

/// Graine par défaut du générateur aléatoire.
///
/// Le déterminisme est une exigence de la CI : un scénario qui échoue doit
/// pouvoir être rejoué à l'identique en local.
pub const SEED_PAR_DEFAUT: u64 = 0x_de_46_07_11_de_46_07_11;

#[cfg(test)]
mod tests {
    use super::SEED_PAR_DEFAUT;

    #[test]
    fn la_graine_est_fixe_et_non_nulle() {
        assert_ne!(SEED_PAR_DEFAUT, 0);
    }

    #[test]
    fn le_coeur_est_reellement_lie() {
        assert_eq!(dengon_core::PROTOCOL_VERSION, 1);
    }
}
