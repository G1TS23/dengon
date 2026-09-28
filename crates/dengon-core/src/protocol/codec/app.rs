//! Frames applicatives L4 — le **clair** transporté dans un `NOISE_MSG` ou
//! une `SEALED_ENVELOPE` (`docs/synthese/05-protocole-et-trame.md` §4.1).
//!
//! ```text
//! AppFrame  = kind(1) ‖ body
//!   kind 1 : Message -> msg_uuid(16) ‖ conv_seq(8) ‖ sent_ms(8) ‖ text_utf8
//!   kind 2 : Ack     -> AckFrame
//! AckFrame  = msg_uuid(16) ‖ status(1) ‖ at_ms(8)
//! ```
//!
//! Seuls les kinds MVP (`Message`, `Ack`) sont décodés ; `ReadReceipt` (v2)
//! et `Profile` (post-MVP) sont refusés avec
//! [`AppDecodeError::UnsupportedKind`], sans deviner leur format.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use super::Reader;
use crate::protocol::types::{AckStatus, AppFrameKind};

/// Longueur de `msg_uuid` (UUIDv4 brut).
pub const MSG_UUID_LEN: usize = 16;
/// Partie fixe d'un corps `Message` : `msg_uuid ‖ conv_seq ‖ sent_ms`.
pub const MESSAGE_FIXED_LEN: usize = MSG_UUID_LEN + 8 + 8;
/// Longueur exacte d'un `AckFrame`.
pub const ACK_FRAME_LEN: usize = MSG_UUID_LEN + 1 + 8;

/// Identifiant de message applicatif, stable de bout en bout (§3.2).
pub type MsgUuid = [u8; MSG_UUID_LEN];

/// Corps d'une frame `Message` (kind 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageFrame {
    pub msg_uuid: MsgUuid,
    /// Compteur par conversation : trous, ordre causal, anti-rejeu (C-7).
    pub conv_seq: u64,
    /// Horloge de l'émetteur, ms UTC.
    pub sent_ms: u64,
    /// Texte UTF-8, sans terminateur : il occupe tout le reste de la frame.
    pub text: String,
}

/// `AckFrame` (kind 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckFrame {
    pub msg_uuid: MsgUuid,
    pub status: AckStatus,
    pub at_ms: u64,
}

/// Frame applicative décodée (sous-ensemble MVP).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppFrame {
    Message(MessageFrame),
    Ack(AckFrame),
}

impl AppFrame {
    /// Le `kind` porté sur le fil.
    #[must_use]
    pub const fn kind(&self) -> AppFrameKind {
        match self {
            Self::Message(_) => AppFrameKind::Message,
            Self::Ack(_) => AppFrameKind::Ack,
        }
    }
}

/// Raison du refus d'une frame par [`decode_app_frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppDecodeError {
    /// Frame vide : pas d'octet `kind`.
    Empty,
    /// Octet `kind` inconnu.
    UnknownKind(u8),
    /// `kind` connu mais hors MVP (`ReadReceipt`, `Profile`).
    UnsupportedKind(AppFrameKind),
    /// Corps plus court que sa partie fixe.
    Truncated {
        kind: AppFrameKind,
        needed: usize,
        got: usize,
    },
    /// `AckFrame` suivi d'octets en trop.
    TrailingBytes { kind: AppFrameKind, extra: usize },
    /// Octet `status` d'un `AckFrame` inconnu.
    UnknownAckStatus(u8),
    /// Texte d'un `Message` qui n'est pas de l'UTF-8 valide.
    InvalidUtf8,
}

impl fmt::Display for AppDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("frame applicative vide"),
            Self::UnknownKind(k) => write!(f, "kind de frame inconnu {k}"),
            Self::UnsupportedKind(k) => write!(f, "kind {k:?} hors périmètre MVP"),
            Self::Truncated { kind, needed, got } => {
                write!(f, "{kind:?} tronqué : {got} octets, {needed} attendus")
            }
            Self::TrailingBytes { kind, extra } => {
                write!(f, "{kind:?} suivi de {extra} octets en trop")
            }
            Self::UnknownAckStatus(s) => write!(f, "statut d'ACK inconnu {s}"),
            Self::InvalidUtf8 => f.write_str("texte de message non UTF-8"),
        }
    }
}

impl core::error::Error for AppDecodeError {}

/// Sérialise `frame` à la fin de `out`. Infaillible : tout [`AppFrame`] est
/// représentable (le texte n'a pas de longueur maximale à ce niveau ; la
/// taille est bornée par le paquet L3 et le padding).
pub fn encode_app_frame_into(frame: &AppFrame, out: &mut Vec<u8>) {
    out.push(frame.kind().to_u8());
    match frame {
        AppFrame::Message(m) => {
            out.reserve(MESSAGE_FIXED_LEN + m.text.len());
            out.extend_from_slice(&m.msg_uuid);
            out.extend_from_slice(&m.conv_seq.to_be_bytes());
            out.extend_from_slice(&m.sent_ms.to_be_bytes());
            out.extend_from_slice(m.text.as_bytes());
        }
        AppFrame::Ack(a) => {
            out.extend_from_slice(&a.msg_uuid);
            out.push(a.status.to_u8());
            out.extend_from_slice(&a.at_ms.to_be_bytes());
        }
    }
}

/// Sérialise `frame` dans un nouveau tampon.
#[must_use]
pub fn encode_app_frame(frame: &AppFrame) -> Vec<u8> {
    let mut out = Vec::new();
    encode_app_frame_into(frame, &mut out);
    out
}

/// Désérialise une frame applicative (le clair issu du déchiffrement Noise).
///
/// Ne panique sur aucune entrée.
///
/// # Errors
///
/// [`AppDecodeError`] si les octets ne forment pas exactement une frame MVP.
pub fn decode_app_frame(raw: &[u8]) -> Result<AppFrame, AppDecodeError> {
    let (&kind_byte, body) = raw.split_first().ok_or(AppDecodeError::Empty)?;
    let kind = AppFrameKind::from_u8(kind_byte).ok_or(AppDecodeError::UnknownKind(kind_byte))?;
    let mut r = Reader::new(body);
    match kind {
        AppFrameKind::Message => {
            let truncated = AppDecodeError::Truncated {
                kind,
                needed: MESSAGE_FIXED_LEN,
                got: body.len(),
            };
            let msg_uuid = r.take_array::<MSG_UUID_LEN>().ok_or(truncated)?;
            let conv_seq = u64::from_be_bytes(r.take_array::<8>().ok_or(truncated)?);
            let sent_ms = u64::from_be_bytes(r.take_array::<8>().ok_or(truncated)?);
            let text = core::str::from_utf8(r.take_rest())
                .map_err(|_| AppDecodeError::InvalidUtf8)?
                .into();
            Ok(AppFrame::Message(MessageFrame {
                msg_uuid,
                conv_seq,
                sent_ms,
                text,
            }))
        }
        AppFrameKind::Ack => {
            let truncated = AppDecodeError::Truncated {
                kind,
                needed: ACK_FRAME_LEN,
                got: body.len(),
            };
            let msg_uuid = r.take_array::<MSG_UUID_LEN>().ok_or(truncated)?;
            let [status_byte] = r.take_array::<1>().ok_or(truncated)?;
            let at_ms = u64::from_be_bytes(r.take_array::<8>().ok_or(truncated)?);
            let extra = r.take_rest().len();
            if extra != 0 {
                return Err(AppDecodeError::TrailingBytes { kind, extra });
            }
            let status = AckStatus::from_u8(status_byte)
                .ok_or(AppDecodeError::UnknownAckStatus(status_byte))?;
            Ok(AppFrame::Ack(AckFrame {
                msg_uuid,
                status,
                at_ms,
            }))
        }
        AppFrameKind::ReadReceipt | AppFrameKind::Profile => {
            Err(AppDecodeError::UnsupportedKind(kind))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    fn message(text: &str) -> AppFrame {
        AppFrame::Message(MessageFrame {
            msg_uuid: [0x11; MSG_UUID_LEN],
            conv_seq: 3,
            sent_ms: 0x0191_0000_0000,
            text: text.into(),
        })
    }

    fn ack() -> AppFrame {
        AppFrame::Ack(AckFrame {
            msg_uuid: [0x22; MSG_UUID_LEN],
            status: AckStatus::Delivered,
            at_ms: 7,
        })
    }

    #[test]
    fn message_aller_retour_et_disposition() {
        let f = message("salut ☕");
        let raw = encode_app_frame(&f);
        assert_eq!(raw[0], 1);
        assert_eq!(&raw[1..17], &[0x11; MSG_UUID_LEN]);
        assert_eq!(&raw[17..25], &3u64.to_be_bytes());
        assert_eq!(&raw[33..], "salut ☕".as_bytes());
        assert_eq!(decode_app_frame(&raw).unwrap(), f);
    }

    #[test]
    fn message_texte_vide() {
        let f = message("");
        let raw = encode_app_frame(&f);
        assert_eq!(raw.len(), 1 + MESSAGE_FIXED_LEN);
        assert_eq!(decode_app_frame(&raw).unwrap(), f);
    }

    #[test]
    fn ack_aller_retour_et_longueur_exacte() {
        let f = ack();
        let raw = encode_app_frame(&f);
        assert_eq!(raw.len(), 1 + ACK_FRAME_LEN);
        assert_eq!(raw[17], AckStatus::Delivered.to_u8());
        assert_eq!(decode_app_frame(&raw).unwrap(), f);
    }

    #[test]
    fn erreurs() {
        assert_eq!(decode_app_frame(&[]), Err(AppDecodeError::Empty));
        assert_eq!(decode_app_frame(&[0]), Err(AppDecodeError::UnknownKind(0)));
        assert_eq!(
            decode_app_frame(&[3, 0, 0]),
            Err(AppDecodeError::UnsupportedKind(AppFrameKind::ReadReceipt))
        );
        assert_eq!(
            decode_app_frame(&[4]),
            Err(AppDecodeError::UnsupportedKind(AppFrameKind::Profile))
        );

        let raw = encode_app_frame(&message("x"));
        assert_eq!(
            decode_app_frame(&raw[..10]),
            Err(AppDecodeError::Truncated {
                kind: AppFrameKind::Message,
                needed: MESSAGE_FIXED_LEN,
                got: 9
            })
        );
        let mut bad = raw;
        bad.push(0xff);
        assert_eq!(decode_app_frame(&bad), Err(AppDecodeError::InvalidUtf8));

        let raw = encode_app_frame(&ack());
        assert_eq!(
            decode_app_frame(&raw[..raw.len() - 1]),
            Err(AppDecodeError::Truncated {
                kind: AppFrameKind::Ack,
                needed: ACK_FRAME_LEN,
                got: ACK_FRAME_LEN - 1
            })
        );
        let mut long = raw.clone();
        long.push(0);
        assert_eq!(
            decode_app_frame(&long),
            Err(AppDecodeError::TrailingBytes {
                kind: AppFrameKind::Ack,
                extra: 1
            })
        );
        let mut bad_status = raw;
        bad_status[17] = 1;
        assert_eq!(
            decode_app_frame(&bad_status),
            Err(AppDecodeError::UnknownAckStatus(1))
        );
    }

    #[test]
    fn messages_d_erreur_non_vides() {
        let errors = vec![
            AppDecodeError::Empty,
            AppDecodeError::UnknownKind(9),
            AppDecodeError::UnsupportedKind(AppFrameKind::Profile),
            AppDecodeError::Truncated {
                kind: AppFrameKind::Ack,
                needed: 25,
                got: 1,
            },
            AppDecodeError::TrailingBytes {
                kind: AppFrameKind::Ack,
                extra: 1,
            },
            AppDecodeError::UnknownAckStatus(0),
            AppDecodeError::InvalidUtf8,
        ];
        for e in errors {
            assert!(!e.to_string().is_empty());
        }
    }
}
