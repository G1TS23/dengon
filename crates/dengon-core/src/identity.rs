//! Identité d'un nœud dengon (US-205, `docs/synthese/06-securite.md` §2).
//!
//! - [`Identity`] : paire de clés `static` (X25519, Noise) + `sign`
//!   (Ed25519) + pseudo, avec `peerID` et empreinte ([`keys`]) ;
//! - [`PublicIdentity`] : la carte de contact, sans secret, échangée par QR
//!   (`dengon:v1:…`, [`qr`]) ;
//! - [`SafetyNumber`] : code de vérification de 60 chiffres, identique chez
//!   les deux correspondants ([`safety`]) ;
//! - [`Vault`] : coffre où l'identité est rangée **chiffrée au repos**
//!   (XChaCha20-Poly1305, [`vault`]).
//!
//! ```text
//! peerID      = SHA-256(pub_static)[0..8]          // affiché en base32
//! fingerprint = SHA-256(pub_static ‖ pub_sign)     // base de la vérification
//! ```
//!
//! # Ce que ce module ne fait pas
//!
//! - **Pas d'aléa propre.** Comme `crypto::noise`, la génération et le
//!   scellement prennent une RNG fournie par l'appelant (`no_std`/ESP32 :
//!   `esp_fill_random`, hôte : `OsRng`).
//! - **Pas de coffre plateforme.** La clé du coffre (32 octets) est fournie
//!   par l'appelant ; Android Keystore / Secret Service viendront derrière
//!   [`Vault`] dans `dengon-ffi` / `dengon-node`. Voir
//!   `docs/suivi/03-ecarts-conception.md`.
//! - **Pas de décision TOFU.** Comparer une clé reçue au contact connu et
//!   alerter en cas de changement relève de `store` / `sync`.
//! - **Pas d'image QR.** « QR » désigne ici la chaîne encodée ; le rendu et le
//!   scan sont côté UI (US-215).

use core::fmt;

pub mod keys;
pub mod qr;
pub mod safety;
pub mod vault;

pub use keys::{
    peer_id_base32, Fingerprint, Identity, PublicIdentity, FINGERPRINT_LEN, MAX_PSEUDO_LEN,
};
pub use qr::QR_PREFIX;
pub use safety::{safety_number, verification_code, SafetyNumber, SAFETY_DIGITS, SAFETY_GROUPS};
#[cfg(feature = "std")]
pub use vault::FileVault;
pub use vault::{load_or_create, MemoryVault, Vault, VaultKey, VAULT_KEY_LEN};

/// Erreur du module `identity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    /// Pseudo vide, trop long (> [`MAX_PSEUDO_LEN`] octets) ou UTF-8 invalide.
    InvalidPseudo,
    /// La chaîne ne commence pas par `dengon:` : ce n'est pas un QR dengon.
    QrPrefix,
    /// QR dengon d'une version non prise en charge (autre que `v1`).
    QrVersion,
    /// Charge utile du QR qui n'est pas du base64url canonique sans padding.
    QrEncoding,
    /// Charge utile du QR de longueur incohérente avec `pseudo_len`.
    QrLength,
    /// `pub_sign` du QR qui n'est pas une clé publique Ed25519 valide.
    QrPublicKey,
    /// Blob de coffre tronqué, d'en-tête inconnu ou de contenu mal formé.
    VaultFormat,
    /// Blob de coffre d'une version de format non prise en charge.
    VaultVersion,
    /// Déchiffrement du coffre refusé : mauvaise clé ou blob altéré. Les deux
    /// causes ne sont volontairement pas distinguées.
    VaultDecrypt,
    /// Erreur d'entrée/sortie du coffre fichier.
    #[cfg(feature = "std")]
    VaultIo(std::io::ErrorKind),
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPseudo => f.write_str("pseudo invalide (1 à 255 octets UTF-8)"),
            Self::QrPrefix => f.write_str("QR non dengon (préfixe « dengon: » absent)"),
            Self::QrVersion => f.write_str("version de QR dengon non prise en charge"),
            Self::QrEncoding => f.write_str("QR : base64url invalide"),
            Self::QrLength => f.write_str("QR : longueur incohérente"),
            Self::QrPublicKey => f.write_str("QR : clé publique Ed25519 invalide"),
            Self::VaultFormat => f.write_str("coffre : format invalide"),
            Self::VaultVersion => f.write_str("coffre : version de format non prise en charge"),
            Self::VaultDecrypt => f.write_str("coffre : mauvaise clé ou contenu altéré"),
            #[cfg(feature = "std")]
            Self::VaultIo(kind) => write!(f, "coffre : erreur d'E/S ({kind})"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for IdentityError {}

#[cfg(test)]
mod tests {
    use super::IdentityError;
    use alloc::string::ToString;

    #[test]
    fn chaque_erreur_a_un_message() {
        let erreurs = [
            IdentityError::InvalidPseudo,
            IdentityError::QrPrefix,
            IdentityError::QrVersion,
            IdentityError::QrEncoding,
            IdentityError::QrLength,
            IdentityError::QrPublicKey,
            IdentityError::VaultFormat,
            IdentityError::VaultVersion,
            IdentityError::VaultDecrypt,
            #[cfg(feature = "std")]
            IdentityError::VaultIo(std::io::ErrorKind::NotFound),
        ];
        for e in erreurs {
            assert!(!e.to_string().is_empty(), "{e:?}");
        }
    }
}
