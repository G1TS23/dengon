//! Code de vérification (« safety number ») de 60 chiffres
//! (`docs/synthese/06-securite.md` §2, C-12).
//!
//! ```text
//! material = SHA-512( min(fpA, fpB) ‖ max(fpA, fpB) )     // ordre-indépendant
//! pour i in 0..12 : g[i] = u40_be(material[5i .. 5i+5]) % 100_000
//! affichage : 12 groupes de 5 chiffres, complétés à gauche par des zéros
//! ```
//!
//! **Écart vs conception** : la conception prend 2 octets par groupe
//! (`u16 % 100000`). Un `u16` ne dépasse pas 65 535 : le modulo ne fait rien
//! et aucun groupe ne commence par 7, 8 ou 9. On prend 5 octets (40 bits,
//! comme le safety number de Signal) : le biais du modulo devient
//! négligeable (2⁴⁰ / 10⁵ ≈ 1,1·10⁷ tirages par valeur). On consomme 60 des
//! 64 octets du SHA-512. Voir `docs/suivi/03-ecarts-conception.md`.

use alloc::string::String;
use core::fmt;
use core::fmt::Write as _;

use sha2::{Digest, Sha512};

use super::keys::{Fingerprint, PublicIdentity};

/// Nombre de groupes de 5 chiffres.
pub const SAFETY_GROUPS: usize = 12;

/// Nombre total de chiffres.
pub const SAFETY_DIGITS: usize = SAFETY_GROUPS * GROUP_DIGITS;

/// Chiffres par groupe.
const GROUP_DIGITS: usize = 5;

/// Octets de `material` consommés par groupe.
const GROUP_BYTES: usize = 5;

/// `10^GROUP_DIGITS`.
const GROUP_MODULUS: u64 = 100_000;

/// Code de vérification : 12 groupes, chacun dans `0..100_000`.
///
/// `Display` : `"01234 56789 …"` (12 groupes séparés par une espace).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SafetyNumber([u32; SAFETY_GROUPS]);

impl SafetyNumber {
    /// Les 12 groupes, dans l'ordre d'affichage.
    #[must_use]
    pub fn groups(&self) -> [u32; SAFETY_GROUPS] {
        self.0
    }

    /// Les 60 chiffres, sans séparateur.
    #[must_use]
    pub fn digits(&self) -> String {
        let mut s = String::with_capacity(SAFETY_DIGITS);
        for g in self.0 {
            // L'écriture dans une `String` ne peut pas échouer.
            let _ = write!(s, "{g:05}");
        }
        s
    }
}

impl fmt::Display for SafetyNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, g) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{g:05}")?;
        }
        Ok(())
    }
}

/// Code de vérification de deux empreintes. **Symétrique** :
/// `safety_number(a, b) == safety_number(b, a)`.
#[must_use]
pub fn safety_number(a: &Fingerprint, b: &Fingerprint) -> SafetyNumber {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let mut h = Sha512::new();
    h.update(lo);
    h.update(hi);
    let material = h.finalize();

    let mut groups = [0u32; SAFETY_GROUPS];
    for (g, chunk) in groups.iter_mut().zip(material.chunks_exact(GROUP_BYTES)) {
        let v = chunk.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b));
        *g = u32::try_from(v % GROUP_MODULUS).unwrap_or_else(|_| unreachable!("valeur < 100 000"));
    }
    SafetyNumber(groups)
}

/// Code de vérification entre deux cartes de contact : ce que les deux
/// correspondants comparent après l'échange de QR.
#[must_use]
pub fn verification_code(a: &PublicIdentity, b: &PublicIdentity) -> SafetyNumber {
    safety_number(&a.fingerprint(), &b.fingerprint())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use proptest::prelude::*;

    #[test]
    fn calcul_recoupe_a_la_main() {
        let (a, b) = ([0x22u8; 32], [0x11u8; 32]);
        let mut cat = [0u8; 64];
        cat[..32].copy_from_slice(&b); // min d'abord
        cat[32..].copy_from_slice(&a);
        let m = Sha512::digest(cat);
        let g0 = (u64::from(m[0]) << 32)
            | (u64::from(m[1]) << 24)
            | (u64::from(m[2]) << 16)
            | (u64::from(m[3]) << 8)
            | u64::from(m[4]);
        let code = safety_number(&a, &b);
        assert_eq!(u64::from(code.groups()[0]), g0 % 100_000);
    }

    #[test]
    fn affichage_60_chiffres_en_12_groupes() {
        let code = SafetyNumber([0, 7, 12_345, 99_999, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(
            code.to_string(),
            "00000 00007 12345 99999 00001 00002 00003 00004 00005 00006 00007 00008"
        );
        assert_eq!(code.digits().len(), SAFETY_DIGITS);
        assert_eq!(code.digits(), code.to_string().replace(' ', ""));
    }

    #[test]
    fn meme_empreinte_des_deux_cotes() {
        // Cas limite (auto-vérification) : défini et déterministe.
        let a = [0x42u8; 32];
        assert_eq!(safety_number(&a, &a), safety_number(&a, &a));
    }

    proptest! {
        /// Critère d'acceptation : A et B calculent le même code.
        #[test]
        fn symetrie(a in any::<[u8; 32]>(), b in any::<[u8; 32]>()) {
            prop_assert_eq!(safety_number(&a, &b), safety_number(&b, &a));
        }

        #[test]
        fn toujours_60_chiffres(a in any::<[u8; 32]>(), b in any::<[u8; 32]>()) {
            let code = safety_number(&a, &b);
            let digits = code.digits();
            prop_assert_eq!(digits.len(), SAFETY_DIGITS);
            prop_assert!(digits.bytes().all(|c| c.is_ascii_digit()));
            prop_assert!(code.groups().iter().all(|&g| g < 100_000));
            prop_assert_eq!(format!("{code}").len(), SAFETY_DIGITS + SAFETY_GROUPS - 1);
        }

        /// Changer une seule empreinte change le code (MITM détecté).
        #[test]
        fn empreinte_differente_code_different(
            a in any::<[u8; 32]>(),
            b in any::<[u8; 32]>(),
            c in any::<[u8; 32]>(),
        ) {
            prop_assume!(b != c && a != b && a != c);
            prop_assert_ne!(safety_number(&a, &b), safety_number(&a, &c));
        }
    }
}
