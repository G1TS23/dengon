//! État persistant du nœud CLI : clé locale, identité scellée, base SQLite.
//!
//! Trois fichiers à côté de `--db` :
//!
//! | Fichier | Contenu |
//! |---|---|
//! | `<db>` | base SQLite (messages, contacts) |
//! | `<db>.identity` | identité scellée par le coffre (`FileVault`) |
//! | `<db>.key` | 32 octets aléatoires : clé du coffre **et** des champs sensibles |
//!
//! ⚠ `<db>.key` est en clair sur disque : c'est un nœud de **test**, sans
//! trousseau système. Quiconque lit ce fichier lit l'identité. Sur Android, la
//! clé vit dans le Keystore matériel ; ici, seuls les droits du fichier
//! (`0600` sous Unix) protègent.

use std::path::{Path, PathBuf};

use dengon_core::api::Node;
use dengon_core::identity::{load_or_create, FileVault};
use dengon_core::store::{FixedKeySource, Store};
use rand_core::{OsRng, RngCore};

/// Erreur lisible pour l'utilisateur de la CLI.
pub type Erreur = String;

fn chemin_avec_suffixe(db: &Path, suffixe: &str) -> PathBuf {
    let mut s = db.as_os_str().to_owned();
    s.push(suffixe);
    s.into()
}

/// Lit `<db>.key`, ou la crée au premier lancement.
fn cle_locale(db: &Path) -> Result<[u8; 32], Erreur> {
    let chemin = chemin_avec_suffixe(db, ".key");
    match std::fs::read(&chemin) {
        Ok(octets) => <[u8; 32]>::try_from(octets.as_slice())
            .map_err(|_| format!("{} : 32 octets attendus", chemin.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut cle = [0u8; 32];
            OsRng.fill_bytes(&mut cle);
            ecrire_prive(&chemin, &cle)?;
            Ok(cle)
        }
        Err(e) => Err(format!("{} : {e}", chemin.display())),
    }
}

fn ecrire_prive(chemin: &Path, octets: &[u8]) -> Result<(), Erreur> {
    use std::io::Write as _;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options
        .open(chemin)
        .and_then(|mut f| f.write_all(octets))
        .map_err(|e| format!("{} : {e}", chemin.display()))
}

/// Ouvre (ou crée) le nœud et sa base pour `db`.
///
/// # Errors
///
/// Fichier illisible, coffre corrompu, base impossible à ouvrir.
pub fn ouvrir_noeud(db: &Path, pseudo: &str) -> Result<Node, Erreur> {
    let cle = cle_locale(db)?;
    let mut coffre = FileVault::new(chemin_avec_suffixe(db, ".identity"));
    let identite = load_or_create(&mut coffre, &cle, pseudo, OsRng)
        .map_err(|e| format!("identité : {e:?}"))?;

    let mut noeud = Node::new(identite, OsRng.next_u64());
    let store = Store::open(db, FixedKeySource(cle)).map_err(|e| format!("base : {e:?}"))?;
    noeud.attach_store(store);
    Ok(noeud)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn dossier_temporaire(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dengon-node-{nom}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn le_peer_id_survit_a_un_redemarrage() {
        let dossier = dossier_temporaire("redemarrage");
        let db = dossier.join("alice.db");

        let premier = ouvrir_noeud(&db, "alice").unwrap().peer_id();
        // Le pseudo est ignoré à la réouverture : l'identité enregistrée fait foi.
        let second = ouvrir_noeud(&db, "autre-pseudo").unwrap();

        assert_eq!(premier, second.peer_id());
        assert_eq!(second.public_identity().pseudo(), "alice");
        drop(second); // Windows refuse de supprimer une base encore ouverte.
        std::fs::remove_dir_all(dossier).unwrap();
    }
}
