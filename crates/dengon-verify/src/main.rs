//! `dengon-verify` — vérificateur de journal chaîné.
//!
//! Binaire appelé **en sous-processus** par le dashboard Python (décision A-5,
//! voir `docs/synthese/09-dashboard-et-donnees.md`). Le dashboard étant en
//! FastAPI, c'est ce binaire qui porte la vérification cryptographique : il
//! réutilisera `dengon_core::ledger::verify_chain` plutôt que de réimplémenter
//! la chaîne en Python (décision B-5).
//!
//! Il lira un export de journal accompagné de son `LOG_ATTEST` et rendra l'un
//! des quatre verdicts de [`Verdict`]. Implémentation : P1.13.
//!
//! # État
//!
//! Squelette livré par l'US-104.

/// Verdicts rendus par la vérification d'un journal chaîné.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Chaîne cohérente et signatures valides.
    Ok,
    /// Une entrée a été modifiée, ou une signature est invalide.
    Broken,
    /// Deux entrées concurrentes revendiquent la même position.
    Fork,
    /// Une ou plusieurs positions manquent dans la chaîne.
    Gap,
}

fn main() {
    println!(
        "dengon-verify {} — squelette (protocole v{})",
        env!("CARGO_PKG_VERSION"),
        dengon_core::PROTOCOL_VERSION
    );
}

#[cfg(test)]
mod tests {
    use super::Verdict;

    #[test]
    fn les_quatre_verdicts_sont_distincts() {
        assert_ne!(Verdict::Ok, Verdict::Broken);
        assert_ne!(Verdict::Fork, Verdict::Gap);
        assert_ne!(Verdict::Ok, Verdict::Gap);
    }
}
