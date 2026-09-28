//! Noise `XX` (session live) et Noise `X` (enveloppe scellée) — US-204.
//!
//! Implémentation unique en Rust (A-3), au-dessus de `snow` ≥ 0.10.0
//! (`docs/synthese/06-securite.md` §3) :
//!
//! - **`XX`** (`Noise_XX_25519_ChaChaPoly_SHA256`) : handshake 3 messages
//!   (`-> e` / `<- e, ee, s, es` / `-> s, se`) entre deux pairs connectés →
//!   [`Session`] bidirectionnelle, authentification mutuelle, forward secrecy.
//! - **`X`** (`Noise_X_25519_ChaChaPoly_SHA256`) : un seul message
//!   (`-> e, es, s, ss`) vers la clé statique publique d'un destinataire
//!   absent ([`seal`] / [`open`]). La clé statique de l'expéditeur voyage
//!   **chiffrée** dans le message : un relais ne sait pas qui envoie.
//!
//! Tout clair (payloads de handshake compris) est paddé vers `PAD_BUCKETS`
//! avant chiffrement ([`super::pad`]).
//!
//! # Transport tolérant aux pertes
//!
//! `NOISE_MSG` est relayé de saut en saut (`05-protocole-et-trame.md` §6.1) et
//! un relais peut jeter, dupliquer ou réordonner (`06-securite.md` §1). Le
//! transport Noise « classique » (nonce implicite) serait désynchronisé pour
//! toujours par une seule perte. [`Session`] utilise donc le transport **sans
//! état** de `snow` : chaque chiffré porte son nonce en clair
//! (`nonce(u64 BE) ‖ chiffré`) et une fenêtre glissante de
//! [`REPLAY_WINDOW`] nonces rejette les rejeux (même principe que WireGuard /
//! IPsec).
//!
//! # Ce que ce module ne fait pas
//!
//! - **Pas de décision de confiance.** [`Session::remote_static`] et
//!   [`Opened::sender_static`] exposent la clé du pair ; la comparer au
//!   contact connu (vérifié ou TOFU) et rejeter/alerter relève de `identity`
//!   et `sync`.
//! - **Pas d'assemblage de paquet.** `recipient_tag ‖ epoch_day ‖ …` et la
//!   signature Ed25519 **extérieure** de la `SEALED_ENVELOPE` sont posés par
//!   la couche qui construit la trame L3. De même, le clair scellé prévu par
//!   `06-securite.md` §3 (`AppFrame ‖ sender_pub_static(32) ‖ sig(64)`) est
//!   assemblé par l'appelant : [`seal`] chiffre des octets opaques. Ces 96
//!   octets comptent dans [`super::pad::MAX_PADDED_PAYLOAD`] : l'`AppFrame`
//!   d'une enveloppe est limitée à 2046 − 96 = 1950 octets.
//! - **Pas de re-négociation après `2^n` messages** (06 §3) : à déclencher par
//!   `sync` en rejouant un handshake.
//! - **Pas de tirage de clé statique.** Comme pour Ed25519 (US-203), le
//!   secret X25519 est fourni par l'appelant. L'aléa des clés **éphémères**
//!   vient d'un RNG passé en argument ([`super::rng`]).

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use core::num::NonZeroU32;

use rand_core::{CryptoRng, RngCore};
use snow::params::{DHChoice, NoiseParams};
use snow::resolvers::{CryptoResolver, DefaultResolver};
use snow::{Builder, HandshakeState, StatelessTransportState};
use zeroize::Zeroize;

use super::pad::{pad, unpad};
use super::rng::CallerResolver;
use super::CryptoError;

/// Motif de la session live.
pub const NOISE_XX: &str = "Noise_XX_25519_ChaChaPoly_SHA256";
/// Motif de l'enveloppe scellée.
pub const NOISE_X: &str = "Noise_X_25519_ChaChaPoly_SHA256";

/// Longueur d'une clé X25519 (publique ou secrète).
pub const DH_LEN: usize = 32;
/// Longueur d'un tag AEAD ChaCha20-Poly1305.
pub const AEAD_TAG_LEN: usize = 16;
/// Surcoût fixe d'un message Noise `X` : `e` (32) + `s` chiffrée (32 + 16) +
/// tag du payload (16). Taille d'une enveloppe = bucket + 96.
pub const SEALED_OVERHEAD: usize = DH_LEN + DH_LEN + AEAD_TAG_LEN + AEAD_TAG_LEN;

/// Longueur du nonce explicite en tête d'un chiffré de [`Session`].
pub const NONCE_LEN: usize = 8;
/// Surcoût d'un chiffré de [`Session`] : nonce (8) + tag AEAD (16).
/// Taille d'un `NOISE_MSG` chiffré = bucket + 24.
pub const SESSION_OVERHEAD: usize = NONCE_LEN + AEAD_TAG_LEN;
/// Taille de la fenêtre anti-rejeu, en nonces : un message arrivant avec plus
/// de 64 messages de retard est rejeté.
pub const REPLAY_WINDOW: u64 = 64;

/// Marge de tampon pour un message de handshake (`e`, `s` chiffrée, tags).
const HANDSHAKE_OVERHEAD: usize = DH_LEN + DH_LEN + AEAD_TAG_LEN + AEAD_TAG_LEN;

/// Paire de clés statiques X25519 (`static` de `06-securite.md` §2).
///
/// Le secret est effacé à la destruction ; `Debug` n'affiche que la clé
/// publique.
#[derive(Clone)]
pub struct StaticKeypair {
    secret: [u8; DH_LEN],
    public: [u8; DH_LEN],
}

impl StaticKeypair {
    /// Construit la paire depuis un secret de 32 octets (clamping X25519
    /// appliqué par la primitive).
    #[must_use]
    pub fn from_secret(secret: [u8; DH_LEN]) -> Self {
        // `default-resolver` + `use-curve25519` sont figés dans Cargo.toml :
        // Curve25519 est toujours résolu. Une clé publique nulle silencieuse
        // serait pire qu'un arrêt net.
        let mut dh = DefaultResolver
            .resolve_dh(&DHChoice::Curve25519)
            .unwrap_or_else(|| unreachable!("snow compilé sans use-curve25519"));
        dh.set(&secret);
        let mut public = [0u8; DH_LEN];
        public.copy_from_slice(dh.pubkey());
        Self { secret, public }
    }

    /// Clé publique (diffusée dans l'`ANNOUNCE` et le QR).
    #[must_use]
    pub fn public(&self) -> [u8; DH_LEN] {
        self.public
    }
}

impl Drop for StaticKeypair {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

impl fmt::Debug for StaticKeypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StaticKeypair { public: ")?;
        for b in self.public {
            write!(f, "{b:02x}")?;
        }
        f.write_str(", .. }")
    }
}

fn params(pattern: &str) -> Result<NoiseParams, CryptoError> {
    pattern.parse().map_err(|_| CryptoError::Noise)
}

fn builder<R>(pattern: &str, rng: R) -> Result<Builder<'static>, CryptoError>
where
    R: RngCore + CryptoRng + Send + Sync + 'static,
{
    Ok(Builder::with_resolver(
        params(pattern)?,
        Box::new(CallerResolver::new(rng)),
    ))
}

fn to_array(bytes: Option<&[u8]>) -> Option<[u8; DH_LEN]> {
    bytes.and_then(|b| b.try_into().ok())
}

// ----- Noise XX ---------------------------------------------------------

/// Handshake Noise `XX` en cours.
///
/// Déroulé : l'initiateur écrit le message 1, le répondeur le lit et écrit le
/// message 2, l'initiateur le lit et écrit le message 3, le répondeur le lit.
/// Ensuite [`Handshake::into_session`] des deux côtés.
///
/// **Un échec est définitif** : après une erreur de [`Handshake::read_message`]
/// (message altéré par un relais, perdu, hors séquence), l'état `snow` n'est
/// plus utilisable. L'appelant (`sync`) doit jeter ce `Handshake` et en
/// recommencer un complet, avec un nouveau RNG.
pub struct Handshake {
    state: HandshakeState,
}

impl Handshake {
    /// Côté initiateur (celui qui écrit le premier message).
    ///
    /// # Errors
    ///
    /// [`CryptoError::Noise`] si `snow` refuse la configuration.
    pub fn initiator<R>(local: &StaticKeypair, rng: R) -> Result<Self, CryptoError>
    where
        R: RngCore + CryptoRng + Send + Sync + 'static,
    {
        Self::build(local, rng, true)
    }

    /// Côté répondeur.
    ///
    /// # Errors
    ///
    /// [`CryptoError::Noise`] si `snow` refuse la configuration.
    pub fn responder<R>(local: &StaticKeypair, rng: R) -> Result<Self, CryptoError>
    where
        R: RngCore + CryptoRng + Send + Sync + 'static,
    {
        Self::build(local, rng, false)
    }

    fn build<R>(local: &StaticKeypair, rng: R, initiator: bool) -> Result<Self, CryptoError>
    where
        R: RngCore + CryptoRng + Send + Sync + 'static,
    {
        let b = builder(NOISE_XX, rng)?
            .local_private_key(&local.secret)
            .map_err(|_| CryptoError::Noise)?;
        let state = if initiator {
            b.build_initiator()
        } else {
            b.build_responder()
        }
        .map_err(|_| CryptoError::Noise)?;
        Ok(Self { state })
    }

    /// Écrit le prochain message de handshake, portant `payload` (paddé).
    ///
    /// # Errors
    ///
    /// [`CryptoError::PayloadTooLarge`] si `payload` ne tient pas dans le
    /// plus grand bucket ; [`CryptoError::Noise`] si ce n'est pas notre tour
    /// ou si le handshake est terminé.
    pub fn write_message(&mut self, payload: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let padded = pad(payload)?;
        let mut out = vec![0u8; padded.len() + HANDSHAKE_OVERHEAD];
        let n = self
            .state
            .write_message(&padded, &mut out)
            .map_err(|_| CryptoError::Noise)?;
        out.truncate(n);
        Ok(out)
    }

    /// Lit un message de handshake du pair et renvoie son payload.
    ///
    /// # Errors
    ///
    /// [`CryptoError::Noise`] si le message est altéré, hors séquence ou mal
    /// formé ; [`CryptoError::InvalidPadding`] si le clair n'est pas paddé.
    /// Dans les deux cas, le handshake est perdu (voir [`Handshake`]).
    pub fn read_message(&mut self, message: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut buf = vec![0u8; message.len()];
        let n = self
            .state
            .read_message(message, &mut buf)
            .map_err(|_| CryptoError::Noise)?;
        Ok(unpad(&buf[..n])?.to_vec())
    }

    /// `true` une fois les 3 messages échangés.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.state.is_handshake_finished()
    }

    /// Clé statique du pair, dès qu'elle a été reçue.
    #[must_use]
    pub fn remote_static(&self) -> Option<[u8; DH_LEN]> {
        to_array(self.state.get_remote_static())
    }

    /// Passe en mode transport.
    ///
    /// # Errors
    ///
    /// [`CryptoError::HandshakeNotFinished`] si le handshake n'est pas
    /// terminé.
    pub fn into_session(self) -> Result<Session, CryptoError> {
        if !self.state.is_handshake_finished() {
            return Err(CryptoError::HandshakeNotFinished);
        }
        let remote = self.remote_static().ok_or(CryptoError::Noise)?;
        let state = self
            .state
            .into_stateless_transport_mode()
            .map_err(|_| CryptoError::Noise)?;
        Ok(Session {
            state,
            remote,
            send_nonce: 0,
            window: ReplayWindow::default(),
        })
    }
}

impl fmt::Debug for Handshake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handshake")
            .field("initiator", &self.state.is_initiator())
            .field("finished", &self.state.is_handshake_finished())
            .finish_non_exhaustive()
    }
}

/// Session Noise `XX` établie : chiffrement bidirectionnel, paddé, tolérant
/// aux pertes et au réordonnancement.
///
/// Format d'un chiffré : `nonce(u64 BE) ‖ ChaCha20-Poly1305(padded)`. Taille =
/// bucket + [`SESSION_OVERHEAD`] : deux clairs du même bucket donnent des
/// chiffrés de même taille.
pub struct Session {
    state: StatelessTransportState,
    remote: [u8; DH_LEN],
    send_nonce: u64,
    window: ReplayWindow,
}

impl Session {
    /// Chiffre `plaintext` (paddé) pour le pair.
    ///
    /// # Errors
    ///
    /// [`CryptoError::PayloadTooLarge`] au-delà du plus grand bucket ;
    /// [`CryptoError::Noise`] si les nonces sont épuisés (`u64::MAX` est
    /// réservé par Noise) — il faut alors re-négocier.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let nonce = self.send_nonce;
        if nonce == u64::MAX {
            return Err(CryptoError::Noise);
        }
        let padded = pad(plaintext)?;
        let mut out = vec![0u8; NONCE_LEN + padded.len() + AEAD_TAG_LEN];
        out[..NONCE_LEN].copy_from_slice(&nonce.to_be_bytes());
        let n = self
            .state
            .write_message(nonce, &padded, &mut out[NONCE_LEN..])
            .map_err(|_| CryptoError::Noise)?;
        out.truncate(NONCE_LEN + n);
        self.send_nonce = nonce + 1;
        Ok(out)
    }

    /// Déchiffre un message du pair, dans n'importe quel ordre.
    ///
    /// La fenêtre anti-rejeu n'est mise à jour **qu'après** authentification :
    /// un nonce forgé ne peut pas la faire avancer.
    ///
    /// # Errors
    ///
    /// [`CryptoError::Noise`] si le chiffré est altéré, tronqué, déjà reçu
    /// (rejeu) ou trop ancien (hors fenêtre) ;
    /// [`CryptoError::InvalidPadding`] si le clair n'est pas paddé.
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if ciphertext.len() < SESSION_OVERHEAD {
            return Err(CryptoError::Noise);
        }
        let (nonce, body) = ciphertext.split_at(NONCE_LEN);
        let mut nonce_be = [0u8; NONCE_LEN];
        nonce_be.copy_from_slice(nonce);
        let nonce = u64::from_be_bytes(nonce_be);
        if !self.window.is_fresh(nonce) {
            return Err(CryptoError::Noise);
        }
        let mut buf = vec![0u8; body.len()];
        let n = self
            .state
            .read_message(nonce, body, &mut buf)
            .map_err(|_| CryptoError::Noise)?;
        self.window.accept(nonce);
        Ok(unpad(&buf[..n])?.to_vec())
    }

    /// Clé statique authentifiée du pair.
    #[must_use]
    pub fn remote_static(&self) -> [u8; DH_LEN] {
        self.remote
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("initiator", &self.state.is_initiator())
            .field("send_nonce", &self.send_nonce)
            .finish_non_exhaustive()
    }
}

/// Fenêtre glissante anti-rejeu (RFC 6479, version bitmap 64 bits).
///
/// `highest` = plus grand nonce accepté ; le bit `i` de `seen` vaut 1 si le
/// nonce `highest - i` a été accepté.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ReplayWindow {
    highest: Option<u64>,
    seen: u64,
}

impl ReplayWindow {
    /// `true` si `nonce` n'a jamais été accepté et n'est pas trop ancien.
    fn is_fresh(&self, nonce: u64) -> bool {
        match self.highest {
            None => true,
            Some(h) if nonce > h => true,
            Some(h) => {
                let age = h - nonce;
                age < REPLAY_WINDOW && self.seen & (1u64 << age) == 0
            }
        }
    }

    /// Marque `nonce` comme accepté (à n'appeler qu'après [`Self::is_fresh`]
    /// et authentification).
    fn accept(&mut self, nonce: u64) {
        match self.highest {
            None => {
                self.highest = Some(nonce);
                self.seen = 1;
            }
            Some(h) if nonce > h => {
                let shift = nonce - h;
                self.seen = if shift >= REPLAY_WINDOW {
                    0
                } else {
                    self.seen << shift
                };
                self.seen |= 1;
                self.highest = Some(nonce);
            }
            Some(h) => self.seen |= 1u64 << (h - nonce),
        }
    }
}

// ----- Noise X ----------------------------------------------------------

/// Scelle `plaintext` pour `recipient_public` (Noise `X`, one-shot).
///
/// Le résultat est `noise_x_message` de `SEALED_ENVELOPE`
/// (`06-securite.md` §3) ; taille = bucket + [`SEALED_OVERHEAD`].
///
/// # Errors
///
/// [`CryptoError::PayloadTooLarge`] au-delà du plus grand bucket ;
/// [`CryptoError::Noise`] si `snow` échoue.
pub fn seal<R>(
    sender: &StaticKeypair,
    recipient_public: &[u8; DH_LEN],
    plaintext: &[u8],
    rng: R,
) -> Result<Vec<u8>, CryptoError>
where
    R: RngCore + CryptoRng + Send + Sync + 'static,
{
    let mut state = builder(NOISE_X, rng)?
        .local_private_key(&sender.secret)
        .and_then(|b| b.remote_public_key(recipient_public))
        .and_then(Builder::build_initiator)
        .map_err(|_| CryptoError::Noise)?;
    let padded = pad(plaintext)?;
    let mut out = vec![0u8; padded.len() + SEALED_OVERHEAD];
    let n = state
        .write_message(&padded, &mut out)
        .map_err(|_| CryptoError::Noise)?;
    out.truncate(n);
    Ok(out)
}

/// Enveloppe ouverte par [`open`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    /// Clé statique de l'expéditeur, révélée seulement au destinataire.
    pub sender_static: [u8; DH_LEN],
    /// Clair (padding retiré).
    pub plaintext: Vec<u8>,
}

/// Ouvre une enveloppe scellée avec la clé statique du destinataire.
///
/// Aucun aléa consommé : le répondeur `X` n'émet rien.
///
/// # Errors
///
/// [`CryptoError::Noise`] si l'enveloppe ne nous est pas destinée, est
/// altérée ou mal formée ; [`CryptoError::InvalidPadding`] si le clair n'est
/// pas paddé.
pub fn open(recipient: &StaticKeypair, sealed: &[u8]) -> Result<Opened, CryptoError> {
    let mut state = builder(NOISE_X, NoRng)?
        .local_private_key(&recipient.secret)
        .and_then(Builder::build_responder)
        .map_err(|_| CryptoError::Noise)?;
    let mut buf = vec![0u8; sealed.len()];
    let n = state
        .read_message(sealed, &mut buf)
        .map_err(|_| CryptoError::Noise)?;
    let sender_static = to_array(state.get_remote_static()).ok_or(CryptoError::Noise)?;
    Ok(Opened {
        sender_static,
        plaintext: unpad(&buf[..n])?.to_vec(),
    })
}

/// RNG qui échoue toujours : `snow` exige un RNG pour construire un état,
/// même quand le motif ne tire aucune clé éphémère (répondeur `X`). `snow`
/// ne passe que par `try_fill_bytes` : si un jour il tirait de l'aléa ici,
/// l'ouverture échouerait proprement au lieu d'utiliser des zéros.
struct NoRng;

impl RngCore for NoRng {
    fn next_u32(&mut self) -> u32 {
        0
    }
    fn next_u64(&mut self) -> u64 {
        0
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        dest.fill(0);
    }
    fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), rand_core::Error> {
        let code = NonZeroU32::new(rand_core::Error::CUSTOM_START).unwrap_or(NonZeroU32::MIN);
        Err(rand_core::Error::from(code))
    }
}

impl CryptoRng for NoRng {}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::{RngCore, SeedableRng};

    fn rng(seed: u8) -> ChaCha20Rng {
        ChaCha20Rng::from_seed([seed; 32])
    }

    fn alice() -> StaticKeypair {
        StaticKeypair::from_secret([0xA1; DH_LEN])
    }

    fn bob() -> StaticKeypair {
        StaticKeypair::from_secret([0xB0; DH_LEN])
    }

    /// Handshake XX complet entre deux instances → (session A, session B).
    fn etablir() -> (Session, Session) {
        let (a, b) = (alice(), bob());
        let mut ia = Handshake::initiator(&a, rng(1)).unwrap();
        let mut rb = Handshake::responder(&b, rng(2)).unwrap();

        let m1 = ia.write_message(b"").unwrap();
        assert_eq!(rb.read_message(&m1).unwrap(), b"");
        let m2 = rb.write_message(b"").unwrap();
        ia.read_message(&m2).unwrap();
        let m3 = ia.write_message(b"").unwrap();
        rb.read_message(&m3).unwrap();

        assert!(ia.is_finished() && rb.is_finished());
        (ia.into_session().unwrap(), rb.into_session().unwrap())
    }

    #[test]
    fn cle_publique_x25519_rfc7748() {
        // RFC 7748 §6.1 : clé privée d'Alice → clé publique d'Alice.
        let secret: [u8; 32] = [
            0x77, 0x07, 0x6d, 0x0a, 0x73, 0x18, 0xa5, 0x7d, 0x3c, 0x16, 0xc1, 0x72, 0x51, 0xb2,
            0x66, 0x45, 0xdf, 0x4c, 0x2f, 0x87, 0xeb, 0xc0, 0x99, 0x2a, 0xb1, 0x77, 0xfb, 0xa5,
            0x1d, 0xb9, 0x2c, 0x2a,
        ];
        let public: [u8; 32] = [
            0x85, 0x20, 0xf0, 0x09, 0x89, 0x30, 0xa7, 0x54, 0x74, 0x8b, 0x7d, 0xdc, 0xb4, 0x3e,
            0xf7, 0x5a, 0x0d, 0xbf, 0x3a, 0x0d, 0x26, 0x38, 0x1a, 0xf4, 0xeb, 0xa4, 0xa9, 0x8e,
            0xaa, 0x9b, 0x4e, 0x6a,
        ];
        assert_eq!(StaticKeypair::from_secret(secret).public(), public);
    }

    #[test]
    fn xx_authentification_mutuelle() {
        let (sa, sb) = etablir();
        assert_eq!(sa.remote_static(), bob().public());
        assert_eq!(sb.remote_static(), alice().public());
    }

    #[test]
    fn xx_echange_bidirectionnel() {
        let (mut sa, mut sb) = etablir();
        for i in 0..5u8 {
            let c = sa.encrypt(&[i; 10]).unwrap();
            assert_eq!(sb.decrypt(&c).unwrap(), [i; 10]);
            let c = sb.encrypt(b"retour").unwrap();
            assert_eq!(sa.decrypt(&c).unwrap(), b"retour");
        }
    }

    #[test]
    fn xx_remote_static_disponible_apres_message_2() {
        let (a, b) = (alice(), bob());
        let mut ia = Handshake::initiator(&a, rng(1)).unwrap();
        let mut rb = Handshake::responder(&b, rng(2)).unwrap();
        assert_eq!(ia.remote_static(), None);
        rb.read_message(&ia.write_message(b"").unwrap()).unwrap();
        ia.read_message(&rb.write_message(b"").unwrap()).unwrap();
        assert_eq!(ia.remote_static(), Some(b.public()));
        assert!(!ia.is_finished());
    }

    #[test]
    fn xx_payload_de_handshake_transporte() {
        let (a, b) = (alice(), bob());
        let mut ia = Handshake::initiator(&a, rng(1)).unwrap();
        let mut rb = Handshake::responder(&b, rng(2)).unwrap();
        rb.read_message(&ia.write_message(b"").unwrap()).unwrap();
        let m2 = rb.write_message(b"coucou").unwrap();
        assert_eq!(ia.read_message(&m2).unwrap(), b"coucou");
    }

    #[test]
    fn xx_session_avant_fin_refusee() {
        let ia = Handshake::initiator(&alice(), rng(1)).unwrap();
        assert!(format!("{ia:?}").contains("finished: false"));
        assert_eq!(
            ia.into_session().unwrap_err(),
            CryptoError::HandshakeNotFinished
        );
    }

    #[test]
    fn xx_message_hors_sequence_refuse() {
        let mut rb = Handshake::responder(&bob(), rng(2)).unwrap();
        // Le répondeur ne peut pas écrire en premier.
        assert_eq!(rb.write_message(b"").unwrap_err(), CryptoError::Noise);
    }

    #[test]
    fn xx_message_de_handshake_altere_rejete() {
        let (a, b) = (alice(), bob());
        let mut ia = Handshake::initiator(&a, rng(1)).unwrap();
        let mut rb = Handshake::responder(&b, rng(2)).unwrap();
        rb.read_message(&ia.write_message(b"").unwrap()).unwrap();
        let mut m2 = rb.write_message(b"").unwrap();
        m2[40] ^= 1; // dans `s` chiffrée
        assert_eq!(ia.read_message(&m2).unwrap_err(), CryptoError::Noise);
    }

    #[test]
    fn xx_chiffre_altere_rejete() {
        let (mut sa, mut sb) = etablir();
        let mut c = sa.encrypt(b"secret").unwrap();
        c[NONCE_LEN + 3] ^= 0x80;
        assert_eq!(sb.decrypt(&c).unwrap_err(), CryptoError::Noise);
    }

    #[test]
    fn xx_rejeu_rejete() {
        let (mut sa, mut sb) = etablir();
        let c = sa.encrypt(b"une fois").unwrap();
        sb.decrypt(&c).unwrap();
        assert_eq!(sb.decrypt(&c).unwrap_err(), CryptoError::Noise);
    }

    /// Revue US-204 : un relais peut jeter un `NOISE_MSG` (05 §6.1, 06 §1).
    /// Une perte ne doit pas désynchroniser la session.
    #[test]
    fn xx_perte_tolere() {
        let (mut sa, mut sb) = etablir();
        let _perdu = sa.encrypt(b"1").unwrap();
        let c2 = sa.encrypt(b"2").unwrap();
        let c3 = sa.encrypt(b"3").unwrap();
        assert_eq!(sb.decrypt(&c2).unwrap(), b"2");
        assert_eq!(sb.decrypt(&c3).unwrap(), b"3");
    }

    #[test]
    fn xx_reordonnancement_tolere() {
        let (mut sa, mut sb) = etablir();
        let c: Vec<Vec<u8>> = (0..5u8).map(|i| sa.encrypt(&[i]).unwrap()).collect();
        for i in [3usize, 0, 4, 1, 2] {
            assert_eq!(sb.decrypt(&c[i]).unwrap(), [u8::try_from(i).unwrap()]);
        }
        // Chaque message rejoué après coup est refusé.
        for ci in &c {
            assert_eq!(sb.decrypt(ci).unwrap_err(), CryptoError::Noise);
        }
    }

    #[test]
    fn xx_trop_ancien_rejete() {
        let (mut sa, mut sb) = etablir();
        let vieux = sa.encrypt(b"vieux").unwrap();
        for _ in 0..REPLAY_WINDOW {
            let c = sa.encrypt(b"x").unwrap();
            sb.decrypt(&c).unwrap();
        }
        // nonce 0 alors que le plus haut vu est 64 : hors fenêtre.
        assert_eq!(sb.decrypt(&vieux).unwrap_err(), CryptoError::Noise);
    }

    #[test]
    fn xx_nonce_forge_ne_fait_pas_avancer_la_fenetre() {
        let (mut sa, mut sb) = etablir();
        let c0 = sa.encrypt(b"legitime").unwrap();
        let mut forge = c0.clone();
        forge[..NONCE_LEN].copy_from_slice(&1_000_000u64.to_be_bytes());
        assert_eq!(sb.decrypt(&forge).unwrap_err(), CryptoError::Noise);
        // Si la fenêtre avait avancé à 1 000 000, c0 (nonce 0) serait refusé.
        assert_eq!(sb.decrypt(&c0).unwrap(), b"legitime");
    }

    #[test]
    fn xx_chiffre_tronque_rejete() {
        let (mut sa, mut sb) = etablir();
        let c = sa.encrypt(b"x").unwrap();
        assert_eq!(
            sb.decrypt(&c[..SESSION_OVERHEAD - 1]).unwrap_err(),
            CryptoError::Noise
        );
        assert_eq!(
            sb.decrypt(&c[..c.len() - 1]).unwrap_err(),
            CryptoError::Noise
        );
        assert_eq!(sb.decrypt(&[]).unwrap_err(), CryptoError::Noise);
    }

    #[test]
    fn xx_nonces_epuises() {
        let (mut sa, _) = etablir();
        sa.send_nonce = u64::MAX;
        assert_eq!(sa.encrypt(b"x").unwrap_err(), CryptoError::Noise);
    }

    #[test]
    fn xx_nonce_en_tete_big_endian() {
        let (mut sa, _) = etablir();
        let c0 = sa.encrypt(b"a").unwrap();
        let c1 = sa.encrypt(b"b").unwrap();
        assert_eq!(c0[..NONCE_LEN], 0u64.to_be_bytes());
        assert_eq!(c1[..NONCE_LEN], 1u64.to_be_bytes());
    }

    #[test]
    fn fenetre_anti_rejeu() {
        let mut w = ReplayWindow::default();
        assert!(w.is_fresh(5));
        w.accept(5);
        assert!(!w.is_fresh(5));
        assert!(w.is_fresh(4) && w.is_fresh(6));
        w.accept(3);
        assert!(!w.is_fresh(3));
        // Saut plus grand que la fenêtre : l'historique est oublié.
        w.accept(5 + 200);
        assert!(!w.is_fresh(205));
        assert!(!w.is_fresh(5)); // trop ancien
        assert!(w.is_fresh(205 - 63));
        assert!(!w.is_fresh(205 - 64));
        // Petit saut : l'historique glisse.
        w.accept(206);
        assert!(!w.is_fresh(205) && !w.is_fresh(206));
        assert!(w.is_fresh(u64::MAX));
    }

    #[test]
    fn xx_clair_trop_grand_rejete() {
        let (mut sa, _) = etablir();
        assert_eq!(
            sa.encrypt(&[0; 2047]).unwrap_err(),
            CryptoError::PayloadTooLarge
        );
    }

    /// Critère d'acceptation US-204 : deux messages de tailles différentes
    /// produisent des trames de même taille.
    #[test]
    fn deux_messages_de_tailles_differentes_meme_taille_de_trame() {
        let (mut sa, _) = etablir();
        let court = sa.encrypt(b"ok").unwrap();
        let long = sa.encrypt(&[b'x'; 200]).unwrap();
        assert_eq!(court.len(), long.len());
        assert_eq!(court.len(), 256 + SESSION_OVERHEAD);

        let (a, b) = (alice(), bob());
        let e1 = seal(&a, &b.public(), b"ok", rng(3)).unwrap();
        let e2 = seal(&a, &b.public(), &[b'x'; 200], rng(4)).unwrap();
        assert_eq!(e1.len(), e2.len());
        assert_eq!(e1.len(), 256 + SEALED_OVERHEAD);
    }

    #[test]
    fn x_aller_retour_et_expediteur_revele() {
        let (a, b) = (alice(), bob());
        let env = seal(&a, &b.public(), b"pour Bob, hors ligne", rng(3)).unwrap();
        let ouvert = open(&b, &env).unwrap();
        assert_eq!(ouvert.plaintext, b"pour Bob, hors ligne");
        assert_eq!(ouvert.sender_static, a.public());
    }

    #[test]
    fn x_expediteur_invisible_en_clair() {
        let (a, b) = (alice(), bob());
        let env = seal(&a, &b.public(), b"x", rng(3)).unwrap();
        let pk = a.public();
        assert!(!env.windows(DH_LEN).any(|w| w == pk));
    }

    #[test]
    fn x_mauvaise_cle_echoue_proprement() {
        let (a, b) = (alice(), bob());
        let env = seal(&a, &b.public(), b"pour Bob", rng(3)).unwrap();
        let eve = StaticKeypair::from_secret([0xEE; DH_LEN]);
        assert_eq!(open(&eve, &env).unwrap_err(), CryptoError::Noise);
    }

    #[test]
    fn x_enveloppe_alteree_ou_tronquee_rejetee() {
        let (a, b) = (alice(), bob());
        let env = seal(&a, &b.public(), b"pour Bob", rng(3)).unwrap();
        let mut alteree = env.clone();
        let dernier = alteree.len() - 1;
        alteree[dernier] ^= 1;
        assert_eq!(open(&b, &alteree).unwrap_err(), CryptoError::Noise);
        assert_eq!(open(&b, &env[..10]).unwrap_err(), CryptoError::Noise);
        assert_eq!(open(&b, &[]).unwrap_err(), CryptoError::Noise);
    }

    #[test]
    fn x_trop_grand_rejete() {
        let (a, b) = (alice(), bob());
        assert_eq!(
            seal(&a, &b.public(), &[0; 4096], rng(3)).unwrap_err(),
            CryptoError::PayloadTooLarge
        );
    }

    #[test]
    fn meme_graine_meme_transcript() {
        let (a, b) = (alice(), bob());
        let e1 = seal(&a, &b.public(), b"det", rng(9)).unwrap();
        let e2 = seal(&a, &b.public(), b"det", rng(9)).unwrap();
        let e3 = seal(&a, &b.public(), b"det", rng(10)).unwrap();
        assert_eq!(e1, e2);
        assert_ne!(e1, e3);
    }

    #[test]
    fn debug_ne_divulgue_pas_le_secret() {
        let kp = StaticKeypair::from_secret([0x5A; DH_LEN]);
        let s = format!("{kp:?}");
        assert!(!s.contains("5a5a5a5a"));
        assert!(s.contains("StaticKeypair"));
        let (sa, _) = etablir();
        assert!(format!("{sa:?}").contains("Session"));
    }

    #[test]
    fn no_rng_echoue() {
        let mut r = NoRng;
        let mut buf = [1u8; 4];
        assert!(r.try_fill_bytes(&mut buf).is_err());
        r.fill_bytes(&mut buf);
        assert_eq!(buf, [0; 4]);
        assert_eq!((r.next_u32(), r.next_u64()), (0, 0));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn xx_aller_retour(data in proptest::collection::vec(any::<u8>(), 0..=2046)) {
            let (mut sa, mut sb) = etablir();
            let c = sa.encrypt(&data).unwrap();
            prop_assert!(crate::crypto::PAD_BUCKETS.contains(&(c.len() - SESSION_OVERHEAD)));
            prop_assert_eq!(sb.decrypt(&c).unwrap(), data);
        }

        #[test]
        fn x_aller_retour(data in proptest::collection::vec(any::<u8>(), 0..=2046), seed: u8) {
            let (a, b) = (alice(), bob());
            let env = seal(&a, &b.public(), &data, rng(seed)).unwrap();
            prop_assert_eq!(open(&b, &env).unwrap().plaintext, data);
        }
    }
}
