//! `recipient_tag` — adressage anonyme des enveloppes scellées (US-204).
//!
//! Formule réconciliée D-2 (`docs/synthese/06-securite.md` §3) :
//!
//! ```text
//! recipient_tag(day) = HMAC-SHA256( pub_static_destinataire , "dengon-tag" ‖ day_u32 )[0..16]
//! ```
//!
//! - clé HMAC = clé statique publique X25519 du destinataire (32 octets) ;
//! - `day_u32` = jour depuis l'époque Unix, encodé **big-endian** sur 4 octets
//!   (endianness de toute la trame, `05-protocole-et-trame.md` §3) ;
//! - sur le fil, `epoch_day` n'occupe que 2 octets (`SEALED_ENVELOPE`) : on
//!   manipule donc un `u16`, élargi en `u32` pour le HMAC.
//!
//! Le tag change chaque jour : un relais ne peut pas suivre un destinataire
//! dans le temps. Le destinataire recalcule ses tags J-1, J, J+1
//! ([`own_tags`]) pour tolérer la dérive d'horloge et le passage de minuit.

use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::noise::DH_LEN;

/// Longueur d'un `recipient_tag` (source unique : `protocol::consts`).
pub use crate::protocol::consts::RECIPIENT_TAG_LEN;

/// Étiquette de séparation de domaine du HMAC.
const TAG_LABEL: &[u8] = b"dengon-tag";

/// Millisecondes par jour.
const MS_PER_DAY: u64 = 86_400_000;

/// Un `recipient_tag` : 16 premiers octets du HMAC.
pub type RecipientTag = [u8; RECIPIENT_TAG_LEN];

/// Jour depuis l'époque Unix pour un horodatage en millisecondes.
///
/// Sature à `u16::MAX` au-delà de l'an 2149 (limite du champ `epoch_day`
/// de 2 octets, hors horizon du projet).
#[must_use]
pub fn epoch_day(ts_ms: u64) -> u16 {
    u16::try_from(ts_ms / MS_PER_DAY).unwrap_or(u16::MAX)
}

/// Calcule le `recipient_tag` de `pub_static` pour le jour `day`.
#[must_use]
pub fn recipient_tag(pub_static: &[u8; DH_LEN], day: u16) -> RecipientTag {
    // HMAC accepte une clé de n'importe quelle longueur : `new_from_slice`
    // ne peut pas échouer pour HMAC-SHA256.
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(pub_static)
        .unwrap_or_else(|_| unreachable!("HMAC accepte toute longueur de clé"));
    mac.update(TAG_LABEL);
    mac.update(&u32::from(day).to_be_bytes());
    let full = mac.finalize().into_bytes();
    let mut tag = [0u8; RECIPIENT_TAG_LEN];
    tag.copy_from_slice(&full[..RECIPIENT_TAG_LEN]);
    tag
}

/// Tags à surveiller par le destinataire : J-1, J, J+1.
#[must_use]
pub fn own_tags(pub_static: &[u8; DH_LEN], day: u16) -> [RecipientTag; 3] {
    [
        recipient_tag(pub_static, day.saturating_sub(1)),
        recipient_tag(pub_static, day),
        recipient_tag(pub_static, day.saturating_add(1)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUB_A: [u8; DH_LEN] = [0x11; DH_LEN];
    const PUB_B: [u8; DH_LEN] = [0x22; DH_LEN];

    #[test]
    fn stable_sur_la_journee() {
        let debut = 20_000 * MS_PER_DAY;
        let fin = debut + MS_PER_DAY - 1;
        assert_eq!(epoch_day(debut), epoch_day(fin));
        assert_eq!(
            recipient_tag(&PUB_A, epoch_day(debut)),
            recipient_tag(&PUB_A, epoch_day(fin))
        );
    }

    #[test]
    fn different_le_lendemain() {
        let jour = epoch_day(20_000 * MS_PER_DAY);
        let lendemain = epoch_day(20_001 * MS_PER_DAY);
        assert_eq!(lendemain, jour + 1);
        assert_ne!(
            recipient_tag(&PUB_A, jour),
            recipient_tag(&PUB_A, lendemain)
        );
    }

    #[test]
    fn different_par_destinataire() {
        assert_ne!(recipient_tag(&PUB_A, 20_000), recipient_tag(&PUB_B, 20_000));
    }

    #[test]
    fn conforme_a_la_formule() {
        // Recalcul indépendant de la formule D-2, octet par octet.
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&PUB_A).unwrap();
        mac.update(b"dengon-tag");
        mac.update(&[0x00, 0x00, 0x4e, 0x20]); // 20 000 en u32 BE
        let attendu = mac.finalize().into_bytes();
        assert_eq!(recipient_tag(&PUB_A, 20_000)[..], attendu[..16]);
    }

    #[test]
    fn own_tags_couvre_hier_aujourdhui_demain() {
        let tags = own_tags(&PUB_A, 20_000);
        assert_eq!(tags[0], recipient_tag(&PUB_A, 19_999));
        assert_eq!(tags[1], recipient_tag(&PUB_A, 20_000));
        assert_eq!(tags[2], recipient_tag(&PUB_A, 20_001));
        // Pas de débordement aux bornes.
        let _ = own_tags(&PUB_A, 0);
        let _ = own_tags(&PUB_A, u16::MAX);
    }

    #[test]
    fn epoch_day_sature() {
        assert_eq!(epoch_day(0), 0);
        assert_eq!(epoch_day(u64::MAX), u16::MAX);
    }
}
