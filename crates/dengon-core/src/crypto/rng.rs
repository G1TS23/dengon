//! Aléa fourni par l'appelant pour `snow` (US-204).
//!
//! `snow` est compilé **sans** `use-getrandom` (indisponible sur xtensa, Spike
//! A) : son `DefaultResolver::resolve_rng()` renvoie alors `None` et tout
//! handshake échouerait au runtime. On branche donc un resolver qui délègue
//! tout au `DefaultResolver` **sauf** l'aléa, pris dans un RNG fourni par
//! l'appelant : `OsRng` sur hôte, `esp_fill_random()` côté firmware (US-307),
//! RNG déterministe dans les vecteurs de conformité.
//!
//! `snow` n'appelle `resolve_rng()` **qu'une fois** par `HandshakeState`
//! construit (`Builder::build`) : le RNG est donc consommé à ce moment-là.

use alloc::boxed::Box;
use core::cell::RefCell;

use rand_core::{CryptoRng, RngCore};
use snow::params::{CipherChoice, DHChoice, HashChoice};
use snow::resolvers::{CryptoResolver, DefaultResolver};
use snow::types::{Cipher, Dh, Hash, Random};

/// Adaptateur `rand_core` → `snow::types::Random`.
struct CallerRng<R>(R);

impl<R: RngCore + CryptoRng + Send + Sync> Random for CallerRng<R> {
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), snow::Error> {
        self.0.try_fill_bytes(dest).map_err(|_| snow::Error::Rng)
    }
}

/// Resolver `snow` : primitives par défaut + RNG de l'appelant (usage unique).
pub(super) struct CallerResolver {
    rng: RefCell<Option<Box<dyn Random>>>,
}

impl CallerResolver {
    pub(super) fn new<R>(rng: R) -> Self
    where
        R: RngCore + CryptoRng + Send + Sync + 'static,
    {
        Self {
            rng: RefCell::new(Some(Box::new(CallerRng(rng)))),
        }
    }
}

impl CryptoResolver for CallerResolver {
    fn resolve_rng(&self) -> Option<Box<dyn Random>> {
        self.rng.borrow_mut().take()
    }

    fn resolve_dh(&self, choice: &DHChoice) -> Option<Box<dyn Dh>> {
        DefaultResolver.resolve_dh(choice)
    }

    fn resolve_hash(&self, choice: &HashChoice) -> Option<Box<dyn Hash>> {
        DefaultResolver.resolve_hash(choice)
    }

    fn resolve_cipher(&self, choice: &CipherChoice) -> Option<Box<dyn Cipher>> {
        DefaultResolver.resolve_cipher(choice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    #[test]
    fn le_rng_n_est_fourni_qu_une_fois() {
        let r = CallerResolver::new(ChaCha20Rng::from_seed([0; 32]));
        let mut rng = r.resolve_rng().expect("premier appel");
        let mut buf = [0u8; 8];
        rng.try_fill_bytes(&mut buf).unwrap();
        assert_ne!(buf, [0u8; 8]);
        assert!(r.resolve_rng().is_none());
    }
}
