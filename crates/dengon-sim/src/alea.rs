//! Générateur pseudo-aléatoire **déterministe** du simulateur.
//!
//! SplitMix64 (Steele, Lea, Flood, 2014) : 64 bits d'état, quelques
//! multiplications, sortie identique sur toutes les plateformes. Pas de
//! dépendance à `rand` : la reproductibilité d'une trace ne doit pas dépendre
//! d'une mise à jour de crate qui changerait l'algorithme par défaut.
//!
//! Ce n'est **pas** un générateur cryptographique, et il n'a pas à l'être : il
//! ne décide que des pertes et des gigues du réseau simulé.

/// Générateur SplitMix64.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alea {
    etat: u64,
}

impl Alea {
    /// Un générateur initialisé par `graine`. Même graine → même suite.
    #[must_use]
    pub const fn new(graine: u64) -> Self {
        Self { etat: graine }
    }

    /// Le prochain entier de 64 bits.
    pub fn suivant(&mut self) -> u64 {
        self.etat = self.etat.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.etat;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Un entier dans `0..borne` (0 si `borne == 0`).
    ///
    /// Réduction par modulo : le biais est de l'ordre de `borne / 2^64`,
    /// négligeable pour des bornes de quelques milliers.
    pub fn sous(&mut self, borne: u64) -> u64 {
        if borne == 0 {
            0
        } else {
            self.suivant() % borne
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Alea;

    #[test]
    fn vecteur_de_reference_splitmix64() {
        // Première sortie de SplitMix64 pour la graine 0 (valeur de
        // référence de l'implémentation de Vigna).
        assert_eq!(Alea::new(0).suivant(), 0xE220_A839_7B1D_CDAF);
    }

    #[test]
    fn meme_graine_meme_suite() {
        let mut a = Alea::new(42);
        let mut b = Alea::new(42);
        for _ in 0..100 {
            assert_eq!(a.suivant(), b.suivant());
        }
        assert_ne!(Alea::new(1).suivant(), Alea::new(2).suivant());
    }

    #[test]
    fn sous_respecte_la_borne() {
        let mut a = Alea::new(7);
        assert_eq!(a.sous(0), 0);
        for _ in 0..1000 {
            assert!(a.sous(10) < 10);
        }
    }
}
