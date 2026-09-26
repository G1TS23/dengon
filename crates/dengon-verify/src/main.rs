//! `dengon-verify` — vérificateur de journal chaîné.
//!
//! Binaire appelé **en sous-processus** par le dashboard Python (décision A-5,
//! voir `docs/synthese/09-dashboard-et-donnees.md`). Le dashboard étant en
//! FastAPI, c'est ce binaire qui porte la vérification cryptographique : il
//! réutilisera `dengon_core::ledger::verify_chain` plutôt que de réimplémenter
//! la chaîne en Python (décision B-5).
//!
//! Il lira un export de journal accompagné de son `LOG_ATTEST` et rendra l'un
//! des quatre verdicts de [`Verdict`]. Implémentation complète (lecture de
//! fichier, `LOG_ATTEST`) : P1.13. `Verdict` et `verify_chain` viennent
//! maintenant réellement de `dengon-core` (US-206), comme annoncé
//! ci-dessus — plus de définition dupliquée ici.
//!
//! # État
//!
//! Squelette livré par l'US-104. Branché sur `dengon_core::ledger` par
//! l'US-206 ; lecture de fichier + `LOG_ATTEST` restent à faire (P1.13).

pub use dengon_core::ledger::Verdict;

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
