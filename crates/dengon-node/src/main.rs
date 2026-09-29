//! `dengon-node` — nœud dengon headless : bancs de test, PC fixe.
//!
//! ```text
//! dengon-node identity --name alice --db ./alice.db
//! dengon-node run --name alice --db ./alice.db --peer <QR-du-pair> --send "salut"
//! ```
//!
//! # Rôle BLE : central seulement
//!
//! Le transport est `btleplug`, qui ne sait que **scanner et se connecter**
//! (Spike B, US-102). Ce nœud rejoint donc un pair qui **annonce** — un
//! téléphone Android, un relais ESP32 — mais **ne peut pas être découvert** :
//! deux `dengon-node` ne se voient pas entre eux.
//!
//! # Contenu du crate
//!
//! - `session` : la boucle nœud ⇄ transport, générique et testée sur bouchon ;
//! - `etat` : clé, identité et base sur disque.

mod etat;
// Sans la feature `ble`, seule la commande `run` utilise la session.
#[cfg_attr(not(feature = "ble"), allow(dead_code))]
mod session;

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use dengon_core::identity::peer_id_base32;

#[derive(Debug, Parser)]
#[command(name = "dengon-node", version, about = "Nœud dengon headless")]
struct Cli {
    #[command(subcommand)]
    commande: Commande,
}

#[derive(Debug, Subcommand)]
enum Commande {
    /// Affiche l'identité du nœud (la crée au premier lancement) : peerID et QR.
    Identity {
        /// Pseudo, utilisé seulement à la création de l'identité.
        #[arg(long)]
        name: String,
        /// Chemin de la base SQLite (les fichiers d'état s'y accrochent).
        #[arg(long)]
        db: PathBuf,
    },
    /// Rejoint le pair, envoie les messages `--send`, affiche ce qui arrive.
    Run {
        /// Pseudo, utilisé seulement à la création de l'identité.
        #[arg(long)]
        name: String,
        /// Chemin de la base SQLite.
        #[arg(long)]
        db: PathBuf,
        /// QR (`dengon:…`) du pair à rejoindre : `dengon-node identity` chez lui.
        #[arg(long)]
        peer: String,
        /// Message à envoyer dès que le lien est ouvert (répétable).
        #[arg(long = "send")]
        envois: Vec<String>,
        /// S'arrête après N secondes (0 = jusqu'à Ctrl-C).
        #[arg(long, default_value_t = 0)]
        duree: u64,
    },
}

fn main() {
    let cli = Cli::parse();
    let resultat = match cli.commande {
        Commande::Identity { name, db } => identite(&name, &db),
        Commande::Run {
            name,
            db,
            peer,
            envois,
            duree,
        } => lancer(&name, &db, &peer, &envois, duree),
    };
    if let Err(e) = resultat {
        eprintln!("dengon-node : {e}");
        std::process::exit(1);
    }
}

fn identite(name: &str, db: &Path) -> Result<(), String> {
    let noeud = etat::ouvrir_noeud(db, name)?;
    let publique = noeud.public_identity();
    println!("pseudo : {}", publique.pseudo());
    println!("peerID : {}", peer_id_base32(&noeud.peer_id()));
    println!("QR     : {}", publique.to_qr());
    Ok(())
}

#[cfg(feature = "ble")]
fn lancer(name: &str, db: &Path, peer: &str, envois: &[String], duree: u64) -> Result<(), String> {
    use std::time::{Duration, Instant};

    use dengon_ble::BtleplugRadio;
    use dengon_core::api::NodeEvent;
    use dengon_core::identity::PublicIdentity;

    let mut noeud = etat::ouvrir_noeud(db, name)?;
    let contact = PublicIdentity::from_qr(peer).map_err(|e| format!("--peer : {e:?}"))?;
    let pair = contact.peer_id();
    noeud.add_contact(contact);

    let mut session = session::Session::new(noeud, BtleplugRadio::transport(), pair);
    session
        .demarrer()
        .map_err(|e| format!("transport BLE : {e}"))?;
    println!(
        "en écoute du pair {} (rôle central : le pair doit annoncer)…",
        peer_id_base32(&pair)
    );

    let mut a_envoyer: Vec<&String> = envois.iter().rev().collect();
    let fin = (duree > 0).then(|| Instant::now() + Duration::from_secs(duree));

    while fin.is_none_or(|f| Instant::now() < f) {
        for evenement in session.tourner() {
            match evenement {
                NodeEvent::PeerConnected(_) => {
                    println!("lien ouvert");
                    while let Some(texte) = a_envoyer.pop() {
                        match session.envoyer(texte) {
                            Ok(_) => println!("→ {texte}"),
                            Err(e) => eprintln!("envoi refusé : {e}"),
                        }
                    }
                }
                NodeEvent::PeerDisconnected(_) => println!("lien fermé"),
                NodeEvent::MessageReceived(m) => println!("← {}", m.body),
                NodeEvent::StatusChanged(_, statut) => println!("statut : {statut:?}"),
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

#[cfg(not(feature = "ble"))]
fn lancer(
    _name: &str,
    _db: &Path,
    _peer: &str,
    _envois: &[String],
    _duree: u64,
) -> Result<(), String> {
    Err("compilé sans la feature `ble` : aucun transport disponible".into())
}
