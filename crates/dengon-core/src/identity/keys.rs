//! Clés, `peerID` et empreinte (`docs/synthese/06-securite.md` §2).

use alloc::string::String;
use core::fmt;

use rand_core::{CryptoRng, RngCore};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::IdentityError;
use crate::crypto::noise::DH_LEN;
use crate::crypto::{SigningKey, StaticKeypair, VerifyingKey, PUBLIC_KEY_LEN, SEED_LEN};
use crate::protocol::consts::PEER_ID_LEN;
use crate::protocol::types::PeerId;

/// Longueur de l'empreinte, en octets.
pub const FINGERPRINT_LEN: usize = 32;

/// Empreinte : `SHA-256(pub_static ‖ pub_sign)`.
pub type Fingerprint = [u8; FINGERPRINT_LEN];

/// Longueur maximale du pseudo, en octets : il est préfixé d'un `u8` dans le
/// QR (`pseudo_len`).
pub const MAX_PSEUDO_LEN: usize = u8::MAX as usize;

/// Vérifie qu'un pseudo tient dans le QR : 1 à [`MAX_PSEUDO_LEN`] octets.
pub(crate) fn check_pseudo(pseudo: &str) -> Result<(), IdentityError> {
    if pseudo.is_empty() || pseudo.len() > MAX_PSEUDO_LEN {
        return Err(IdentityError::InvalidPseudo);
    }
    Ok(())
}

/// `peerID = SHA-256(pub_static)[0..8]`.
///
/// `pub(crate)` (pas privée) : `api.rs` en a besoin pour dériver le `PeerId`
/// de l'expéditeur d'une enveloppe scellée, connu seulement par sa clé
/// statique (voir `api::handle_sealed_envelope`). Avant ce correctif (revue
/// PR #102), `api.rs` réimplémentait cette formule à la main
/// (`peer_id_of_pub_static`) faute d'accès à celle-ci — deux copies à tenir
/// manuellement synchronisées, sans qu'un futur changement de l'une déclenche
/// une erreur de compilation dans l'autre.
pub(crate) fn peer_id_of(pub_static: &[u8; DH_LEN]) -> PeerId {
    let digest = Sha256::digest(pub_static);
    let mut id = [0u8; PEER_ID_LEN];
    id.copy_from_slice(&digest[..PEER_ID_LEN]);
    id
}

/// `fingerprint = SHA-256(pub_static ‖ pub_sign)`.
fn fingerprint_of(pub_static: &[u8; DH_LEN], pub_sign: &[u8; PUBLIC_KEY_LEN]) -> Fingerprint {
    let mut h = Sha256::new();
    h.update(pub_static);
    h.update(pub_sign);
    h.finalize().into()
}

/// `peerID` affiché : base32 RFC 4648, minuscules, sans padding (13
/// caractères pour 8 octets).
#[must_use]
pub fn peer_id_base32(peer_id: &PeerId) -> String {
    data_encoding::BASE32_NOPAD
        .encode(peer_id)
        .to_ascii_lowercase()
}

/// Identité locale : les deux paires de clés et le pseudo.
///
/// `Debug` n'affiche que les parties publiques ; les secrets sont effacés à
/// la destruction (`StaticKeypair`, `SigningKey`). Pas de `Clone`, comme
/// `SigningKey` (revue #78) : une copie serait un exemplaire de plus des
/// secrets en mémoire.
pub struct Identity {
    static_kp: StaticKeypair,
    sign: SigningKey,
    pseudo: String,
}

impl Identity {
    /// Génère une nouvelle identité (« premier lancement » ou « nouvelle
    /// identité »).
    ///
    /// Consomme 64 octets de `rng` : 32 pour le secret X25519, puis 32 pour
    /// la graine Ed25519.
    ///
    /// # Errors
    ///
    /// [`IdentityError::InvalidPseudo`] si le pseudo est vide ou dépasse
    /// [`MAX_PSEUDO_LEN`] octets.
    pub fn generate<R>(pseudo: &str, mut rng: R) -> Result<Self, IdentityError>
    where
        R: RngCore + CryptoRng,
    {
        check_pseudo(pseudo)?;
        let mut secret = Zeroizing::new([0u8; DH_LEN]);
        let mut seed = Zeroizing::new([0u8; SEED_LEN]);
        rng.fill_bytes(secret.as_mut());
        rng.fill_bytes(seed.as_mut());
        Ok(Self::from_parts(&secret, &seed, String::from(pseudo)))
    }

    /// Reconstruit une identité depuis ses secrets (coffre). Le pseudo doit
    /// avoir été validé par l'appelant.
    pub(crate) fn from_parts(secret: &[u8; DH_LEN], seed: &[u8; SEED_LEN], pseudo: String) -> Self {
        Self {
            static_kp: StaticKeypair::from_secret(*secret),
            sign: SigningKey::from_seed(seed),
            pseudo,
        }
    }

    /// Pseudo (libre, non vérifié : l'identité réelle est la clé).
    #[must_use]
    pub fn pseudo(&self) -> &str {
        &self.pseudo
    }

    /// Paire X25519, pour les handshakes Noise `XX` et l'ouverture des
    /// enveloppes `X`.
    #[must_use]
    pub fn static_keypair(&self) -> &StaticKeypair {
        &self.static_kp
    }

    /// Clé de signature Ed25519.
    #[must_use]
    pub fn signing_key(&self) -> &SigningKey {
        &self.sign
    }

    /// `peerID` : stable tant que le coffre survit.
    #[must_use]
    pub fn peer_id(&self) -> PeerId {
        peer_id_of(&self.static_kp.public())
    }

    /// Empreinte, base du code de vérification.
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        fingerprint_of(
            &self.static_kp.public(),
            &self.sign.verifying_key().to_bytes(),
        )
    }

    /// Carte de contact publique (ce que contient le QR).
    #[must_use]
    pub fn public(&self) -> PublicIdentity {
        PublicIdentity {
            pseudo: self.pseudo.clone(),
            pub_static: self.static_kp.public(),
            pub_sign: self.sign.verifying_key(),
        }
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity")
            .field("pseudo", &self.pseudo)
            .field("static_kp", &self.static_kp)
            .field("sign", &self.sign.verifying_key())
            .finish_non_exhaustive()
    }
}

/// Carte de contact : pseudo + clés publiques. Aucun secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicIdentity {
    pseudo: String,
    pub_static: [u8; DH_LEN],
    pub_sign: VerifyingKey,
}

impl PublicIdentity {
    /// Construit une carte de contact.
    ///
    /// La clé X25519 n'est pas validée : les 32 octets sont toujours une
    /// coordonnée acceptée par X25519 (RFC 7748).
    ///
    /// # Errors
    ///
    /// [`IdentityError::InvalidPseudo`] si le pseudo est vide ou trop long.
    pub fn new(
        pseudo: &str,
        pub_static: [u8; DH_LEN],
        pub_sign: VerifyingKey,
    ) -> Result<Self, IdentityError> {
        check_pseudo(pseudo)?;
        Ok(Self {
            pseudo: String::from(pseudo),
            pub_static,
            pub_sign,
        })
    }

    /// Pseudo annoncé par le contact.
    #[must_use]
    pub fn pseudo(&self) -> &str {
        &self.pseudo
    }

    /// Clé statique publique X25519.
    #[must_use]
    pub fn pub_static(&self) -> [u8; DH_LEN] {
        self.pub_static
    }

    /// Clé publique Ed25519.
    #[must_use]
    pub fn pub_sign(&self) -> VerifyingKey {
        self.pub_sign
    }

    /// `peerID` du contact.
    #[must_use]
    pub fn peer_id(&self) -> PeerId {
        peer_id_of(&self.pub_static)
    }

    /// Empreinte du contact.
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        fingerprint_of(&self.pub_static, &self.pub_sign.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    fn rng(seed: u8) -> ChaCha20Rng {
        ChaCha20Rng::from_seed([seed; 32])
    }

    #[test]
    fn peer_id_et_empreinte_recalcules_a_la_main() {
        let id = Identity::generate("alice", rng(1)).unwrap();
        let pub_static = id.static_keypair().public();
        let pub_sign = id.signing_key().verifying_key().to_bytes();

        let h = Sha256::digest(pub_static);
        assert_eq!(id.peer_id(), h[..8]);

        let mut cat = [0u8; 64];
        cat[..32].copy_from_slice(&pub_static);
        cat[32..].copy_from_slice(&pub_sign);
        assert_eq!(id.fingerprint()[..], Sha256::digest(cat)[..]);

        // Identité et carte publique donnent les mêmes valeurs.
        assert_eq!(id.public().peer_id(), id.peer_id());
        assert_eq!(id.public().fingerprint(), id.fingerprint());
    }

    #[test]
    fn generation_deterministe_pour_une_rng_donnee() {
        let a = Identity::generate("alice", rng(7)).unwrap();
        let b = Identity::generate("alice", rng(7)).unwrap();
        let c = Identity::generate("alice", rng(8)).unwrap();
        assert_eq!(a.public(), b.public());
        assert_ne!(a.peer_id(), c.peer_id());
    }

    #[test]
    fn pseudo_vide_ou_trop_long_refuse() {
        assert_eq!(
            Identity::generate("", rng(1)).unwrap_err(),
            IdentityError::InvalidPseudo
        );
        let long = "é".repeat(128); // 256 octets
        assert_eq!(
            Identity::generate(&long, rng(1)).unwrap_err(),
            IdentityError::InvalidPseudo
        );
        let max = "a".repeat(MAX_PSEUDO_LEN);
        assert!(Identity::generate(&max, rng(1)).is_ok());
    }

    #[test]
    fn peer_id_en_base32() {
        let id = [0u8; PEER_ID_LEN];
        assert_eq!(peer_id_base32(&id), "aaaaaaaaaaaaa");
        // RFC 4648 §10 : BASE32("foobar") = "MZXW6YTBOI======".
        let mut foobar = [0u8; PEER_ID_LEN];
        foobar[..6].copy_from_slice(b"foobar");
        assert!(peer_id_base32(&foobar).starts_with("mzxw6ytboi"));
        assert_eq!(peer_id_base32(&foobar).len(), 13);
    }

    #[test]
    fn debug_n_affiche_aucun_secret() {
        let mut r = rng(3);
        let mut secret = [0u8; 32];
        r.fill_bytes(&mut secret);
        let id = Identity::generate("alice", rng(3)).unwrap();
        let dbg = format!("{id:?}");
        let secret_hex: String = secret.iter().map(|b| format!("{b:02x}")).collect();
        assert!(!dbg.contains(&secret_hex));
        assert!(dbg.contains("alice"));
    }
}
