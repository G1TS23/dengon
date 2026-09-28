//! Conversions entre les types de `dengon_core::api` / `identity` (octets,
//! tableaux fixes) et les types du contrat FFI (chaînes, `Vec<u8>`).
//!
//! Formats des identifiants en chaîne (voir l'en-tête de `dengon.udl`) :
//! - `peer_id` : base32 minuscules sans padding, celui qu'affiche déjà
//!   [`identity::peer_id_base32`] ;
//! - `conv_id` / `msg_uuid` : hexadécimal minuscule.

use dengon_core::api;
use dengon_core::crypto::noise::DH_LEN;
use dengon_core::crypto::{VerifyingKey, PUBLIC_KEY_LEN};
use dengon_core::identity::{self, PublicIdentity};
use dengon_core::protocol::PeerId;

use crate::{Conversation, DengonError, Identity, Message, MessageStatus, NodeEvent};

pub(crate) fn peer_id_to_string(peer_id: &PeerId) -> String {
    identity::peer_id_base32(peer_id)
}

/// # Errors
///
/// [`DengonError::UnknownPeer`] si la chaîne n'est pas le base32 d'un
/// `peerID` de 8 octets : aucun pair ne peut porter cet identifiant.
pub(crate) fn peer_id_from_str(s: &str) -> Result<PeerId, DengonError> {
    data_encoding::BASE32_NOPAD
        .decode(s.to_ascii_uppercase().as_bytes())
        .ok()
        .and_then(|octets| PeerId::try_from(octets.as_slice()).ok())
        .ok_or(DengonError::UnknownPeer)
}

pub(crate) fn to_hex(bytes: &[u8]) -> String {
    data_encoding::HEXLOWER.encode(bytes)
}

pub(crate) fn from_hex<const N: usize>(s: &str) -> Option<[u8; N]> {
    let octets = data_encoding::HEXLOWER_PERMISSIVE
        .decode(s.as_bytes())
        .ok()?;
    <[u8; N]>::try_from(octets.as_slice()).ok()
}

pub(crate) fn identity_to_ffi(identity: &PublicIdentity) -> Identity {
    Identity {
        peer_id: peer_id_to_string(&identity.peer_id()),
        pseudo: identity.pseudo().into(),
        pub_static: identity.pub_static().to_vec(),
        pub_sign: identity.pub_sign().to_bytes().to_vec(),
    }
}

/// Reconstruit une carte de contact depuis le FFI, en revérifiant tout ce
/// que Kotlin a pu altérer : longueurs, clé Ed25519, pseudo, et surtout que
/// le `peer_id` annoncé est bien celui qui dérive de `pub_static`.
///
/// # Errors
///
/// [`DengonError::Internal`] si l'un de ces contrôles échoue.
pub(crate) fn identity_from_ffi(identity: &Identity) -> Result<PublicIdentity, DengonError> {
    let pub_static = <[u8; DH_LEN]>::try_from(identity.pub_static.as_slice())
        .map_err(|_| DengonError::Internal)?;
    let pub_sign = <[u8; PUBLIC_KEY_LEN]>::try_from(identity.pub_sign.as_slice())
        .map_err(|_| DengonError::Internal)?;
    let pub_sign = VerifyingKey::from_bytes(&pub_sign).map_err(|_| DengonError::Internal)?;
    let public = PublicIdentity::new(&identity.pseudo, pub_static, pub_sign)?;
    if peer_id_to_string(&public.peer_id()) != identity.peer_id {
        return Err(DengonError::Internal);
    }
    Ok(public)
}

pub(crate) fn status_to_ffi(status: api::MessageStatus) -> MessageStatus {
    match status {
        api::MessageStatus::Queued => MessageStatus::Queued,
        api::MessageStatus::InFlight => MessageStatus::InFlight,
        api::MessageStatus::Delivered => MessageStatus::Delivered,
        api::MessageStatus::Expired => MessageStatus::Expired,
    }
}

pub(crate) fn message_to_ffi(message: api::Message) -> Message {
    Message {
        msg_uuid: to_hex(&message.msg_uuid),
        conv_id: to_hex(&message.conv_id),
        author_peer_id: peer_id_to_string(&message.author_peer_id),
        body: message.body,
        outgoing: message.outgoing,
        // Au-delà de i64::MAX ms (an 292 278 994) : borné plutôt que négatif.
        sent_ms: i64::try_from(message.sent_ms).unwrap_or(i64::MAX),
        status: status_to_ffi(message.status),
    }
}

pub(crate) fn conversation_to_ffi(conversation: api::Conversation) -> Conversation {
    Conversation {
        conv_id: to_hex(&conversation.conv_id),
        peer_id: peer_id_to_string(&conversation.peer_id),
        peer_pseudo: conversation.peer_pseudo,
        last_message: conversation.last_message.map(message_to_ffi),
        unread_count: conversation.unread_count,
    }
}

pub(crate) fn event_to_ffi(event: api::NodeEvent) -> NodeEvent {
    match event {
        api::NodeEvent::MessageReceived(message) => NodeEvent::MessageReceived {
            message: message_to_ffi(message),
        },
        api::NodeEvent::StatusChanged(msg_uuid, status) => NodeEvent::StatusChanged {
            msg_uuid: to_hex(&msg_uuid),
            status: status_to_ffi(status),
        },
        api::NodeEvent::PeerConnected(peer_id) => NodeEvent::PeerConnected {
            peer_id: peer_id_to_string(&peer_id),
        },
        api::NodeEvent::PeerDisconnected(peer_id) => NodeEvent::PeerDisconnected {
            peer_id: peer_id_to_string(&peer_id),
        },
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn peer_id_aller_retour() {
        let id: PeerId = [0, 1, 2, 3, 0xfd, 0xfe, 0xff, 0x80];
        let s = peer_id_to_string(&id);
        assert_eq!(s.len(), 13);
        assert_eq!(s, s.to_ascii_lowercase());
        assert_eq!(peer_id_from_str(&s).unwrap(), id);
        // Tolère les majuscules (saisie ou copie manuelle).
        assert_eq!(peer_id_from_str(&s.to_ascii_uppercase()).unwrap(), id);
    }

    #[test]
    fn hex_aller_retour_et_longueur_stricte() {
        let uuid = [0xab_u8; 16];
        let s = to_hex(&uuid);
        assert_eq!(s, "ab".repeat(16));
        assert_eq!(from_hex::<16>(&s), Some(uuid));
        assert_eq!(from_hex::<8>(&s), None);
        assert_eq!(from_hex::<16>("zz"), None);
    }
}
