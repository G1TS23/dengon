//! Coffre : identité chiffrée au repos (US-205).
//!
//! ```text
//! blob  = "DGID" ‖ version:u8 = 1 ‖ nonce:24 ‖ XChaCha20-Poly1305(clair)
//! clair = secret_static:32 ‖ seed_sign:32 ‖ pseudo_len:u8 ‖ pseudo:utf8
//! AAD   = "DGID" ‖ version
//! ```
//!
//! - La clé du coffre ([`VaultKey`], 32 octets) est fournie par l'appelant :
//!   à terme, tirée du coffre de la plateforme (Android Keystore, Secret
//!   Service, NVS chiffrée). C'est le même principe que `store::KeySource`
//!   (US-207), sans en dépendre : `store` range aussi les secrets de
//!   l'identité (`Store::set_identity`), les deux rangements coexistent en
//!   attendant l'US d'intégration (`docs/suivi/03-ecarts-conception.md`).
//! - L'en-tête est authentifié (AAD) : changer la version ou la signature
//!   de fichier fait échouer le déchiffrement.
//! - Le nonce de 24 octets est tiré de la RNG injectée : XChaCha20 tolère
//!   les nonces aléatoires (2¹⁹² valeurs, pas de risque de collision).
//! - [`Vault`] ne fait que ranger des octets opaques : [`MemoryVault`] (tests,
//!   `no_std`) et [`FileVault`] (`std`, nœud CLI et simulateur).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand_core::{CryptoRng, RngCore};
use zeroize::Zeroizing;

use super::keys::{check_pseudo, Identity};
use super::IdentityError;
use crate::crypto::noise::DH_LEN;
use crate::crypto::SEED_LEN;

/// Longueur de la clé du coffre, en octets.
pub const VAULT_KEY_LEN: usize = 32;

/// Clé symétrique du coffre.
///
/// Simple tableau : le cœur ne la copie pas (elle est prise par référence),
/// et c'est à l'appelant de l'effacer. Le plus simple est de la garder dans un
/// `Zeroizing<VaultKey>`, qui se passe tel quel là où `&VaultKey` est attendu.
pub type VaultKey = [u8; VAULT_KEY_LEN];

/// Signature de fichier du blob.
const MAGIC: &[u8; 4] = b"DGID";

/// Version du format de blob.
const VERSION: u8 = 1;

/// `MAGIC ‖ VERSION`.
const HEADER_LEN: usize = MAGIC.len() + 1;

/// Nonce XChaCha20.
const NONCE_LEN: usize = 24;

/// Tag Poly1305.
const TAG_LEN: usize = 16;

/// Taille du clair hors pseudo : deux secrets + `pseudo_len`.
const PLAIN_FIXED_LEN: usize = DH_LEN + SEED_LEN + 1;

/// Plus petit blob possible (pseudo d'un octet).
const MIN_BLOB_LEN: usize = HEADER_LEN + NONCE_LEN + PLAIN_FIXED_LEN + 1 + TAG_LEN;

fn header() -> [u8; HEADER_LEN] {
    let mut h = [0u8; HEADER_LEN];
    h[..MAGIC.len()].copy_from_slice(MAGIC);
    h[MAGIC.len()] = VERSION;
    h
}

impl Identity {
    /// Chiffre l'identité (secrets + pseudo) pour le stockage.
    ///
    /// Consomme 24 octets de `rng` (nonce) : deux scellements de la même
    /// identité donnent deux blobs différents.
    #[must_use]
    pub fn seal<R>(&self, key: &VaultKey, mut rng: R) -> Vec<u8>
    where
        R: RngCore + CryptoRng,
    {
        let pseudo = self.pseudo().as_bytes();
        let len = u8::try_from(pseudo.len())
            .unwrap_or_else(|_| unreachable!("pseudo validé à la construction"));

        let mut plain = Zeroizing::new(Vec::with_capacity(PLAIN_FIXED_LEN + pseudo.len()));
        plain.extend_from_slice(self.static_keypair().secret());
        plain.extend_from_slice(self.signing_key().to_seed().as_ref());
        plain.push(len);
        plain.extend_from_slice(pseudo);

        let mut nonce = [0u8; NONCE_LEN];
        rng.fill_bytes(&mut nonce);
        let head = header();
        let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
        let ct = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &plain,
                    aad: &head,
                },
            )
            .unwrap_or_else(|_| unreachable!("clair de quelques centaines d'octets"));

        let mut blob = Vec::with_capacity(HEADER_LEN + NONCE_LEN + ct.len());
        blob.extend_from_slice(&head);
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ct);
        blob
    }

    /// Déchiffre un blob produit par [`Identity::seal`].
    ///
    /// # Errors
    ///
    /// - [`IdentityError::VaultFormat`] : blob tronqué, signature de fichier
    ///   inconnue, ou clair mal formé ;
    /// - [`IdentityError::VaultVersion`] : version de format inconnue ;
    /// - [`IdentityError::VaultDecrypt`] : mauvaise clé ou blob altéré.
    pub fn unseal(key: &VaultKey, blob: &[u8]) -> Result<Self, IdentityError> {
        if blob.len() < MIN_BLOB_LEN || &blob[..MAGIC.len()] != MAGIC {
            return Err(IdentityError::VaultFormat);
        }
        if blob[MAGIC.len()] != VERSION {
            return Err(IdentityError::VaultVersion);
        }
        let (head, rest) = blob.split_at(HEADER_LEN);
        let (nonce, ct) = rest.split_at(NONCE_LEN);

        let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
        let plain = Zeroizing::new(
            cipher
                .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad: head })
                .map_err(|_| IdentityError::VaultDecrypt)?,
        );

        // Authentifié mais mal formé : ne peut venir que d'un bogue d'écriture.
        if plain.len() < PLAIN_FIXED_LEN {
            return Err(IdentityError::VaultFormat);
        }
        let (secret, rest) = plain.split_at(DH_LEN);
        let (seed, rest) = rest.split_at(SEED_LEN);
        let (&len, pseudo) = rest.split_first().ok_or(IdentityError::VaultFormat)?;
        if pseudo.len() != usize::from(len) {
            return Err(IdentityError::VaultFormat);
        }
        let pseudo = core::str::from_utf8(pseudo).map_err(|_| IdentityError::VaultFormat)?;
        check_pseudo(pseudo).map_err(|_| IdentityError::VaultFormat)?;

        let mut secret_arr = Zeroizing::new([0u8; DH_LEN]);
        secret_arr.copy_from_slice(secret);
        let mut seed_arr = Zeroizing::new([0u8; SEED_LEN]);
        seed_arr.copy_from_slice(seed);
        Ok(Self::from_parts(
            &secret_arr,
            &seed_arr,
            String::from(pseudo),
        ))
    }
}

/// Emplacement de stockage du blob d'identité (octets opaques, déjà
/// chiffrés).
pub trait Vault {
    /// Lit le blob, ou `None` si le coffre est vide (premier lancement).
    ///
    /// # Errors
    ///
    /// Erreur propre au support (E/S).
    fn load(&self) -> Result<Option<Vec<u8>>, IdentityError>;

    /// Remplace le blob.
    ///
    /// # Errors
    ///
    /// Erreur propre au support (E/S).
    fn save(&mut self, blob: &[u8]) -> Result<(), IdentityError>;
}

/// Charge l'identité du coffre, ou la crée au premier lancement.
///
/// - coffre non vide → déchiffre ; `pseudo` est **ignoré** (le pseudo
///   enregistré fait foi) ;
/// - coffre vide → génère, scelle et enregistre.
///
/// C'est ce qui rend le `peerID` stable entre redémarrages.
///
/// # Errors
///
/// Erreurs de [`Identity::unseal`], de [`Identity::generate`] (pseudo) et du
/// support.
pub fn load_or_create<V, R>(
    vault: &mut V,
    key: &VaultKey,
    pseudo: &str,
    mut rng: R,
) -> Result<Identity, IdentityError>
where
    V: Vault + ?Sized,
    R: RngCore + CryptoRng,
{
    if let Some(blob) = vault.load()? {
        return Identity::unseal(key, &blob);
    }
    let id = Identity::generate(pseudo, &mut rng)?;
    vault.save(&id.seal(key, &mut rng))?;
    Ok(id)
}

/// Coffre en mémoire (tests, simulateur, `no_std`).
#[derive(Clone, Default)]
pub struct MemoryVault {
    blob: Option<Vec<u8>>,
}

impl MemoryVault {
    /// Coffre vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Vault for MemoryVault {
    fn load(&self) -> Result<Option<Vec<u8>>, IdentityError> {
        Ok(self.blob.clone())
    }

    fn save(&mut self, blob: &[u8]) -> Result<(), IdentityError> {
        self.blob = Some(blob.to_vec());
        Ok(())
    }
}

impl fmt::Debug for MemoryVault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryVault")
            .field("blob_len", &self.blob.as_ref().map(Vec::len))
            .finish()
    }
}

/// Coffre fichier (`std`) : un blob par fichier.
///
/// Écriture atomique (fichier temporaire `<chemin>.tmp` puis renommage) : un
/// arrêt brutal ne laisse jamais un coffre à moitié écrit. Sous Unix, le
/// fichier est créé en `0600` et le répertoire est synchronisé après le
/// renommage, pour que celui-ci survive à une coupure de courant.
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct FileVault {
    path: std::path::PathBuf,
}

#[cfg(feature = "std")]
impl FileVault {
    /// Coffre au chemin `path` (le fichier peut ne pas exister encore).
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn tmp_path(&self) -> std::path::PathBuf {
        let mut tmp = self.path.clone().into_os_string();
        tmp.push(".tmp");
        tmp.into()
    }
}

#[cfg(feature = "std")]
impl Vault for FileVault {
    fn load(&self) -> Result<Option<Vec<u8>>, IdentityError> {
        match std::fs::read(&self.path) {
            Ok(blob) => Ok(Some(blob)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(IdentityError::VaultIo(e.kind())),
        }
    }

    fn save(&mut self, blob: &[u8]) -> Result<(), IdentityError> {
        use std::io::Write as _;

        let io = |e: std::io::Error| IdentityError::VaultIo(e.kind());
        let tmp = self.tmp_path();
        // Un `.tmp` laissé par un crash garderait ses droits d'origine :
        // `mode(0o600)` ne s'applique qu'à la création.
        match std::fs::remove_file(&tmp) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(io(e)),
            _ => {}
        }
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            opts.mode(0o600);
        }
        let mut file = opts.open(&tmp).map_err(io)?;
        file.write_all(blob).map_err(io)?;
        file.sync_all().map_err(io)?;
        std::fs::rename(&tmp, &self.path).map_err(io)?;
        // Rend le renommage durable (sous Windows, un répertoire ne s'ouvre
        // pas comme un fichier ; `rename` y passe par `MoveFileEx`).
        #[cfg(unix)]
        {
            let parent = match self.path.parent() {
                Some(p) if !p.as_os_str().is_empty() => p,
                _ => std::path::Path::new("."),
            };
            std::fs::File::open(parent)
                .and_then(|d| d.sync_all())
                .map_err(io)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    const KEY: VaultKey = [0x5A; VAULT_KEY_LEN];

    fn rng(seed: u8) -> ChaCha20Rng {
        ChaCha20Rng::from_seed([seed; 32])
    }

    fn alice() -> Identity {
        Identity::generate("alice", rng(1)).unwrap()
    }

    #[test]
    fn aller_retour_seal_unseal() {
        let id = alice();
        let blob = id.seal(&KEY, rng(2));
        let back = Identity::unseal(&KEY, &blob).unwrap();
        assert_eq!(back.public(), id.public());
        assert_eq!(back.static_keypair().secret(), id.static_keypair().secret());
        assert_eq!(*back.signing_key().to_seed(), *id.signing_key().to_seed());
    }

    #[test]
    fn deux_scellements_different() {
        let id = alice();
        assert_ne!(id.seal(&KEY, rng(2)), id.seal(&KEY, rng(3)));
    }

    #[test]
    fn le_blob_ne_contient_pas_les_secrets() {
        let id = alice();
        let blob = id.seal(&KEY, rng(2));
        let secret = id.static_keypair().secret();
        let seed = id.signing_key().to_seed();
        assert!(!blob.windows(32).any(|w| w == secret));
        assert!(!blob.windows(32).any(|w| w == seed.as_ref()));
        assert!(!blob.windows(5).any(|w| w == b"alice"));
    }

    #[test]
    fn mauvaise_cle_refusee() {
        let blob = alice().seal(&KEY, rng(2));
        assert_eq!(
            Identity::unseal(&[0x00; 32], &blob).unwrap_err(),
            IdentityError::VaultDecrypt
        );
    }

    #[test]
    fn en_tete_invalide_refuse() {
        let blob = alice().seal(&KEY, rng(2));

        let mut magic = blob.clone();
        magic[0] ^= 1;
        assert_eq!(
            Identity::unseal(&KEY, &magic).unwrap_err(),
            IdentityError::VaultFormat
        );

        let mut version = blob.clone();
        version[4] = 2;
        assert_eq!(
            Identity::unseal(&KEY, &version).unwrap_err(),
            IdentityError::VaultVersion
        );

        for n in [0, 5, MIN_BLOB_LEN - 1] {
            assert_eq!(
                Identity::unseal(&KEY, &blob[..n]).unwrap_err(),
                IdentityError::VaultFormat,
                "tronqué à {n}"
            );
        }
    }

    #[test]
    fn clair_mal_forme_refuse() {
        // Blob authentique dont le clair est incohérent : `pseudo_len` ne
        // correspond pas au pseudo.
        let mut plain = [0x11u8; PLAIN_FIXED_LEN + 3].to_vec();
        plain[DH_LEN + SEED_LEN] = 5;
        let nonce = [0u8; NONCE_LEN];
        let head = header();
        let ct = XChaCha20Poly1305::new(Key::from_slice(&KEY))
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &plain,
                    aad: &head,
                },
            )
            .unwrap();
        let blob = [&head[..], &nonce, &ct].concat();
        assert_eq!(
            Identity::unseal(&KEY, &blob).unwrap_err(),
            IdentityError::VaultFormat
        );
    }

    #[test]
    fn load_or_create_est_stable() {
        let mut vault = MemoryVault::new();
        assert!(vault.load().unwrap().is_none());
        let first = load_or_create(&mut vault, &KEY, "alice", rng(1)).unwrap();
        assert!(vault.load().unwrap().is_some());
        // Deuxième lancement : autre RNG, autre pseudo demandé → même identité.
        let second = load_or_create(&mut vault, &KEY, "autre", rng(9)).unwrap();
        assert_eq!(second.peer_id(), first.peer_id());
        assert_eq!(second.pseudo(), "alice");
    }

    #[test]
    fn load_or_create_mauvaise_cle() {
        let mut vault = MemoryVault::new();
        load_or_create(&mut vault, &KEY, "alice", rng(1)).unwrap();
        assert_eq!(
            load_or_create(&mut vault, &[1; 32], "alice", rng(1)).unwrap_err(),
            IdentityError::VaultDecrypt
        );
    }

    #[test]
    fn memory_vault_debug_sans_contenu() {
        let mut vault = MemoryVault::new();
        vault.save(&[1, 2, 3]).unwrap();
        assert_eq!(
            alloc::format!("{vault:?}"),
            "MemoryVault { blob_len: Some(3) }"
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn file_vault_chiffre_sur_disque() {
        let dir = std::env::temp_dir().join(alloc::format!(
            "dengon-us205-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("identity.bin");
        let _ = std::fs::remove_file(&path);

        let mut vault = FileVault::new(&path);
        assert!(vault.load().unwrap().is_none());
        let id = load_or_create(&mut vault, &KEY, "alice", rng(1)).unwrap();

        // « grep binaire » : ni secret, ni pseudo en clair dans le fichier.
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.windows(32).any(|w| w == id.static_keypair().secret()));
        assert!(!bytes.windows(5).any(|w| w == b"alice"));
        assert!(!vault.tmp_path().exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        // Redémarrage : nouvelle instance sur le même fichier.
        let mut reopened = FileVault::new(&path);
        let again = load_or_create(&mut reopened, &KEY, "alice", rng(9)).unwrap();
        assert_eq!(again.peer_id(), id.peer_id());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(feature = "std")]
    #[test]
    fn file_vault_erreur_io() {
        // Un répertoire à la place du fichier : lecture impossible. Pas
        // `temp_dir()` directement : sous Windows il finit par `\` et la
        // lecture rend `NotFound` (coffre vide) au lieu d'une erreur.
        let dir = std::env::temp_dir().join("dengon-vault-est-un-dossier");
        std::fs::create_dir_all(&dir).unwrap();
        let vault = FileVault::new(&dir);
        assert!(matches!(vault.load(), Err(IdentityError::VaultIo(_))));
    }

    #[cfg(all(feature = "std", unix))]
    #[test]
    fn file_vault_tmp_residuel_remplace() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = std::env::temp_dir().join(alloc::format!(
            "dengon-us205-tmp-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("identity.bin");
        let mut vault = FileVault::new(&path);

        // `.tmp` d'un crash précédent, lisible par tous.
        let tmp = vault.tmp_path();
        std::fs::write(&tmp, b"reste d'un crash").unwrap();
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o644)).unwrap();

        load_or_create(&mut vault, &KEY, "alice", rng(1)).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(!tmp.exists());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    proptest! {
        #[test]
        fn aller_retour_pour_tout_pseudo(pseudo in "\\PC{1,60}", seed in any::<[u8; 32]>()) {
            prop_assume!(pseudo.len() <= crate::identity::MAX_PSEUDO_LEN);
            let id = Identity::generate(&pseudo, ChaCha20Rng::from_seed(seed)).unwrap();
            let blob = id.seal(&KEY, ChaCha20Rng::from_seed(seed));
            prop_assert_eq!(Identity::unseal(&KEY, &blob).unwrap().public(), id.public());
        }

        /// Un seul octet modifié, où que ce soit, fait refuser le blob.
        #[test]
        fn octet_altere_refuse(idx in any::<prop::sample::Index>(), bit in 0u8..8) {
            let mut blob = alice().seal(&KEY, rng(2));
            let i = idx.index(blob.len());
            blob[i] ^= 1 << bit;
            prop_assert!(Identity::unseal(&KEY, &blob).is_err());
        }
    }
}
