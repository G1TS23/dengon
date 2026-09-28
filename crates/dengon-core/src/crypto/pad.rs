//! Padding vers `PAD_BUCKETS` (US-204) — anti-analyse de trafic par la taille.
//!
//! Tout clair chiffré par Noise (`NOISE_HS`, `NOISE_MSG`, `SEALED_ENVELOPE`)
//! est d'abord complété jusqu'au plus petit bucket qui le contient
//! (`docs/synthese/06-securite.md` §3). Deux messages de tailles différentes
//! dans le même bucket donnent donc des chiffrés de **même taille**.
//!
//! # Format (écart vs conception)
//!
//! ```text
//! padded = len(u16 BE) ‖ données(len) ‖ 0x00 … 0x00      // taille ∈ PAD_BUCKETS
//! ```
//!
//! La conception prévoit du PKCS#7 ; c'est impossible ici : PKCS#7 code la
//! longueur du bourrage sur **un** octet (1 à 255), alors qu'il faut jusqu'à
//! 1023 octets de bourrage pour atteindre le bucket 2048. On préfixe donc la
//! longueur utile sur deux octets (big-endian, comme toute la trame) — voir
//! `docs/suivi/03-ecarts-conception.md`.
//!
//! Le bourrage est appliqué **avant** chiffrement : il est donc authentifié
//! par l'AEAD Noise et invisible pour un relais.

use alloc::vec::Vec;

use super::CryptoError;

/// Tailles cibles d'un clair paddé (source unique : `protocol::consts`).
pub use crate::protocol::consts::PAD_BUCKETS;

/// Longueur du préfixe de longueur.
const LEN_PREFIX: usize = 2;

/// Taille maximale de données utiles paddables (`2048 - 2`).
pub const MAX_PADDED_PAYLOAD: usize = PAD_BUCKETS[PAD_BUCKETS.len() - 1] - LEN_PREFIX;

/// Bucket cible pour `len` octets utiles, ou `None` si trop grand.
#[must_use]
pub fn bucket_for(len: usize) -> Option<usize> {
    let needed = len.checked_add(LEN_PREFIX)?;
    PAD_BUCKETS.iter().copied().find(|&b| b >= needed)
}

/// Complète `data` jusqu'au plus petit bucket qui le contient.
///
/// # Errors
///
/// [`CryptoError::PayloadTooLarge`] si `data` dépasse
/// [`MAX_PADDED_PAYLOAD`] : la fragmentation L2 est la responsabilité de
/// `protocol`, pas de `crypto`.
pub fn pad(data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let bucket = bucket_for(data.len()).ok_or(CryptoError::PayloadTooLarge)?;
    // `bucket_for` garantit data.len() <= 2046 : tient sur un u16.
    let len = u16::try_from(data.len()).map_err(|_| CryptoError::PayloadTooLarge)?;
    let mut out = Vec::with_capacity(bucket);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(data);
    out.resize(bucket, 0);
    Ok(out)
}

/// Retire le padding produit par [`pad`] et renvoie les données utiles.
///
/// Strict : la taille doit être un bucket, la longueur annoncée doit tenir,
/// et tout le bourrage doit être nul (un clair qui ne respecte pas le format
/// n'a pas été produit par [`pad`]).
///
/// # Errors
///
/// [`CryptoError::InvalidPadding`] dans tous les cas de non-conformité.
pub fn unpad(padded: &[u8]) -> Result<&[u8], CryptoError> {
    if !PAD_BUCKETS.contains(&padded.len()) {
        return Err(CryptoError::InvalidPadding);
    }
    let len = usize::from(u16::from_be_bytes([padded[0], padded[1]]));
    let end = LEN_PREFIX
        .checked_add(len)
        .filter(|&e| e <= padded.len())
        .ok_or(CryptoError::InvalidPadding)?;
    if padded[end..].iter().any(|&b| b != 0) {
        return Err(CryptoError::InvalidPadding);
    }
    Ok(&padded[LEN_PREFIX..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn taille_toujours_dans_les_buckets() {
        for len in [0usize, 1, 100, 254, 255, 510, 1022, 2046] {
            let p = pad(&vec![0xAA; len]).unwrap();
            assert!(PAD_BUCKETS.contains(&p.len()), "len {len} → {}", p.len());
        }
    }

    #[test]
    fn bornes_des_buckets() {
        // 254 + 2 = 256 tient pile ; 255 + 2 = 257 passe au bucket suivant.
        assert_eq!(pad(&[1; 254]).unwrap().len(), 256);
        assert_eq!(pad(&[1; 255]).unwrap().len(), 512);
        assert_eq!(pad(&[1; 510]).unwrap().len(), 512);
        assert_eq!(pad(&[1; 511]).unwrap().len(), 1024);
        assert_eq!(pad(&[1; 1022]).unwrap().len(), 1024);
        assert_eq!(pad(&[1; 1023]).unwrap().len(), 2048);
        assert_eq!(pad(&[1; MAX_PADDED_PAYLOAD]).unwrap().len(), 2048);
    }

    #[test]
    fn deux_tailles_differentes_meme_taille_paddee() {
        assert_eq!(pad(b"ok").unwrap().len(), pad(&[b'x'; 200]).unwrap().len());
    }

    #[test]
    fn trop_grand_rejete() {
        assert_eq!(
            pad(&vec![0; MAX_PADDED_PAYLOAD + 1]),
            Err(CryptoError::PayloadTooLarge)
        );
        assert_eq!(bucket_for(usize::MAX), None);
    }

    #[test]
    fn unpad_rejette_taille_hors_bucket() {
        assert_eq!(unpad(&[0; 255]), Err(CryptoError::InvalidPadding));
        assert_eq!(unpad(&[]), Err(CryptoError::InvalidPadding));
    }

    #[test]
    fn unpad_rejette_longueur_qui_deborde() {
        let mut p = pad(b"abc").unwrap();
        p[0..2].copy_from_slice(&255u16.to_be_bytes());
        assert_eq!(unpad(&p), Err(CryptoError::InvalidPadding));
    }

    #[test]
    fn unpad_rejette_bourrage_non_nul() {
        let mut p = pad(b"abc").unwrap();
        p[200] = 1;
        assert_eq!(unpad(&p), Err(CryptoError::InvalidPadding));
    }

    proptest! {
        #[test]
        fn aller_retour(data in proptest::collection::vec(any::<u8>(), 0..=MAX_PADDED_PAYLOAD)) {
            let p = pad(&data).unwrap();
            prop_assert!(PAD_BUCKETS.contains(&p.len()));
            prop_assert_eq!(unpad(&p).unwrap(), &data[..]);
        }
    }
}
