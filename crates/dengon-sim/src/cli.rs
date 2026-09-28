//! Logique de la ligne de commande, séparée de `main.rs` pour être testée.
//!
//! ```text
//! dengon-sim [--graine N] <scenario.ron>...
//! ```
//!
//! Une ligne par scénario : `✓`/`✗`, nom, graine, empreinte de trace. Code de
//! sortie non nul si un scénario est illisible ou si un attendu échoue. Le job
//! CI `sim` lance la commande deux fois et compare les sorties : c'est la
//! preuve de déterminisme de bout en bout.

use std::fmt::Write as _;
use std::fs;

use crate::scenario::Scenario;

/// Sortie texte et succès global.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sortie {
    pub texte: String,
    pub succes: bool,
}

/// Exécute la commande sur ses arguments (sans le nom du programme).
#[must_use]
pub fn executer(args: &[String]) -> Sortie {
    let mut texte = String::new();
    let mut succes = true;
    let mut graine = None;
    let mut fichiers = Vec::new();

    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--graine" {
            match it.next().map(|v| parse_graine(v)) {
                Some(Some(g)) => graine = Some(g),
                _ => {
                    return Sortie {
                        texte: "--graine attend un entier (décimal ou 0x…)\n".into(),
                        succes: false,
                    }
                }
            }
        } else {
            fichiers.push(a.clone());
        }
    }
    if fichiers.is_empty() {
        return Sortie {
            texte: "usage : dengon-sim [--graine N] <scenario.ron>...\n".into(),
            succes: false,
        };
    }

    for f in &fichiers {
        let resultat = fs::read_to_string(f)
            .map_err(|e| format!("lecture de {f} : {e}"))
            .and_then(|t| Scenario::depuis_ron(&t).map_err(|e| format!("{f} : {e}")))
            .and_then(|s| s.executer(graine).map_err(|e| format!("{f} : {e}")));
        match resultat {
            Ok(r) => {
                let marque = if r.reussi() { '✓' } else { '✗' };
                let _ = writeln!(
                    texte,
                    "{marque} {} graine={:#x} empreinte={:#018x} evenements={}",
                    r.nom,
                    r.graine,
                    r.empreinte,
                    r.trace.len()
                );
                for e in &r.echecs {
                    let _ = writeln!(texte, "    échec : {e}");
                }
                succes &= r.reussi();
            }
            Err(e) => {
                let _ = writeln!(texte, "✗ {e}");
                succes = false;
            }
        }
    }
    Sortie { texte, succes }
}

fn parse_graine(v: &str) -> Option<u64> {
    v.strip_prefix("0x")
        .map_or_else(|| v.parse().ok(), |h| u64::from_str_radix(h, 16).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    fn scenario(dir: &std::path::Path, nom: &str, contenu: &str) -> String {
        let p = dir.join(nom);
        fs::write(&p, contenu).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn usage_et_graine_invalide() {
        assert!(!executer(&[]).succes);
        assert!(!executer(&args(&["--graine"])).succes);
        assert!(!executer(&args(&["--graine", "zz", "x.ron"])).succes);
        assert_eq!(parse_graine("0x10"), Some(16));
        assert_eq!(parse_graine("10"), Some(10));
    }

    #[test]
    fn execute_et_rapporte_chaque_fichier() {
        let dir = std::env::temp_dir().join(format!("dengon-sim-cli-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let ok = scenario(
            &dir,
            "ok.ron",
            r#"Scenario(nom: "ok", noeuds: 2, duree_ms: 50, liens: [(a: 0, b: 1)],
                actions: [(a_ms: 0, action: Emettre(noeud: 0, charge: "m"))],
                attendus: [Livre(noeud: 1, charge: "m")])"#,
        );
        let ko = scenario(
            &dir,
            "ko.ron",
            r#"Scenario(nom: "ko", noeuds: 2, duree_ms: 50,
                attendus: [Livre(noeud: 1, charge: "m")])"#,
        );
        let illisible = scenario(&dir, "illisible.ron", "nope");

        let s = executer(&args(&[&ok, "--graine", "0x2a"]));
        assert!(s.succes, "{}", s.texte);
        assert!(s.texte.starts_with("✓ ok graine=0x2a"));
        assert_eq!(s, executer(&args(&[&ok, "--graine", "42"])), "déterministe");

        let s = executer(&args(&[&ok, &ko, &illisible, "absent.ron"]));
        assert!(!s.succes);
        assert!(s.texte.contains("✗ ko"));
        assert!(s.texte.contains("échec : « m » non livré"));
        assert!(s.texte.contains("illisible"));
        assert!(s.texte.contains("lecture de absent.ron"));
        let _ = fs::remove_dir_all(&dir);
    }
}
