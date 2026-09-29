//! Aléa de la plateforme pour Noise (US-308).
//!
//! `snow` est compilé sans `use-getrandom` (inutilisable sur xtensa) : son
//! `DefaultResolver` ne fournit alors **aucun** générateur
//! (`docs/synthese/06-securite.md`, Spike A point 2). `dengon-core` a déjà le
//! point d'accroche : chaque entrée Noise (`Handshake::initiator` /
//! `responder`, `seal`) prend un `R: RngCore + CryptoRng` que son
//! `CallerResolver` remet à `snow`. Il suffit donc d'un [`PlatformRng`] qui
//! implémente ces traits au-dessus d'une fonction C — `esp_fill_random()`
//! côté firmware — plutôt que d'un second `CryptoResolver`.
//!
//! [`dengon_noise_selftest`] le prouve sur cible : un handshake `XX` complet
//! entre deux paires de clés tirées de cet aléa, puis un aller-retour chiffré.

use dengon_core::crypto::noise::DH_LEN;
use dengon_core::crypto::{Handshake, StaticKeypair};
use rand_core::{CryptoRng, RngCore};

use crate::Status;

/// Remplit `len` octets à `buf` avec de l'aléa **cryptographique** (côté
/// firmware : `esp_fill_random`, radio active ou `bootloader_random_enable()`
/// appelé — sinon ce n'est qu'un PRNG).
pub type FillRandom = unsafe extern "C" fn(buf: *mut u8, len: usize);

/// Générateur adossé à une fonction C de la plateforme.
#[derive(Debug, Clone, Copy)]
pub struct PlatformRng(FillRandom);

impl PlatformRng {
    /// Enveloppe `fill`.
    ///
    /// # Safety
    ///
    /// `fill` doit écrire exactement `len` octets à `buf` à chaque appel, et
    /// être sûr à appeler depuis n'importe quelle tâche.
    pub unsafe fn new(fill: FillRandom) -> Self {
        Self(fill)
    }
}

impl RngCore for PlatformRng {
    fn next_u32(&mut self) -> u32 {
        let mut b = [0u8; 4];
        self.fill_bytes(&mut b);
        u32::from_le_bytes(b)
    }

    fn next_u64(&mut self) -> u64 {
        let mut b = [0u8; 8];
        self.fill_bytes(&mut b);
        u64::from_le_bytes(b)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        if dest.is_empty() {
            return;
        }
        // SAFETY : contrat de `PlatformRng::new` — `self.0` écrit exactement
        // `dest.len()` octets à `dest.as_mut_ptr()`, tampon valide et exclusif.
        unsafe { (self.0)(dest.as_mut_ptr(), dest.len()) }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

// Aléa matériel (TRNG) : c'est précisément la garantie demandée à `fill`.
impl CryptoRng for PlatformRng {}

/// Auto-test Noise avec l'aléa de la plateforme : deux paires de clés
/// tirées de `fill`, handshake `XX` complet, puis un message chiffré dans
/// chaque sens. [`Status::Ok`] si tout aboutit, [`Status::Crypto`] sinon
/// (notamment si `snow` n'obtient aucun aléa), [`Status::NullPointer`] si
/// `fill` est nul.
///
/// # Safety
///
/// `fill` : voir [`PlatformRng::new`].
#[no_mangle]
pub unsafe extern "C" fn dengon_noise_selftest(
    fill: Option<unsafe extern "C" fn(buf: *mut u8, len: usize)>,
) -> Status {
    let Some(fill) = fill else {
        return Status::NullPointer;
    };
    // SAFETY : transmis tel quel, même contrat que cette fonction.
    let rng = unsafe { PlatformRng::new(fill) };
    if selftest(rng).is_some() {
        Status::Ok
    } else {
        Status::Crypto
    }
}

fn selftest(mut rng: PlatformRng) -> Option<()> {
    let mut paire = || {
        let mut secret = [0u8; DH_LEN];
        rng.fill_bytes(&mut secret);
        StaticKeypair::from_secret(secret)
    };
    let (a, b) = (paire(), paire());
    if a.public() == b.public() {
        return None; // aléa constant : inutilisable
    }
    let mut ini = Handshake::initiator(&a, rng).ok()?;
    let mut rep = Handshake::responder(&b, rng).ok()?;
    let m1 = ini.write_message(&[]).ok()?;
    rep.read_message(&m1).ok()?;
    let m2 = rep.write_message(&[]).ok()?;
    ini.read_message(&m2).ok()?;
    let m3 = ini.write_message(&[]).ok()?;
    rep.read_message(&m3).ok()?;
    let (mut sa, mut sb) = (ini.into_session().ok()?, rep.into_session().ok()?);
    let aller = sa.encrypt(b"dengon").ok()?;
    let retour = sb.encrypt(b"relais").ok()?;
    (sb.decrypt(&aller).ok()? == b"dengon" && sa.decrypt(&retour).ok()? == b"relais").then_some(())
}
