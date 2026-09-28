//! QR de contact (`docs/synthese/09-dashboard-et-donnees.md` §11.3).
//!
//! ```text
//! dengon:v1:<base64url( pseudo_len:u8 ‖ pseudo:utf8 ‖ pub_static:32 ‖ pub_sign:32 )>
//! ```
//!
//! Base64url **sans padding** et **canonique** (bits de fin nuls) : une carte
//! n'a qu'un seul encodage, donc `to_qr(from_qr(s)) == s` pour tout QR
//! accepté. Le QR ne contient aucun secret.

use alloc::string::String;
use alloc::vec::Vec;

use data_encoding::BASE64URL_NOPAD;

use super::keys::PublicIdentity;
use super::IdentityError;
use crate::crypto::noise::DH_LEN;
use crate::crypto::{VerifyingKey, PUBLIC_KEY_LEN};

/// Préfixe complet d'un QR dengon, version comprise.
pub const QR_PREFIX: &str = "dengon:v1:";

/// Schéma commun à toutes les versions : distingue « pas un QR dengon » de
/// « QR dengon d'une autre version ».
const QR_SCHEME: &str = "dengon:";

/// Longueur fixe des deux clés publiques.
const KEYS_LEN: usize = DH_LEN + PUBLIC_KEY_LEN;

impl PublicIdentity {
    /// Encode la carte de contact en chaîne QR.
    #[must_use]
    pub fn to_qr(&self) -> String {
        let pseudo = self.pseudo().as_bytes();
        // Invariant de construction : 1 ≤ len ≤ 255 (`check_pseudo`).
        let len = u8::try_from(pseudo.len())
            .unwrap_or_else(|_| unreachable!("pseudo validé à la construction"));
        let mut raw = Vec::with_capacity(1 + pseudo.len() + KEYS_LEN);
        raw.push(len);
        raw.extend_from_slice(pseudo);
        raw.extend_from_slice(&self.pub_static());
        raw.extend_from_slice(&self.pub_sign().to_bytes());

        let mut qr = String::from(QR_PREFIX);
        qr.push_str(&BASE64URL_NOPAD.encode(&raw));
        qr
    }

    /// Décode une chaîne QR scannée.
    ///
    /// # Errors
    ///
    /// Une variante `Qr*` de [`IdentityError`] par cause (préfixe, version,
    /// encodage, longueur, clé Ed25519), ou
    /// [`IdentityError::InvalidPseudo`] si le pseudo est vide ou n'est pas
    /// de l'UTF-8.
    pub fn from_qr(qr: &str) -> Result<Self, IdentityError> {
        if !qr.starts_with(QR_SCHEME) {
            return Err(IdentityError::QrPrefix);
        }
        let payload = qr.strip_prefix(QR_PREFIX).ok_or(IdentityError::QrVersion)?;
        let raw = BASE64URL_NOPAD
            .decode(payload.as_bytes())
            .map_err(|_| IdentityError::QrEncoding)?;

        let (&len, rest) = raw.split_first().ok_or(IdentityError::QrLength)?;
        let len = usize::from(len);
        if rest.len() != len + KEYS_LEN {
            return Err(IdentityError::QrLength);
        }
        let (pseudo, keys) = rest.split_at(len);
        let (pub_static, pub_sign) = keys.split_at(DH_LEN);

        let pseudo = core::str::from_utf8(pseudo).map_err(|_| IdentityError::InvalidPseudo)?;
        let pub_static: [u8; DH_LEN] =
            pub_static.try_into().map_err(|_| IdentityError::QrLength)?;
        let pub_sign: [u8; PUBLIC_KEY_LEN] =
            pub_sign.try_into().map_err(|_| IdentityError::QrLength)?;
        let pub_sign =
            VerifyingKey::from_bytes(&pub_sign).map_err(|_| IdentityError::QrPublicKey)?;

        Self::new(pseudo, pub_static, pub_sign)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;
    use proptest::prelude::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    fn carte(pseudo: &str, seed: u8) -> PublicIdentity {
        Identity::generate(pseudo, ChaCha20Rng::from_seed([seed; 32]))
            .unwrap()
            .public()
    }

    /// Construit un QR v1 depuis des octets bruts arbitraires.
    fn qr_brut(raw: &[u8]) -> String {
        let mut s = String::from(QR_PREFIX);
        s.push_str(&BASE64URL_NOPAD.encode(raw));
        s
    }

    #[test]
    fn format_du_qr() {
        let c = carte("bob", 1);
        let qr = c.to_qr();
        assert!(qr.starts_with("dengon:v1:"));
        let raw = BASE64URL_NOPAD
            .decode(&qr.as_bytes()[QR_PREFIX.len()..])
            .unwrap();
        assert_eq!(raw.len(), 1 + 3 + 64);
        assert_eq!(raw[0], 3);
        assert_eq!(&raw[1..4], b"bob");
        assert_eq!(raw[4..36], c.pub_static());
        assert_eq!(raw[36..], c.pub_sign().to_bytes());
        assert!(!qr.contains('='), "pas de padding");
    }

    #[test]
    fn prefixe_et_version_refuses() {
        assert_eq!(
            PublicIdentity::from_qr("https://example.org").unwrap_err(),
            IdentityError::QrPrefix
        );
        let qr = carte("bob", 1).to_qr();
        let v2 = qr.replacen("dengon:v1:", "dengon:v2:", 1);
        assert_eq!(
            PublicIdentity::from_qr(&v2).unwrap_err(),
            IdentityError::QrVersion
        );
    }

    #[test]
    fn encodage_invalide_refuse() {
        let qr = carte("bob", 1).to_qr();
        // Padding explicite, caractère hors alphabet base64url.
        for bad in [format!("{qr}="), format!("{qr}+"), format!("{qr}\n")] {
            assert_eq!(
                PublicIdentity::from_qr(&bad).unwrap_err(),
                IdentityError::QrEncoding,
                "{bad:?}"
            );
        }
    }

    #[test]
    fn longueur_incoherente_refusee() {
        let c = carte("bob", 1);
        let mut raw = vec![3u8];
        raw.extend_from_slice(b"bob");
        raw.extend_from_slice(&c.pub_static());
        raw.extend_from_slice(&c.pub_sign().to_bytes());

        assert_eq!(
            PublicIdentity::from_qr(QR_PREFIX).unwrap_err(),
            IdentityError::QrLength,
            "charge vide"
        );
        let court = &raw[..raw.len() - 1];
        assert_eq!(
            PublicIdentity::from_qr(&qr_brut(court)).unwrap_err(),
            IdentityError::QrLength
        );
        let mut long = raw.clone();
        long.push(0);
        assert_eq!(
            PublicIdentity::from_qr(&qr_brut(&long)).unwrap_err(),
            IdentityError::QrLength
        );
        // Référence : le brut intact est accepté.
        assert_eq!(PublicIdentity::from_qr(&qr_brut(&raw)).unwrap(), c);
    }

    #[test]
    fn pseudo_invalide_refuse() {
        let c = carte("bob", 1);
        let keys = [c.pub_static().as_slice(), &c.pub_sign().to_bytes()].concat();

        let vide = [&[0u8][..], &keys].concat();
        assert_eq!(
            PublicIdentity::from_qr(&qr_brut(&vide)).unwrap_err(),
            IdentityError::InvalidPseudo
        );
        let non_utf8 = [&[2u8, 0xC3, 0x28][..], &keys].concat();
        assert_eq!(
            PublicIdentity::from_qr(&qr_brut(&non_utf8)).unwrap_err(),
            IdentityError::InvalidPseudo
        );
    }

    #[test]
    fn cle_ed25519_invalide_refusee() {
        let c = carte("bob", 1);
        let mut raw = vec![3u8];
        raw.extend_from_slice(b"bob");
        raw.extend_from_slice(&c.pub_static());
        // y = 2 : pas un point de la courbe (même cas que les tests `crypto`).
        let mut bad = [0u8; 32];
        bad[0] = 2;
        raw.extend_from_slice(&bad);
        assert_eq!(
            PublicIdentity::from_qr(&qr_brut(&raw)).unwrap_err(),
            IdentityError::QrPublicKey
        );
    }

    proptest! {
        /// Critère d'acceptation : round-trip QR, pour tout pseudo valide et
        /// toutes clés.
        #[test]
        fn aller_retour_qr(pseudo in "\\PC{1,80}", seed in any::<[u8; 32]>()) {
            prop_assume!(pseudo.len() <= crate::identity::MAX_PSEUDO_LEN);
            let c = Identity::generate(&pseudo, ChaCha20Rng::from_seed(seed))
                .unwrap()
                .public();
            let qr = c.to_qr();
            let decode = PublicIdentity::from_qr(&qr).unwrap();
            prop_assert_eq!(&decode, &c);
            // Encodage canonique : l'aller-retour inverse est aussi exact.
            prop_assert_eq!(decode.to_qr(), qr);
        }

        /// Une chaîne arbitraire ne fait jamais paniquer le décodeur.
        #[test]
        fn decodage_sans_panique(s in ".{0,200}") {
            let _ = PublicIdentity::from_qr(&s);
            let _ = PublicIdentity::from_qr(&format!("{QR_PREFIX}{s}"));
        }

        /// Des octets bruts arbitraires ne font jamais paniquer le décodeur.
        #[test]
        fn decodage_brut_sans_panique(raw in proptest::collection::vec(any::<u8>(), 0..400)) {
            let _ = PublicIdentity::from_qr(&qr_brut(&raw));
        }
    }
}
