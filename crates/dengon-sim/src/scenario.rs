//! Scénarios versionnés au format RON (`crates/dengon-sim/scenarios/*.ron`).
//!
//! Un scénario décrit une topologie, une suite d'actions datées (émettre,
//! partitionner, réunir, relier, délier) et des **attendus** vérifiés en fin
//! d'exécution. Il est exécuté avec le comportement [`Inondation`] en
//! attendant le vrai nœud `dengon-core`.
//!
//! ```ron
//! Scenario(
//!     nom: "direct",
//!     noeuds: 2,
//!     duree_ms: 500,
//!     liens: [(a: 0, b: 1, latence_ms: 20)],
//!     actions: [(a_ms: 10, action: Emettre(noeud: 0, charge: "bonjour"))],
//!     attendus: [Livre(noeud: 1, charge: "bonjour")],
//! )
//! ```

use core::fmt;

use serde::Deserialize;

use crate::harness::{Inondation, Simulation};
use crate::reseau::{empreinte, Horodate, NoeudId, ParametresLien};
use crate::SEED_PAR_DEFAUT;

/// Un scénario, tel que lu dans un fichier `.ron`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub nom: String,
    #[serde(default)]
    pub description: String,
    /// Graine de l'aléa ; [`SEED_PAR_DEFAUT`] si absente.
    #[serde(default)]
    pub graine: Option<u64>,
    pub noeuds: usize,
    pub duree_ms: u64,
    #[serde(default = "pas_par_defaut")]
    pub pas_ms: u64,
    #[serde(default)]
    pub liens: Vec<LienScenario>,
    #[serde(default)]
    pub actions: Vec<ActionDatee>,
    #[serde(default)]
    pub attendus: Vec<Attendu>,
}

const fn pas_par_defaut() -> u64 {
    10
}

/// Une arête radio entre `a` et `b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LienScenario {
    pub a: NoeudId,
    pub b: NoeudId,
    #[serde(default)]
    pub latence_ms: u64,
    #[serde(default)]
    pub gigue_ms: u64,
    #[serde(default)]
    pub perte_pour_mille: u16,
}

impl LienScenario {
    const fn parametres(&self) -> ParametresLien {
        ParametresLien {
            latence_ms: self.latence_ms,
            gigue_ms: self.gigue_ms,
            perte_pour_mille: self.perte_pour_mille,
        }
    }
}

/// Une action et sa date.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionDatee {
    pub a_ms: u64,
    pub action: Action,
}

/// Ce qui peut arriver pendant un scénario.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum Action {
    /// L'application de `noeud` émet `charge` (texte UTF-8).
    Emettre { noeud: NoeudId, charge: String },
    /// Seuls les liens internes à chaque groupe survivent.
    Partitionner { groupes: Vec<Vec<NoeudId>> },
    /// Lève la partition.
    Reunir,
    /// Ajoute une arête, ou change ses paramètres.
    Relier(LienScenario),
    /// Retire une arête (coupure brutale).
    Delier { a: NoeudId, b: NoeudId },
}

/// Ce qui doit être vrai à la fin.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum Attendu {
    /// `charge` a été livrée à `noeud`.
    Livre { noeud: NoeudId, charge: String },
    /// `charge` n'a **pas** été livrée à `noeud`.
    NonLivre { noeud: NoeudId, charge: String },
}

/// Scénario illisible ou incohérent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErreurScenario {
    /// Le texte n'est pas un scénario RON valide.
    Lecture(String),
    /// Le scénario se lit, mais référence un nœud inexistant, etc.
    Invalide(String),
}

impl fmt::Display for ErreurScenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lecture(e) => write!(f, "scénario illisible : {e}"),
            Self::Invalide(e) => write!(f, "scénario invalide : {e}"),
        }
    }
}

impl std::error::Error for ErreurScenario {}

/// Résultat d'une exécution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rapport {
    pub nom: String,
    pub graine: u64,
    /// [`empreinte`] de la trace : identique d'une exécution à l'autre à
    /// graine égale.
    pub empreinte: u64,
    pub trace: Vec<Horodate>,
    /// Attendus non satisfaits, en clair. Vide = succès.
    pub echecs: Vec<String>,
}

impl Rapport {
    /// Tous les attendus sont satisfaits.
    #[must_use]
    pub fn reussi(&self) -> bool {
        self.echecs.is_empty()
    }
}

impl Scenario {
    /// Lit et valide un scénario RON.
    ///
    /// # Errors
    ///
    /// [`ErreurScenario`] si le texte est illisible ou incohérent.
    pub fn depuis_ron(texte: &str) -> Result<Self, ErreurScenario> {
        let s: Self = ron::from_str(texte).map_err(|e| ErreurScenario::Lecture(e.to_string()))?;
        s.valider()?;
        Ok(s)
    }

    fn verifier_noeud(&self, n: NoeudId, ou: &str) -> Result<(), ErreurScenario> {
        if n < self.noeuds {
            Ok(())
        } else {
            Err(ErreurScenario::Invalide(format!(
                "{ou} : nœud {n} hors de 0..{}",
                self.noeuds
            )))
        }
    }

    fn verifier_lien(&self, l: &LienScenario) -> Result<(), ErreurScenario> {
        self.verifier_noeud(l.a, "lien")?;
        self.verifier_noeud(l.b, "lien")?;
        if l.a == l.b {
            return Err(ErreurScenario::Invalide(format!(
                "lien {0}—{0} : boucle",
                l.a
            )));
        }
        if l.perte_pour_mille > 1000 {
            return Err(ErreurScenario::Invalide(format!(
                "lien {}—{} : perte {} ‰ > 1000",
                l.a, l.b, l.perte_pour_mille
            )));
        }
        Ok(())
    }

    /// Vérifie la cohérence interne (indices, pas, pertes).
    ///
    /// # Errors
    ///
    /// [`ErreurScenario::Invalide`] au premier problème.
    pub fn valider(&self) -> Result<(), ErreurScenario> {
        if self.noeuds == 0 {
            return Err(ErreurScenario::Invalide("aucun nœud".into()));
        }
        if self.pas_ms == 0 {
            return Err(ErreurScenario::Invalide("pas_ms doit être > 0".into()));
        }
        for l in &self.liens {
            self.verifier_lien(l)?;
        }
        for a in &self.actions {
            match &a.action {
                Action::Emettre { noeud, .. } => self.verifier_noeud(*noeud, "Emettre")?,
                Action::Partitionner { groupes } => {
                    for n in groupes.iter().flatten() {
                        self.verifier_noeud(*n, "Partitionner")?;
                    }
                }
                Action::Reunir => {}
                Action::Relier(l) => self.verifier_lien(l)?,
                Action::Delier { a, b } => {
                    self.verifier_noeud(*a, "Delier")?;
                    self.verifier_noeud(*b, "Delier")?;
                }
            }
        }
        for a in &self.attendus {
            let (Attendu::Livre { noeud, .. } | Attendu::NonLivre { noeud, .. }) = a;
            self.verifier_noeud(*noeud, "attendu")?;
        }
        Ok(())
    }

    /// Exécute le scénario. `graine_forcee` remplace la graine du fichier.
    ///
    /// # Errors
    ///
    /// [`ErreurScenario::Invalide`] si le scénario est incohérent.
    pub fn executer(&self, graine_forcee: Option<u64>) -> Result<Rapport, ErreurScenario> {
        self.valider()?;
        let graine = graine_forcee.or(self.graine).unwrap_or(SEED_PAR_DEFAUT);
        let mut sim = Simulation::new(graine);
        sim.ajouter_noeuds(self.noeuds, || Box::new(Inondation::default()))
            .map_err(|e| ErreurScenario::Invalide(e.to_string()))?;
        for l in &self.liens {
            sim.reseau().relier(l.a, l.b, l.parametres());
        }

        // Tri stable : deux actions à la même date gardent l'ordre du fichier.
        let mut actions: Vec<&ActionDatee> = self.actions.iter().collect();
        actions.sort_by_key(|a| a.a_ms);
        let mut suivantes = actions.into_iter().peekable();

        while sim.reseau().maintenant_ms() < self.duree_ms {
            let t = sim.reseau().maintenant_ms();
            while let Some(a) = suivantes.next_if(|a| a.a_ms <= t) {
                appliquer(&mut sim, &a.action);
            }
            sim.pas(self.pas_ms);
        }

        let echecs = self
            .attendus
            .iter()
            .filter_map(|a| match a {
                Attendu::Livre { noeud, charge }
                    if !sim.livres(*noeud).iter().any(|m| m == charge.as_bytes()) =>
                {
                    Some(format!("« {charge} » non livré au nœud {noeud}"))
                }
                Attendu::NonLivre { noeud, charge }
                    if sim.livres(*noeud).iter().any(|m| m == charge.as_bytes()) =>
                {
                    Some(format!(
                        "« {charge} » livré au nœud {noeud} alors qu'il ne devait pas l'être"
                    ))
                }
                _ => None,
            })
            .collect();

        let trace = sim.trace();
        Ok(Rapport {
            nom: self.nom.clone(),
            graine,
            empreinte: empreinte(&trace),
            trace,
            echecs,
        })
    }
}

fn appliquer(sim: &mut Simulation, action: &Action) {
    match action {
        Action::Emettre { noeud, charge } => sim.emettre(*noeud, charge.as_bytes()),
        Action::Partitionner { groupes } => sim.reseau().partitionner(groupes),
        Action::Reunir => sim.reseau().reunir(),
        Action::Relier(l) => sim.reseau().relier(l.a, l.b, l.parametres()),
        Action::Delier { a, b } => sim.reseau().delier(*a, *b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRECT: &str = r#"Scenario(
        nom: "direct",
        noeuds: 2,
        duree_ms: 300,
        liens: [(a: 0, b: 1, latence_ms: 20)],
        actions: [(a_ms: 10, action: Emettre(noeud: 0, charge: "bonjour"))],
        attendus: [Livre(noeud: 1, charge: "bonjour"), NonLivre(noeud: 0, charge: "bonjour")],
    )"#;

    #[test]
    fn un_scenario_direct_reussit_et_se_rejoue() {
        let s = Scenario::depuis_ron(DIRECT).unwrap();
        let r1 = s.executer(None).unwrap();
        assert!(r1.reussi(), "{:?}", r1.echecs);
        assert_eq!(r1.graine, SEED_PAR_DEFAUT);
        assert_eq!(s.executer(None).unwrap(), r1);
        assert_eq!(s.executer(Some(5)).unwrap().graine, 5);
    }

    #[test]
    fn les_attendus_non_tenus_sont_rapportes() {
        let s = Scenario::depuis_ron(
            r#"Scenario(nom: "x", noeuds: 2, duree_ms: 50,
                actions: [(a_ms: 0, action: Emettre(noeud: 0, charge: "m"))],
                attendus: [Livre(noeud: 1, charge: "m"), NonLivre(noeud: 1, charge: "absent")])"#,
        )
        .unwrap();
        let r = s.executer(None).unwrap();
        assert!(!r.reussi());
        assert_eq!(r.echecs.len(), 1, "{:?}", r.echecs);

        // Toutes les actions passent par `appliquer`.
        let s = Scenario::depuis_ron(
            r#"Scenario(nom: "y", noeuds: 3, duree_ms: 100, pas_ms: 5,
                actions: [
                    (a_ms: 0, action: Relier((a: 0, b: 1))),
                    (a_ms: 0, action: Relier((a: 1, b: 2, gigue_ms: 3))),
                    (a_ms: 10, action: Partitionner(groupes: [[0], [1, 2]])),
                    (a_ms: 20, action: Reunir),
                    (a_ms: 30, action: Delier(a: 0, b: 1)),
                    (a_ms: 40, action: Emettre(noeud: 0, charge: "seul")),
                ],
                attendus: [NonLivre(noeud: 2, charge: "seul")])"#,
        )
        .unwrap();
        assert!(s.executer(None).unwrap().reussi());
    }

    #[test]
    fn les_scenarios_incoherents_sont_refuses() {
        let cas = [
            r#"Scenario(nom: "a", noeuds: 0, duree_ms: 1)"#,
            r#"Scenario(nom: "a", noeuds: 1, duree_ms: 1, pas_ms: 0)"#,
            r#"Scenario(nom: "a", noeuds: 2, duree_ms: 1, liens: [(a: 0, b: 2)])"#,
            r#"Scenario(nom: "a", noeuds: 2, duree_ms: 1, liens: [(a: 1, b: 1)])"#,
            r#"Scenario(nom: "a", noeuds: 2, duree_ms: 1, liens: [(a: 0, b: 1, perte_pour_mille: 1001)])"#,
            r#"Scenario(nom: "a", noeuds: 2, duree_ms: 1, actions: [(a_ms: 0, action: Emettre(noeud: 5, charge: ""))])"#,
            r#"Scenario(nom: "a", noeuds: 2, duree_ms: 1, actions: [(a_ms: 0, action: Partitionner(groupes: [[3]]))])"#,
            r#"Scenario(nom: "a", noeuds: 2, duree_ms: 1, actions: [(a_ms: 0, action: Relier((a: 0, b: 9)))])"#,
            r#"Scenario(nom: "a", noeuds: 2, duree_ms: 1, actions: [(a_ms: 0, action: Delier(a: 0, b: 9))])"#,
            r#"Scenario(nom: "a", noeuds: 2, duree_ms: 1, attendus: [Livre(noeud: 4, charge: "")])"#,
        ];
        for texte in cas {
            assert!(
                matches!(
                    Scenario::depuis_ron(texte),
                    Err(ErreurScenario::Invalide(_))
                ),
                "{texte}"
            );
        }
        let lecture = Scenario::depuis_ron("pas du ron").unwrap_err();
        assert!(matches!(lecture, ErreurScenario::Lecture(_)));
        assert!(lecture.to_string().starts_with("scénario illisible"));
        assert!(
            Scenario::depuis_ron(r#"Scenario(nom: "a", noeuds: 1, duree_ms: 1, inconnu: 1)"#)
                .is_err()
        );
        assert!(ErreurScenario::Invalide("x".into())
            .to_string()
            .starts_with("scénario invalide"));
    }
}
