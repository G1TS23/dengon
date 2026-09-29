//! Payload `ANNOUNCE` (`0x01`) — présentation d'un nœud à ses voisins
//! (`docs/synthese/05-protocole-et-trame.md` §4, US-308).
//!
//! ```text
//! Announce = peerID(8) ‖ pub_static(32) ‖ pub_sign(32) ‖ pseudo_len(1) ‖ pseudo ‖ ledger_height(8) ‖ caps(1)
//! ```
//!
//! Entiers en big-endian. C'est par ce paquet (non adressé, toujours signé)
//! qu'un nœud apprend le `peerID` du voisin au bout d'un lien — la couche
//! transport ne le donne pas — et donc qu'il peut lui adresser `INVENTORY` et
//! `ENVELOPE_OFFER`.
//!
//! [`Announce::verify`] porte les deux contrôles anti-usurpation de
//! `synthese/05` §7 : `peerID == SHA-256(pub_static)[0..8]`, et signature
//! Ed25519 du paquet valide pour `pub_sign`.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use super::{received_signing_input, Packet, Reader};
use crate::crypto::noise::DH_LEN;
use crate::crypto::{VerifyingKey, PUBLIC_KEY_LEN};
use crate::protocol::types::{PacketType, PeerId};

/// Longueur maximale du pseudo (`pseudo_len` tient sur un octet).
pub const PSEUDO_MAX_LEN: usize = u8::MAX as usize;

/// Bit de `caps` : le nœud est un relais dédié (ESP32), pas un téléphone.
pub const CAP_RELAY: u8 = 1 << 0;

/// Contenu d'un `ANNOUNCE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announce {
    /// `peerID` revendiqué ; doit égaler `SHA-256(pub_static)[0..8]`.
    pub peer_id: PeerId,
    /// Clé publique X25519 (Noise).
    pub pub_static: [u8; DH_LEN],
    /// Clé publique Ed25519 (signatures).
    pub pub_sign: [u8; PUBLIC_KEY_LEN],
    /// Pseudo libre, UTF-8.
    pub pseudo: String,
    /// Hauteur du journal chaîné du nœud (nombre d'entrées).
    pub ledger_height: u64,
    /// Capacités (champ de bits, voir [`CAP_RELAY`]).
    pub caps: u8,
}

/// Payload `ANNOUNCE` invalide ou non authentique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnounceError {
    /// Payload tronqué, octets en trop, pseudo non UTF-8 ou trop long.
    Malformed,
    /// Le paquet n'est pas un `ANNOUNCE`, ou n'est pas signé.
    NotAnnounce,
    /// `peerID` ≠ `SHA-256(pub_static)[0..8]`, ou ≠ `sender_id` de l'en-tête.
    PeerIdMismatch,
    /// Signature absente ou invalide pour `pub_sign`.
    BadSignature,
}

impl fmt::Display for AnnounceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Malformed => "payload ANNOUNCE mal formé",
            Self::NotAnnounce => "paquet qui n'est pas un ANNOUNCE signé",
            Self::PeerIdMismatch => "peerID incohérent avec pub_static ou l'en-tête",
            Self::BadSignature => "signature ANNOUNCE invalide",
        })
    }
}

impl core::error::Error for AnnounceError {}

impl Announce {
    /// Sérialise le payload.
    ///
    /// # Errors
    ///
    /// [`AnnounceError::Malformed`] si le pseudo dépasse [`PSEUDO_MAX_LEN`].
    pub fn encode(&self) -> Result<Vec<u8>, AnnounceError> {
        let pseudo_len = u8::try_from(self.pseudo.len()).map_err(|_| AnnounceError::Malformed)?;
        let mut out = Vec::with_capacity(8 + DH_LEN + PUBLIC_KEY_LEN + 1 + self.pseudo.len() + 9);
        out.extend_from_slice(&self.peer_id);
        out.extend_from_slice(&self.pub_static);
        out.extend_from_slice(&self.pub_sign);
        out.push(pseudo_len);
        out.extend_from_slice(self.pseudo.as_bytes());
        out.extend_from_slice(&self.ledger_height.to_be_bytes());
        out.push(self.caps);
        Ok(out)
    }

    /// Relit un payload. Ne vérifie **pas** l'authenticité : voir
    /// [`Announce::verify`].
    ///
    /// # Errors
    ///
    /// [`AnnounceError::Malformed`] si le payload est tronqué, trop long ou
    /// si le pseudo n'est pas de l'UTF-8.
    pub fn decode(payload: &[u8]) -> Result<Self, AnnounceError> {
        let mut r = Reader::new(payload);
        let peer_id = r.take_array().ok_or(AnnounceError::Malformed)?;
        let pub_static = r.take_array().ok_or(AnnounceError::Malformed)?;
        let pub_sign = r.take_array().ok_or(AnnounceError::Malformed)?;
        let [pseudo_len] = r.take_array::<1>().ok_or(AnnounceError::Malformed)?;
        let pseudo = r
            .take(usize::from(pseudo_len))
            .ok_or(AnnounceError::Malformed)?;
        let pseudo = core::str::from_utf8(pseudo).map_err(|_| AnnounceError::Malformed)?;
        let height = r.take_array::<8>().ok_or(AnnounceError::Malformed)?;
        let [caps] = r.take_array::<1>().ok_or(AnnounceError::Malformed)?;
        if !r.take_rest().is_empty() {
            return Err(AnnounceError::Malformed);
        }
        Ok(Self {
            peer_id,
            pub_static,
            pub_sign,
            pseudo: String::from(pseudo),
            ledger_height: u64::from_be_bytes(height),
            caps,
        })
    }

    /// Décode **et authentifie** l'`ANNOUNCE` porté par `packet`, dont `raw`
    /// sont les octets reçus : `peerID` cohérent avec `pub_static` et avec
    /// l'en-tête, signature Ed25519 valide pour `pub_sign`.
    ///
    /// # Errors
    ///
    /// Voir [`AnnounceError`].
    pub fn verify(packet: &Packet, raw: &[u8]) -> Result<Self, AnnounceError> {
        if packet.header.packet_type != PacketType::Announce {
            return Err(AnnounceError::NotAnnounce);
        }
        let Some(signature) = packet.signature.as_ref() else {
            return Err(AnnounceError::NotAnnounce);
        };
        let announce = Self::decode(&packet.payload)?;
        if announce.peer_id != packet.header.sender_id
            || announce.peer_id != crate::identity::keys::peer_id_of(&announce.pub_static)
        {
            return Err(AnnounceError::PeerIdMismatch);
        }
        let key = VerifyingKey::from_bytes(&announce.pub_sign)
            .map_err(|_| AnnounceError::BadSignature)?;
        let input = received_signing_input(raw)
            .ok()
            .flatten()
            .ok_or(AnnounceError::BadSignature)?;
        key.verify(&input, signature)
            .map_err(|_| AnnounceError::BadSignature)?;
        Ok(announce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;
    use crate::protocol::codec::{decode, encode, signing_input};
    use crate::protocol::consts::PROTO_VERSION;
    use crate::protocol::types::{Flags, Header};
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    fn identite() -> Identity {
        match Identity::generate("relais", ChaCha20Rng::seed_from_u64(1)) {
            Ok(i) => i,
            Err(e) => panic!("{e}"),
        }
    }

    fn announce_de(id: &Identity) -> Announce {
        Announce {
            peer_id: id.peer_id(),
            pub_static: id.static_keypair().public(),
            pub_sign: id.signing_key().verifying_key().to_bytes(),
            pseudo: String::from(id.pseudo()),
            ledger_height: 12,
            caps: CAP_RELAY,
        }
    }

    fn paquet_signe(id: &Identity, a: &Announce) -> Vec<u8> {
        let payload = a.encode().unwrap();
        let mut p = Packet {
            header: Header {
                version: PROTO_VERSION,
                packet_type: PacketType::Announce,
                ttl: 1,
                flags: Flags::SIGNED,
                timestamp_ms: 1_000,
                sender_id: a.peer_id,
                recipient_id: None,
                payload_len: u16::try_from(payload.len()).unwrap(),
            },
            payload,
            signature: None,
        };
        p.signature = Some(id.signing_key().sign(&signing_input(&p).unwrap()));
        encode(&p).unwrap()
    }

    #[test]
    fn aller_retour() {
        let a = announce_de(&identite());
        assert_eq!(Announce::decode(&a.encode().unwrap()), Ok(a));
    }

    #[test]
    fn payload_tronque_ou_trop_long_refuse() {
        let octets = announce_de(&identite()).encode().unwrap();
        for n in 0..octets.len() {
            assert_eq!(
                Announce::decode(&octets[..n]),
                Err(AnnounceError::Malformed)
            );
        }
        let mut long = octets;
        long.push(0);
        assert_eq!(Announce::decode(&long), Err(AnnounceError::Malformed));
    }

    #[test]
    fn announce_authentique_verifie() {
        let id = identite();
        let brut = paquet_signe(&id, &announce_de(&id));
        let p = decode(&brut).unwrap();
        assert_eq!(Announce::verify(&p, &brut), Ok(announce_de(&id)));
    }

    #[test]
    fn peer_id_usurpe_refuse() {
        let id = identite();
        let mut a = announce_de(&id);
        a.peer_id = [0x42; 8];
        let brut = paquet_signe(&id, &a);
        let p = decode(&brut).unwrap();
        assert_eq!(
            Announce::verify(&p, &brut),
            Err(AnnounceError::PeerIdMismatch)
        );
    }

    #[test]
    fn signature_d_une_autre_cle_refusee() {
        let id = identite();
        let autre = match Identity::generate("x", ChaCha20Rng::seed_from_u64(2)) {
            Ok(i) => i,
            Err(e) => panic!("{e}"),
        };
        // Clés de `id` annoncées, mais paquet signé par `autre`.
        let brut = paquet_signe(&autre, &announce_de(&id));
        let p = decode(&brut).unwrap();
        assert_eq!(
            Announce::verify(&p, &brut),
            Err(AnnounceError::BadSignature)
        );
    }
}
