//! Primitives cryptographiques de dengon (`docs/synthese/06-securite.md`).
//!
//! - Signature Ed25519 (US-203) — ce fichier ;
//! - Noise `XX` / `X` ([`noise`]), `recipient_tag` ([`tag`]) et padding
//!   `PAD_BUCKETS` ([`pad`]) — US-204.
//!
//! # Signature Ed25519 (RFC 8032)
//!
//! Sert à signer le journal chaîné (`ledger`, US-206), les batchs d'événements
//! remontés au dashboard (US-216, US-309) et, à terme, les paquets `ANNOUNCE`,
//! `INVENTORY`, `ENVELOPE_*` et `LOG_ATTEST`
//! (`docs/synthese/06-securite.md` §2).
//!
//! # Ce que ce module ne fait pas
//!
//! - **Pas de génération de clé.** Une clé se construit depuis une graine de
//!   32 octets ([`SigningKey::from_seed`]). Le tirage aléatoire de la graine
//!   est à la charge de l'appelant (module `identity`, ou `esp_fill_random`
//!   côté firmware, US-307) : cela évite d'embarquer `getrandom` sur la cible
//!   `no_std`.
//! - **Pas de séparation de domaine.** On signe les octets fournis tels quels,
//!   comme le décrit la conception (préfixe de paquet, `entry_hash`, JSON
//!   canonique d'un batch). C'est à l'appelant de ne signer que des octets
//!   dont le sens est non ambigu.
//! - **Pas de mise en forme des paquets L3.** Un paquet ne se signe pas tel
//!   qu'il circule : la signature couvre l'en-tête avec l'octet `ttl` mis
//!   à 0, sinon elle casse au premier relais (qui décrémente `ttl`). Les
//!   octets à signer et à vérifier sont fournis par `protocol::codec`
//!   (`signing_input` à l'émission, `received_signing_input` à la
//!   réception, US-201), jamais la trame brute.
//!
//! # `no_std`
//!
//! Le module ne dépend ni de `std` ni d'`alloc` : `ed25519-dalek` est utilisé
//! avec `default-features = false` (configuration validée par le Spike A,
//! `docs/suivi/spikes/US-101-cross-compile-xtensa.md`).

use core::fmt;

use ed25519_dalek::{Signature as DalekSignature, Signer as _};

pub mod noise;
pub mod pad;
mod rng;
pub mod tag;

pub use noise::{open, seal, Handshake, Opened, Session, StaticKeypair};
pub use pad::{pad, unpad, PAD_BUCKETS};
pub use tag::{epoch_day, own_tags, recipient_tag, RecipientTag, RECIPIENT_TAG_LEN};

/// Longueur d'une signature Ed25519 et signature `R ‖ S` (64 octets).
///
/// Réexportées depuis `protocol` (US-108), seule source de vérité : les
/// modules s'échangent des signatures sans conversion.
pub use crate::protocol::{Signature, SIGNATURE_LEN};

// `ledger` (US-206) a sa propre constante, faute de `crypto` au moment où il
// a été écrit : elle doit rester égale à celle du protocole.
const _: () = assert!(crate::ledger::SIG_LEN == SIGNATURE_LEN);

/// Longueur d'une clé publique Ed25519, en octets.
pub const PUBLIC_KEY_LEN: usize = 32;

/// Longueur de la graine (clé privée) Ed25519, en octets.
pub const SEED_LEN: usize = 32;

/// Erreur de la couche `crypto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    /// Les 32 octets fournis ne décodent pas un point valide de la courbe.
    InvalidPublicKey,
    /// La signature ne correspond pas au message et à la clé publique, ou
    /// elle est rejetée par la vérification stricte (signature malléable,
    /// clé de faible ordre).
    InvalidSignature,
    /// Échec Noise : message altéré, hors séquence, pas destiné à cette clé,
    /// ou configuration refusée par `snow`. Les causes ne sont
    /// volontairement pas distinguées (pas d'oracle pour un attaquant).
    Noise,
    /// Clair trop grand pour le plus grand bucket de padding.
    PayloadTooLarge,
    /// Clair déchiffré dont le padding n'est pas conforme.
    InvalidPadding,
    /// Passage en transport demandé avant la fin du handshake.
    HandshakeNotFinished,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPublicKey => f.write_str("clé publique Ed25519 invalide"),
            Self::InvalidSignature => f.write_str("signature Ed25519 invalide"),
            Self::Noise => f.write_str("échec Noise (message invalide ou clé inattendue)"),
            Self::PayloadTooLarge => f.write_str("clair trop grand pour PAD_BUCKETS"),
            Self::InvalidPadding => f.write_str("padding invalide"),
            Self::HandshakeNotFinished => f.write_str("handshake Noise non terminé"),
        }
    }
}

impl core::error::Error for CryptoError {}

/// Clé de signature Ed25519 (partie privée).
///
/// Le secret est effacé de la mémoire à la destruction (feature `zeroize` de
/// `ed25519-dalek`). `Debug` n'affiche jamais le secret. Pas de `Clone` :
/// chaque copie serait un exemplaire de plus du secret en mémoire.
pub struct SigningKey(ed25519_dalek::SigningKey);

impl SigningKey {
    /// Construit la clé depuis une graine de 32 octets.
    ///
    /// Déterministe : la même graine donne toujours la même clé publique.
    #[must_use]
    pub fn from_seed(seed: &[u8; SEED_LEN]) -> Self {
        Self(ed25519_dalek::SigningKey::from_bytes(seed))
    }

    /// Clé publique correspondante.
    #[must_use]
    pub fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey(self.0.verifying_key())
    }

    /// Signe `message`.
    ///
    /// Ed25519 est déterministe (RFC 8032) : aucun aléa n'est consommé, le
    /// même message donne toujours la même signature.
    #[must_use]
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.0.sign(message).to_bytes()
    }
}

/// Branche une vraie clé Ed25519 sur le journal chaîné, à la place de
/// `ledger::NullSigner`.
impl crate::ledger::Signer for SigningKey {
    fn sign(&mut self, message: &[u8]) -> Signature {
        SigningKey::sign(self, message)
    }
}

impl fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SigningKey")
            .field("verifying_key", &self.verifying_key())
            .finish_non_exhaustive()
    }
}

/// Clé de vérification Ed25519 (partie publique).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VerifyingKey(ed25519_dalek::VerifyingKey);

impl VerifyingKey {
    /// Décode une clé publique.
    ///
    /// # Errors
    ///
    /// [`CryptoError::InvalidPublicKey`] si les octets ne sont pas
    /// l'encodage d'un point valide de la courbe.
    pub fn from_bytes(bytes: &[u8; PUBLIC_KEY_LEN]) -> Result<Self, CryptoError> {
        ed25519_dalek::VerifyingKey::from_bytes(bytes)
            .map(Self)
            .map_err(|_| CryptoError::InvalidPublicKey)
    }

    /// Encodage de la clé publique sur 32 octets.
    #[must_use]
    pub fn to_bytes(self) -> [u8; PUBLIC_KEY_LEN] {
        self.0.to_bytes()
    }

    /// Vérifie que `signature` est bien la signature de `message` par la clé
    /// privée associée.
    ///
    /// Utilise la vérification **stricte** de `ed25519-dalek` : rejette en
    /// plus les clés de faible ordre et les signatures malléables. Un journal
    /// d'audit ne doit pas accepter deux signatures distinctes pour un même
    /// message (voir `docs/suivi/03-ecarts-conception.md`).
    ///
    /// # Errors
    ///
    /// [`CryptoError::InvalidSignature`] si la vérification échoue, quelle
    /// qu'en soit la raison (on ne distingue volontairement pas les causes).
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), CryptoError> {
        self.0
            .verify_strict(message, &DalekSignature::from_bytes(signature))
            .map_err(|_| CryptoError::InvalidSignature)
    }
}

impl fmt::Debug for VerifyingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VerifyingKey(")?;
        for b in self.to_bytes() {
            write!(f, "{b:02x}")?;
        }
        f.write_str(")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Décode une chaîne hexadécimale (minuscules) en octets.
    fn hex(s: &str) -> Vec<u8> {
        assert_eq!(s.len() % 2, 0, "longueur hex impaire");
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex invalide"))
            .collect()
    }

    fn hex32(s: &str) -> [u8; 32] {
        hex(s).try_into().expect("32 octets attendus")
    }

    fn hex64(s: &str) -> [u8; 64] {
        hex(s).try_into().expect("64 octets attendus")
    }

    /// Vecteur de test connu (KAT) : graine, clé publique, message, signature.
    struct Kat {
        nom: &'static str,
        seed: &'static str,
        public: &'static str,
        message: &'static str,
        signature: &'static str,
    }

    /// RFC 8032 §7.1 (TEST 1, 2, 3 et SHA(abc)). TEST 1024 n'est pas repris :
    /// son message de 1023 octets alourdirait le fichier sans couvrir de cas
    /// de plus.
    ///
    /// Les valeurs ont été recoupées avec une implémentation indépendante
    /// (Python `cryptography`, OpenSSL) — voir le journal de suivi.
    const KATS: &[Kat] = &[
        Kat {
            nom: "TEST 1 (message vide)",
            seed: "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
            public: "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            message: "",
            signature: "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155\
                        5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
        },
        Kat {
            nom: "TEST 2 (1 octet)",
            seed: "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
            public: "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
            message: "72",
            signature: "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da\
                        085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
        },
        Kat {
            nom: "TEST 3 (2 octets)",
            seed: "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
            public: "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
            message: "af82",
            signature: "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac\
                        18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
        },
        Kat {
            nom: "TEST SHA(abc) (message de 64 octets)",
            seed: "833fe62409237b9d62ec77587520911e9a759cec1d19755b7da901b96dca3d42",
            public: "ec172b93ad5e563bf4932c70e1245034c35467ef2efd4d64ebf819683467e2bf",
            message: "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
                      2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
            signature: "dc2a4459e7369633a52b1bf277839a00201009a3efbf3ecb69bea2186c26b589\
                        09351fc9ac90b3ecfdfbc7c66431e0303dca179c138ac17ad9bef1177331a704",
        },
    ];

    fn cle_de_test() -> SigningKey {
        SigningKey::from_seed(&[7u8; SEED_LEN])
    }

    #[test]
    fn kat_rfc8032_cle_publique() {
        for kat in KATS {
            let sk = SigningKey::from_seed(&hex32(kat.seed));
            assert_eq!(
                sk.verifying_key().to_bytes(),
                hex32(kat.public),
                "{} : clé publique",
                kat.nom
            );
        }
    }

    #[test]
    fn kat_rfc8032_signature_identique_octet_a_octet() {
        for kat in KATS {
            let sk = SigningKey::from_seed(&hex32(kat.seed));
            assert_eq!(
                sk.sign(&hex(kat.message)),
                hex64(kat.signature),
                "{} : signature",
                kat.nom
            );
        }
    }

    #[test]
    fn kat_rfc8032_verification() {
        for kat in KATS {
            let vk = VerifyingKey::from_bytes(&hex32(kat.public)).expect("clé du RFC valide");
            assert_eq!(
                vk.verify(&hex(kat.message), &hex64(kat.signature)),
                Ok(()),
                "{} : verify",
                kat.nom
            );
        }
    }

    #[test]
    fn aller_retour_sur_plusieurs_longueurs() {
        let sk = cle_de_test();
        let vk = sk.verifying_key();
        for len in [0usize, 1, 31, 32, 63, 64, 65, 1000] {
            let message: Vec<u8> = (0..=250u8).cycle().take(len).collect();
            let sig = sk.sign(&message);
            assert_eq!(vk.verify(&message, &sig), Ok(()), "longueur {len}");
        }
    }

    #[test]
    fn la_signature_est_deterministe() {
        let sk = cle_de_test();
        assert_eq!(sk.sign(b"dengon"), sk.sign(b"dengon"));
        assert_ne!(sk.sign(b"dengon"), sk.sign(b"dengon!"));
    }

    #[test]
    fn la_meme_graine_donne_la_meme_cle_publique() {
        let a = SigningKey::from_seed(&[1u8; SEED_LEN]).verifying_key();
        let b = SigningKey::from_seed(&[1u8; SEED_LEN]).verifying_key();
        let c = SigningKey::from_seed(&[2u8; SEED_LEN]).verifying_key();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // ----- Tests négatifs -------------------------------------------------

    #[test]
    fn signature_forgee_rejetee() {
        let vk = cle_de_test().verifying_key();
        let message = b"message legitime";

        // Signature entièrement arbitraire.
        let forgee = [0xAB; SIGNATURE_LEN];
        assert_eq!(
            vk.verify(message, &forgee),
            Err(CryptoError::InvalidSignature)
        );

        // Signature nulle.
        assert_eq!(
            vk.verify(message, &[0u8; SIGNATURE_LEN]),
            Err(CryptoError::InvalidSignature)
        );

        // Signature valide d'une AUTRE clé sur le même message.
        let autre = SigningKey::from_seed(&[9u8; SEED_LEN]);
        assert_eq!(
            vk.verify(message, &autre.sign(message)),
            Err(CryptoError::InvalidSignature)
        );
    }

    #[test]
    fn message_bit_flippe_rejete_pour_chaque_bit() {
        let sk = cle_de_test();
        let vk = sk.verifying_key();
        let message = b"dengon: message a alterer".to_vec();
        let sig = sk.sign(&message);
        assert_eq!(vk.verify(&message, &sig), Ok(()), "témoin non altéré");

        for octet in 0..message.len() {
            for bit in 0..8 {
                let mut altere = message.clone();
                altere[octet] ^= 1 << bit;
                assert_eq!(
                    vk.verify(&altere, &sig),
                    Err(CryptoError::InvalidSignature),
                    "octet {octet}, bit {bit} accepté"
                );
            }
        }
    }

    #[test]
    fn signature_bit_flippee_rejetee_pour_chaque_bit() {
        let sk = cle_de_test();
        let vk = sk.verifying_key();
        let message = b"message signe";
        let sig = sk.sign(message);

        for octet in 0..SIGNATURE_LEN {
            for bit in 0..8 {
                let mut altere = sig;
                altere[octet] ^= 1 << bit;
                assert_eq!(
                    vk.verify(message, &altere),
                    Err(CryptoError::InvalidSignature),
                    "signature : octet {octet}, bit {bit} acceptée"
                );
            }
        }
    }

    #[test]
    fn message_tronque_ou_allonge_rejete() {
        let sk = cle_de_test();
        let vk = sk.verifying_key();
        let message = b"message complet";
        let sig = sk.sign(message);

        assert_eq!(
            vk.verify(&message[..message.len() - 1], &sig),
            Err(CryptoError::InvalidSignature)
        );
        assert_eq!(
            vk.verify(b"message complet\0", &sig),
            Err(CryptoError::InvalidSignature)
        );
        assert_eq!(vk.verify(b"", &sig), Err(CryptoError::InvalidSignature));
    }

    #[test]
    fn mauvaise_cle_publique_rejetee() {
        let sk = cle_de_test();
        let autre = SigningKey::from_seed(&[9u8; SEED_LEN]).verifying_key();
        let sig = sk.sign(b"message");
        assert_eq!(
            autre.verify(b"message", &sig),
            Err(CryptoError::InvalidSignature)
        );
    }

    #[test]
    fn cle_publique_invalide_rejetee_au_decodage() {
        // y = 2 n'est l'ordonnée d'aucun point de la courbe : le décodage
        // échoue (constaté à l'exécution).
        let mut invalide = [0u8; PUBLIC_KEY_LEN];
        invalide[0] = 2;
        assert_eq!(
            VerifyingKey::from_bytes(&invalide),
            Err(CryptoError::InvalidPublicKey)
        );
    }

    #[test]
    fn cle_de_faible_ordre_ne_valide_aucune_signature() {
        // Le point neutre (y = 1) est de faible ordre : le décodage l'accepte
        // (constaté à l'exécution), mais `verify_strict` doit refuser toute
        // signature « trivialement » forgée (R = neutre, S = 0).
        let mut neutre = [0u8; PUBLIC_KEY_LEN];
        neutre[0] = 1;
        let mut signature = [0u8; SIGNATURE_LEN];
        signature[0] = 1;

        let vk = VerifyingKey::from_bytes(&neutre).expect("le point neutre se décode");
        assert_eq!(
            vk.verify(b"n'importe quoi", &signature),
            Err(CryptoError::InvalidSignature)
        );
    }

    #[test]
    fn debug_ne_divulgue_pas_le_secret() {
        let seed = [0x5Au8; SEED_LEN];
        let affichage = format!("{:?}", SigningKey::from_seed(&seed));
        assert!(!affichage.contains("5a5a5a5a"));
        assert!(!affichage.contains("90, 90, 90"));
        assert!(affichage.contains("SigningKey"));
    }

    #[test]
    fn signe_le_journal_chaine() {
        use crate::ledger::{Ledger, Verdict};

        let mut journal = Ledger::new(cle_de_test());
        let entree = journal
            .append("msg.queued", r#"{"msg_uuid":"x"}"#, 1_000)
            .clone();
        assert_eq!(journal.verify_chain(), Verdict::Ok);
        assert_eq!(
            cle_de_test()
                .verifying_key()
                .verify(&entree.entry_hash, &entree.sig),
            Ok(())
        );
    }

    #[test]
    fn erreurs_affichables() {
        assert_eq!(
            format!("{}", CryptoError::InvalidSignature),
            "signature Ed25519 invalide"
        );
        assert_eq!(
            format!("{}", CryptoError::InvalidPublicKey),
            "clé publique Ed25519 invalide"
        );
        for e in [
            CryptoError::Noise,
            CryptoError::PayloadTooLarge,
            CryptoError::InvalidPadding,
            CryptoError::HandshakeNotFinished,
        ] {
            assert!(!format!("{e}").is_empty());
        }
    }
}
