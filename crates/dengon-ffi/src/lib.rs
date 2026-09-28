//! `dengon-ffi` — surface FFI de dengon exposée via UniFFI.
//!
//! Génère les bindings Kotlin consommés par l'application Android ; Swift est
//! reporté en v2 (`docs/synthese/04-architecture.md` §5).
//!
//! Contrat : `src/dengon.udl`. v0 gelé par l'US-106, étendu en v1 par
//! l'US-302 (constructeur `open` avec coffre, chemin des octets radio) —
//! l'écart est consigné dans `docs/suivi/03-ecarts-conception.md`.
//!
//! Depuis l'US-302, **plus aucun bouchon** : chaque appel est une fine couche
//! de conversion au-dessus de [`dengon_core::api::Node`] (US-301) et de
//! [`dengon_core::identity`] (US-205). Tout ce qui est protocole, crypto et
//! routage vit dans `dengon-core` ; ce fichier ne fait que :
//! - traduire les identifiants binaires en chaînes (module [`convert`]) ;
//! - fournir l'horloge ([`Now`]) et l'aléa (`OsRng`), que le cœur `no_std`
//!   attend de l'appelant ;
//! - rendre le nœud partageable entre threads (`Mutex`) : l'UI et le service
//!   de premier plan l'appellent depuis des threads différents.
//!
//! # Note sur `unsafe`
//!
//! UniFFI génère du code `unsafe`. Le lint `unsafe_code = "deny"` défini dans
//! `[workspace.lints.rust]` est donc neutralisé ici par
//! `#![allow(unsafe_code)]`. C'est précisément pour cela qu'il est en `deny`
//! et non en `forbid`, qui serait inviolable.

#![allow(unsafe_code)]
// Le scaffolding généré par `uniffi::include_scaffolding!` ci-dessous déclenche
// ces deux lints ; ils s'appliquent à toute la crate car le code généré est
// inclus tel quel (pas de fichier séparé sur lequel cibler l'allow).
#![allow(unused_qualifications, clippy::empty_line_after_doc_comments)]

mod convert;

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use dengon_core::api;
use dengon_core::identity::{self as core_identity, FileVault, VaultKey};
use dengon_core::sync::routing::Now;
use rand_core::{OsRng, RngCore};

uniffi::include_scaffolding!("dengon");

/// Nom du coffre d'identité dans le `data_dir` passé à [`DengonNode::open`].
const VAULT_FILE: &str = "identity.vault";

/// Version de la surface FFI, destinée à être exposée aux plateformes hôtes.
pub fn version() -> String {
    format!(
        "{} (protocole v{})",
        env!("CARGO_PKG_VERSION"),
        dengon_core::PROTOCOL_VERSION
    )
}

// ---------------------------------------------------------------------------
// Types du contrat (miroirs Rust de `dengon.udl`).
// ---------------------------------------------------------------------------

/// Carte de contact d'un nœud : `peer_id` + pseudo + clés publiques, aucun
/// secret. Miroir FFI de [`dengon_core::identity::PublicIdentity`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub peer_id: String,
    pub pseudo: String,
    pub pub_static: Vec<u8>,
    pub pub_sign: Vec<u8>,
}

/// Statuts MVP du cycle de vie d'un message
/// (`docs/synthese/07-cycle-de-vie-et-statuts.md` §1).
///
/// `Read` et `Cancelled` restent dans le contrat mais ne sont jamais émis par
/// la façade de l'US-301 (pas de `mark_read` ni de `cancel_message`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStatus {
    Queued,
    InFlight,
    Delivered,
    Read,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub msg_uuid: String,
    pub conv_id: String,
    pub author_peer_id: String,
    pub body: String,
    pub outgoing: bool,
    pub sent_ms: i64,
    pub status: MessageStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub conv_id: String,
    pub peer_id: String,
    pub peer_pseudo: String,
    pub last_message: Option<Message>,
    pub unread_count: u32,
}

/// Trame à écrire sur le lien radio du pair `peer_id` (v1, US-302).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingFrame {
    pub peer_id: String,
    pub frame: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeEvent {
    MessageReceived {
        message: Message,
    },
    StatusChanged {
        msg_uuid: String,
        status: MessageStatus,
    },
    PeerConnected {
        peer_id: String,
    },
    PeerDisconnected {
        peer_id: String,
    },
}

/// Erreurs de la surface FFI, gelées à trois variantes sans charge utile.
///
/// - `UnknownPeer` : correspondant jamais vu, ou `peer_id` mal formé ;
/// - `NotConnected` : reprise telle quelle de [`api::DengonError`] ;
/// - `Internal` : tout le reste (identité ou QR invalide, coffre illisible,
///   crypto). Le détail n'est volontairement pas exposé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DengonError {
    UnknownPeer,
    NotConnected,
    Internal,
}

impl std::fmt::Display for DengonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DengonError::UnknownPeer => write!(f, "pair inconnu"),
            DengonError::NotConnected => write!(f, "aucun pair connecté"),
            DengonError::Internal => write!(f, "erreur interne"),
        }
    }
}

impl std::error::Error for DengonError {}

impl From<api::DengonError> for DengonError {
    fn from(value: api::DengonError) -> Self {
        match value {
            api::DengonError::UnknownPeer => Self::UnknownPeer,
            api::DengonError::NotConnected => Self::NotConnected,
            api::DengonError::Internal => Self::Internal,
        }
    }
}

impl From<core_identity::IdentityError> for DengonError {
    fn from(_: core_identity::IdentityError) -> Self {
        Self::Internal
    }
}

// ---------------------------------------------------------------------------
// `DengonNode` — le vrai nœud, derrière un verrou.
// ---------------------------------------------------------------------------

/// Nœud dengon de cet appareil : [`api::Node`] rendu partageable entre
/// threads, plus l'horloge et l'aléa qu'il attend de l'appelant.
#[derive(Debug)]
pub struct DengonNode {
    node: Mutex<api::Node>,
    /// Origine de l'horloge monotone passée au routeur.
    origin: Instant,
}

impl DengonNode {
    /// Ouvre le nœud : charge l'identité depuis `<data_dir>/identity.vault`,
    /// ou la crée au premier lancement (`pseudo` n'est lu qu'à ce moment-là).
    ///
    /// # Errors
    ///
    /// [`DengonError::Internal`] si `vault_key` ne fait pas 32 octets, si le
    /// répertoire ne peut pas être créé, si le pseudo est invalide ou si le
    /// coffre ne se déchiffre pas (mauvaise clé ou fichier altéré).
    pub fn open(data_dir: String, vault_key: Vec<u8>, pseudo: String) -> Result<Self, DengonError> {
        let key: VaultKey = vault_key
            .as_slice()
            .try_into()
            .map_err(|_| DengonError::Internal)?;
        let data_dir = PathBuf::from(data_dir);
        std::fs::create_dir_all(&data_dir).map_err(|_| DengonError::Internal)?;
        let mut vault = FileVault::new(data_dir.join(VAULT_FILE));
        let identity = core_identity::load_or_create(&mut vault, &key, &pseudo, OsRng)?;

        // Graine du jitter de relais : pas un secret (voir `api::Node::new`),
        // mais différente d'un nœud à l'autre pour décorréler les relais.
        let routing_seed = OsRng.next_u64();
        Ok(Self {
            node: Mutex::new(api::Node::new(identity, routing_seed)),
            origin: Instant::now(),
        })
    }

    pub fn local_identity(&self) -> Identity {
        convert::identity_to_ffi(&self.lock().public_identity())
    }

    pub fn add_contact(&self, contact: Identity) -> Result<(), DengonError> {
        let contact = convert::identity_from_ffi(&contact)?;
        self.lock().add_contact(contact);
        Ok(())
    }

    pub fn send_message(&self, dest_peer_id: String, body: String) -> Result<String, DengonError> {
        let dest = convert::peer_id_from_str(&dest_peer_id)?;
        let now = self.now();
        let msg_uuid = self.lock().send_message(dest, &body, now, OsRng)?;
        Ok(convert::to_hex(&msg_uuid))
    }

    pub fn poll_events(&self) -> Vec<NodeEvent> {
        let now = self.now();
        self.lock()
            .poll_events(now)
            .into_iter()
            .map(convert::event_to_ffi)
            .collect()
    }

    pub fn on_peer_connected(&self, peer_id: String) -> Result<(), DengonError> {
        let peer_id = convert::peer_id_from_str(&peer_id)?;
        let now = self.now();
        self.lock().on_peer_connected(peer_id, now, OsRng);
        Ok(())
    }

    pub fn on_peer_disconnected(&self, peer_id: String) -> Result<(), DengonError> {
        let peer_id = convert::peer_id_from_str(&peer_id)?;
        self.lock().on_peer_disconnected(peer_id);
        Ok(())
    }

    pub fn on_bytes_received(&self, peer_id: String, frame: Vec<u8>) -> Result<(), DengonError> {
        let peer_id = convert::peer_id_from_str(&peer_id)?;
        let now = self.now();
        self.lock().on_bytes_received(peer_id, &frame, now);
        Ok(())
    }

    pub fn take_outgoing(&self) -> Vec<OutgoingFrame> {
        self.lock()
            .take_outgoing()
            .into_iter()
            .map(|(peer_id, frame)| OutgoingFrame {
                peer_id: convert::peer_id_to_string(&peer_id),
                frame,
            })
            .collect()
    }

    pub fn list_conversations(&self) -> Vec<Conversation> {
        self.lock()
            .list_conversations()
            .into_iter()
            .map(convert::conversation_to_ffi)
            .collect()
    }

    /// Liste vide pour un `conv_id` inconnu **ou mal formé** : le contrat ne
    /// prévoit pas d'erreur ici, et aucune conversation ne peut avoir un
    /// identifiant qui ne se décode pas.
    pub fn list_messages(&self, conv_id: String) -> Vec<Message> {
        let Some(conv_id) = convert::from_hex::<8>(&conv_id) else {
            return Vec::new();
        };
        self.lock()
            .list_messages(conv_id)
            .into_iter()
            .map(convert::message_to_ffi)
            .collect()
    }

    fn now(&self) -> Now {
        let wall_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        let mono_ms = u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX);
        Now::new(wall_ms, mono_ms)
    }

    /// Un mutex empoisonné (panique pendant qu'il était tenu) ne doit pas
    /// faire perdre les données déjà écrites : on récupère le contenu plutôt
    /// que de propager la panique à chaque appel suivant.
    fn lock(&self) -> MutexGuard<'_, api::Node> {
        self.node.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

// ---------------------------------------------------------------------------
// Identité + QR (fonctions libres du namespace `dengon`).
// ---------------------------------------------------------------------------

/// Identité **jetable** : vraies clés, mais les secrets sont perdus au retour
/// (seule la carte publique franchit le FFI). Pour les tests ; l'app passe
/// par [`DengonNode::open`].
///
/// # Errors
///
/// [`DengonError::Internal`] si le pseudo est vide ou trop long.
pub fn generate_identity(pseudo: String) -> Result<Identity, DengonError> {
    let identity = core_identity::Identity::generate(&pseudo, OsRng)?;
    Ok(convert::identity_to_ffi(&identity.public()))
}

/// Format `dengon:v1:…` de `docs/synthese/06-securite.md` §2.
///
/// # Errors
///
/// [`DengonError::Internal`] si `identity` n'est pas une carte valide.
pub fn identity_qr_code(identity: Identity) -> Result<String, DengonError> {
    Ok(convert::identity_from_ffi(&identity)?.to_qr())
}

/// # Errors
///
/// [`DengonError::Internal`] pour tout QR qui n'est pas une carte dengon v1
/// valide (y compris un QR étranger : URL, menu…).
pub fn identity_from_qr_code(qr_code: String) -> Result<Identity, DengonError> {
    let identity = core_identity::PublicIdentity::from_qr(&qr_code)?;
    Ok(convert::identity_to_ffi(&identity))
}

/// Code de vérification 60 chiffres (SHA-512 des deux empreintes), symétrique.
///
/// # Errors
///
/// [`DengonError::Internal`] si l'une des deux cartes est invalide.
pub fn verification_code(local: Identity, remote: Identity) -> Result<String, DengonError> {
    let local = convert::identity_from_ffi(&local)?;
    let remote = convert::identity_from_ffi(&remote)?;
    Ok(core_identity::verification_code(&local, &remote).to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    const KEY: [u8; 32] = [7; 32];

    /// Répertoire temporaire propre au test (pas de dépendance `tempfile`
    /// pour si peu) ; nettoyé à la destruction.
    struct Dossier(PathBuf);

    impl Dossier {
        fn nouveau() -> Self {
            static COMPTEUR: AtomicU32 = AtomicU32::new(0);
            let n = COMPTEUR.fetch_add(1, Ordering::Relaxed);
            let chemin =
                std::env::temp_dir().join(format!("dengon-ffi-test-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&chemin);
            Self(chemin)
        }

        fn chemin(&self) -> String {
            self.0.to_string_lossy().into_owned()
        }
    }

    impl Drop for Dossier {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn ouvrir(dossier: &Dossier, pseudo: &str) -> DengonNode {
        DengonNode::open(dossier.chemin(), KEY.to_vec(), pseudo.into()).unwrap()
    }

    /// Recopie les trames de chacun vers l'autre jusqu'au silence : c'est le
    /// rôle que tiendra `AndroidTransport` (US-213) sur une vraie radio.
    fn pomper(a: &DengonNode, b: &DengonNode) {
        let id_a = a.local_identity().peer_id;
        let id_b = b.local_identity().peer_id;
        for _ in 0..32 {
            let de_a = a.take_outgoing();
            let de_b = b.take_outgoing();
            if de_a.is_empty() && de_b.is_empty() {
                return;
            }
            for f in de_a {
                assert_eq!(f.peer_id, id_b);
                b.on_bytes_received(id_a.clone(), f.frame).unwrap();
            }
            for f in de_b {
                assert_eq!(f.peer_id, id_a);
                a.on_bytes_received(id_b.clone(), f.frame).unwrap();
            }
        }
        panic!("les deux nœuds échangent encore après 32 tours");
    }

    /// Alice et Bob appairés (contacts croisés) et connectés, handshake fini.
    fn paire(da: &Dossier, db: &Dossier) -> (DengonNode, DengonNode) {
        let alice = ouvrir(da, "alice");
        let bob = ouvrir(db, "bob");
        alice.add_contact(bob.local_identity()).unwrap();
        bob.add_contact(alice.local_identity()).unwrap();
        alice
            .on_peer_connected(bob.local_identity().peer_id)
            .unwrap();
        bob.on_peer_connected(alice.local_identity().peer_id)
            .unwrap();
        pomper(&alice, &bob);
        let _ = alice.poll_events();
        let _ = bob.poll_events();
        (alice, bob)
    }

    #[test]
    fn message_de_bout_en_bout_entre_deux_noeuds() {
        let (da, db) = (Dossier::nouveau(), Dossier::nouveau());
        let (alice, bob) = paire(&da, &db);
        let id_alice = alice.local_identity().peer_id;
        let id_bob = bob.local_identity().peer_id;

        let msg_uuid = alice
            .send_message(id_bob.clone(), "bonjour".into())
            .unwrap();
        assert_eq!(msg_uuid.len(), 32, "16 octets en hexadécimal");
        pomper(&alice, &bob);

        let recu = bob
            .poll_events()
            .into_iter()
            .find_map(|e| match e {
                NodeEvent::MessageReceived { message } => Some(message),
                _ => None,
            })
            .expect("Bob reçoit le message");
        assert_eq!(recu.body, "bonjour");
        assert_eq!(recu.author_peer_id, id_alice);
        assert!(!recu.outgoing);

        let statuts: Vec<_> = alice
            .poll_events()
            .into_iter()
            .filter_map(|e| match e {
                NodeEvent::StatusChanged {
                    msg_uuid: u,
                    status,
                } if u == msg_uuid => Some(status),
                _ => None,
            })
            .collect();
        assert!(
            statuts.contains(&MessageStatus::InFlight),
            "statuts vus : {statuts:?}"
        );

        // Même `conv_id` des deux côtés, et le message est listé.
        let conv_alice = alice.list_conversations();
        let conv_bob = bob.list_conversations();
        assert_eq!(conv_alice.len(), 1);
        assert_eq!(conv_alice[0].conv_id, conv_bob[0].conv_id);
        assert_eq!(conv_alice[0].peer_id, id_bob);
        let messages = bob.list_messages(conv_bob[0].conv_id.clone());
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].msg_uuid, msg_uuid);
    }

    #[test]
    fn evenements_de_connexion_et_deconnexion() {
        let (da, db) = (Dossier::nouveau(), Dossier::nouveau());
        let alice = ouvrir(&da, "alice");
        let bob = ouvrir(&db, "bob");
        let id_bob = bob.local_identity().peer_id;

        alice.on_peer_connected(id_bob.clone()).unwrap();
        alice.on_peer_disconnected(id_bob.clone()).unwrap();
        assert_eq!(
            alice.poll_events(),
            vec![
                NodeEvent::PeerConnected {
                    peer_id: id_bob.clone()
                },
                NodeEvent::PeerDisconnected { peer_id: id_bob },
            ]
        );
    }

    #[test]
    fn identite_stable_d_une_ouverture_a_l_autre() {
        let dossier = Dossier::nouveau();
        let premiere = ouvrir(&dossier, "alice").local_identity();
        // Le pseudo passé à la réouverture est ignoré : le coffre fait foi.
        let seconde = ouvrir(&dossier, "autre").local_identity();
        assert_eq!(premiere, seconde);
        assert_eq!(seconde.pseudo, "alice");
    }

    #[test]
    fn mauvaise_cle_de_coffre_refusee() {
        let dossier = Dossier::nouveau();
        drop(ouvrir(&dossier, "alice"));
        let err = DengonNode::open(dossier.chemin(), vec![8; 32], "alice".into()).unwrap_err();
        assert_eq!(err, DengonError::Internal);
        let err = DengonNode::open(dossier.chemin(), vec![7; 31], "alice".into()).unwrap_err();
        assert_eq!(err, DengonError::Internal);
    }

    #[test]
    fn envoi_a_un_inconnu_refuse() {
        let dossier = Dossier::nouveau();
        let alice = ouvrir(&dossier, "alice");
        let inconnu = generate_identity("carol".into()).unwrap().peer_id;
        assert_eq!(
            alice.send_message(inconnu, "x".into()),
            Err(DengonError::UnknownPeer)
        );
    }

    #[test]
    fn peer_id_mal_forme_refuse() {
        let dossier = Dossier::nouveau();
        let alice = ouvrir(&dossier, "alice");
        for mauvais in ["", "peer-canned", "aaaaaaaaaaaaaa", "AAAAAAAAAAAA!"] {
            assert_eq!(
                alice.on_peer_connected(mauvais.into()),
                Err(DengonError::UnknownPeer),
                "{mauvais:?}"
            );
        }
        assert!(alice.list_messages("pas-hexa".into()).is_empty());
    }

    #[test]
    fn qr_aller_retour() {
        let alice = generate_identity("alice".into()).unwrap();
        let qr = identity_qr_code(alice.clone()).unwrap();
        assert!(qr.starts_with("dengon:v1:"));
        assert_eq!(identity_from_qr_code(qr).unwrap(), alice);
        assert_eq!(
            identity_from_qr_code("https://example.org".into()),
            Err(DengonError::Internal)
        );
    }

    #[test]
    fn carte_au_peer_id_falsifie_refusee() {
        let alice = generate_identity("alice".into()).unwrap();
        let bob = generate_identity("bob".into()).unwrap();
        let falsifiee = Identity {
            peer_id: bob.peer_id,
            ..alice
        };
        assert_eq!(identity_qr_code(falsifiee), Err(DengonError::Internal));
    }

    #[test]
    fn code_de_verification_symetrique_60_chiffres() {
        let alice = generate_identity("alice".into()).unwrap();
        let bob = generate_identity("bob".into()).unwrap();
        let ab = verification_code(alice.clone(), bob.clone()).unwrap();
        assert_eq!(ab, verification_code(bob, alice).unwrap());
        let groupes: Vec<_> = ab.split(' ').collect();
        assert_eq!(groupes.len(), 12);
        assert!(groupes
            .iter()
            .all(|g| g.len() == 5 && g.bytes().all(|b| b.is_ascii_digit())));
    }

    #[test]
    fn pseudo_vide_refuse() {
        assert_eq!(generate_identity(String::new()), Err(DengonError::Internal));
    }
}
