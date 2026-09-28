// Fichier de test intégré : un `unwrap` qui panique = échec de test.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Les scénarios livrés dans `scenarios/*.ron` : tous lisibles, tous réussis,
//! et **déterministes** (même graine → même trace, graine différente → trace
//! différente dès qu'il y a de l'aléa).

use std::fs;
use std::path::PathBuf;

use dengon_sim::{EntreeTrace, Scenario};

fn scenarios() -> Vec<(String, Scenario)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scenarios");
    let mut v: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .map(|p| {
            let texte = fs::read_to_string(&p).unwrap();
            let s = Scenario::depuis_ron(&texte).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            (p.file_name().unwrap().to_string_lossy().into_owned(), s)
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

#[test]
fn les_scenarios_livres_reussissent() {
    let tous = scenarios();
    assert!(tous.len() >= 4, "au moins 4 scénarios livrés");
    for (fichier, s) in tous {
        let r = s.executer(None).unwrap();
        assert!(r.reussi(), "{fichier} : {:?}", r.echecs);
    }
}

#[test]
fn meme_graine_meme_trace_pour_chaque_scenario() {
    for (fichier, s) in scenarios() {
        let a = s.executer(None).unwrap();
        let b = s.executer(None).unwrap();
        assert_eq!(a.trace, b.trace, "{fichier} : traces différentes");
        assert_eq!(a.empreinte, b.empreinte, "{fichier}");
    }
}

#[test]
fn la_graine_change_le_resultat_d_un_reseau_avec_pertes() {
    let (_, s) = scenarios()
        .into_iter()
        .find(|(f, _)| f == "lossy_mesh.ron")
        .unwrap();
    let a = s.executer(Some(1)).unwrap();
    let b = s.executer(Some(2)).unwrap();
    assert_ne!(a.empreinte, b.empreinte, "la graine doit piloter l'aléa");
    // Et la perte est réellement exercée.
    assert!(a
        .trace
        .iter()
        .any(|h| matches!(h.entree, EntreeTrace::Perte { .. })));
}
