//! Format de trame L3 dengon — **types et constantes uniquement** (US-108).
//!
//! Fait foi : [`docs/powl/03-network-protocol.md`] (décision A-12), résumé dans
//! [`docs/synthese/05-protocole-et-trame.md`]. Le brouillon
//! `docs/olivier/format-trame.md` est **remplacé** (conservé pour mémoire).
//!
//! Les **types** (US-108) ont été livrés avant l'encodage / décodage octet par
//! octet ([`codec`], US-201), pour que `sync::*` puisse démarrer sans attendre
//! la sérialisation.
//!
//! # Contenu
//!
//! - [`consts`] — toutes les constantes du protocole (`synthese/05` §2).
//! - [`types`] — [`PacketType`], [`Flags`], [`Header`], [`AppFrameKind`],
//!   [`AckStatus`], et les alias d'identifiants ([`PeerId`], [`MsgId`]).
//! - [`codec`] — octets ⇄ [`Packet`] (L3) et ⇄ [`AppFrame`] (L4, clair Noise).
//!
//! Les **vecteurs de conformité v0** vivent dans
//! `crates/dengon-core/tests/vectors_v0.json` (neutre en langage) et sont
//! contrôlés par `tests/protocol_vectors.rs`. Ils sont la base du job CI
//! `cross-vectors` (US-222) : le firmware et le dashboard consomment le même
//! fichier. Un futur déplacement vers `contracts/packet/` est possible une
//! fois ce dossier stabilisé sur `main` (voir `03-ecarts-conception.md`).

pub mod codec;
pub mod consts;
pub mod types;

pub use codec::app::{
    decode_app_frame, encode_app_frame, encode_app_frame_into, AckFrame, AppDecodeError, AppFrame,
    MessageFrame, MsgUuid,
};
pub use codec::{
    decode, encode, encode_into, signed_len, DecodeError, EncodeError, FrameRule, Packet,
};
pub use consts::*;
pub use types::{AckStatus, AppFrameKind, Flags, Header, MsgId, PacketType, PeerId, Signature};
