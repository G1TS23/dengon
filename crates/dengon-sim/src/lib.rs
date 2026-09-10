//! `dengon-sim` — simulateur multi-nœuds.
//!
//! N instances de `dengon-core` reliées par un `Transport` **en mémoire**,
//! avec un modèle réseau scriptable : latence, perte, bande passante,
//! partition, churn, horloges désynchronisées
//! (`docs/synthese/10-benchmarks-mvp-tests.md` §4.3).
//!
//! Les scénarios sont versionnés dans `scenarios/*.ron` (`direct`, `multihop`,
//! `recipient_offline`, `partition_merge`, `flood`, `tamper`…) et **rejouables** :
//! la graine du générateur aléatoire est fixe, sans quoi un échec de CI ne
//! serait pas reproductible.
//!
//! Implémentation réelle : P1.12. Job CI dédié `sim` : US-222.
//!
//! # État
//!
//! Squelette livré par l'US-104.

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
