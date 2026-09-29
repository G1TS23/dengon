//! Batch signé pour `POST /ingest/batch` (US-309).
//!
//! Référence : `contracts/events/CANONICAL.md` §2-3, `batch.schema.json`,
//! `dashboard/api/app/ingest.py` (ce que le serveur recalcule réellement).
//!
//! Un batch est l'objet `{batch_id, node_id, schema_version, events, sig}` :
//!
//! - `batch_id = hex(SHA-256(canonical_json(events)))` ;
//! - `sig = base64(Ed25519(canonical_json(batch sans sig)))`, base64
//!   standard avec bourrage — **une** signature pour tout le batch, pas une
//!   par événement.
//!
//! # Source : le journal chaîné
//!
//! Sur le relais, les événements sont d'abord des entrées du journal
//! (`ledger::Entry`) : c'est déjà là que `seq` est monotone et survit à un
//! redémarrage. [`Envelope::from_ledger_entry`] les convertit sans rien
//! inventer : `seq` et `ts_ms` sont ceux de l'entrée, `event_id` en découle,
//! et le payload texte est **re-canonicalisé** ([`canonical::parse`]) —
//! le serveur signe/vérifie sur `canonical_json` de ce qu'il reçoit, pas sur
//! le texte tel que le firmware l'a écrit.
//!
//! La clé Ed25519 ne quitte pas l'appelant : [`signed_batch`] prend une
//! référence à la `SigningKey` du nœud (côté relais, celle du handle
//! `Relay`, jamais exposée au C).

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

use super::{canonical, catalog, hex, Envelope, NodeKind, Value};
use crate::crypto::SigningKey;
use crate::ledger::Entry;
use crate::protocol::PeerId;

/// `schema_version` des batchs émis (`batch.schema.json`, fixtures v1).
pub const SCHEMA_VERSION: i64 = 1;

/// Nombre maximal d'événements par batch accepté par le serveur
/// (`batch.schema.json`, `maxItems`).
pub const MAX_EVENTS: usize = 1000;

/// Pourquoi un batch n'a pas pu être construit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchError {
    /// Aucun événement : le schéma exige `minItems: 1`.
    Empty,
    /// Plus de [`MAX_EVENTS`] événements.
    TooMany,
    /// Nom d'événement absent du catalogue MVP.
    UnknownEvent,
    /// Payload illisible, ou qui n'est pas un objet JSON.
    BadPayload,
    /// Une enveloppe porte un autre `node_id` que le batch : le dashboard
    /// refuserait le lot entier (400).
    NodeMismatch,
}

impl core::fmt::Display for BatchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            BatchError::Empty => "batch vide",
            BatchError::TooMany => "trop d'événements pour un batch",
            BatchError::UnknownEvent => "événement hors catalogue",
            BatchError::BadPayload => "payload JSON invalide",
            BatchError::NodeMismatch => "node_id d'enveloppe différent de celui du batch",
        })
    }
}

impl core::error::Error for BatchError {}

/// `node_id` d'un relais : `relay-` + les 3 premiers octets du `peerID` en
/// hex (`envelope.schema.json` : `^relay-[0-9a-f]{6}$`).
pub fn relay_node_id(peer_id: &PeerId) -> String {
    let mut s = String::from("relay-");
    s.push_str(&hex(&peer_id[..3]));
    s
}

impl Envelope {
    /// Enveloppe d'une entrée du journal chaîné — voir la note de module.
    pub fn from_ledger_entry(
        entry: &Entry,
        node_id: &str,
        node_kind: NodeKind,
    ) -> Result<Self, BatchError> {
        let name = catalog::lookup(&entry.event_name).ok_or(BatchError::UnknownEvent)?;
        let payload = canonical::parse(&entry.payload_json).map_err(|_| BatchError::BadPayload)?;
        if !matches!(payload, Value::Object(_)) {
            return Err(BatchError::BadPayload);
        }
        Ok(Envelope::new(
            node_id,
            node_kind,
            entry.seq,
            entry.ts_ms,
            name,
            payload,
        ))
    }
}

/// `canonical_json(events)` : sérialisé **une seule fois**, puis réutilisé
/// pour `batch_id` et dans le corps (mémoire comptée sur l'ESP32).
fn events_bytes(events: &[Envelope]) -> Vec<u8> {
    Value::Array(events.iter().map(Envelope::to_value).collect()).to_canonical_bytes()
}

/// `hex(SHA-256(canonical_json(events)))` — `CANONICAL.md` §3.
pub fn batch_id(events: &[Envelope]) -> String {
    hex(&Sha256::digest(events_bytes(events)))
}

/// Le corps HTTP complet d'un batch signé, en JSON canonique.
///
/// Chaque enveloppe doit porter `node_id` : le serveur exige qu'il soit
/// identique pour chaque événement et pour le batch
/// ([`BatchError::NodeMismatch`] sinon).
///
/// Le batch est assemblé à la main dans l'ordre canonique des clés
/// (`batch_id` < `events` < `node_id` < `schema_version` < `sig`) plutôt que
/// re-sérialisé depuis un [`Value`] : les événements ne sont ainsi convertis
/// en octets qu'une fois. `le_sig_raccroche_egale_la_serialisation_complete`
/// vérifie que le résultat est exactement la sérialisation canonique.
pub fn signed_batch(
    events: &[Envelope],
    node_id: &str,
    key: &SigningKey,
) -> Result<Vec<u8>, BatchError> {
    if events.is_empty() {
        return Err(BatchError::Empty);
    }
    if events.len() > MAX_EVENTS {
        return Err(BatchError::TooMany);
    }
    if events.iter().any(|e| e.node_id != node_id) {
        return Err(BatchError::NodeMismatch);
    }
    let events_json = events_bytes(events);
    let id = hex(&Sha256::digest(&events_json));
    let node_json = Value::Str(String::from(node_id)).to_canonical_bytes();

    let mut body = Vec::with_capacity(events_json.len() + node_json.len() + 200);
    body.extend_from_slice(b"{\"batch_id\":\"");
    body.extend_from_slice(id.as_bytes());
    body.extend_from_slice(b"\",\"events\":");
    body.extend_from_slice(&events_json);
    drop(events_json);
    body.extend_from_slice(b",\"node_id\":");
    body.extend_from_slice(&node_json);
    body.extend_from_slice(b",\"schema_version\":");
    body.extend_from_slice(SCHEMA_VERSION.to_string().as_bytes());
    body.push(b'}');

    // `body` est maintenant `canonical_json(batch sans sig)` : ce qu'on signe.
    // `sig` étant la plus grande clé, on la raccroche en dernier.
    let sig = data_encoding::BASE64.encode(&key.sign(&body));
    body.pop(); // '}'
    body.extend_from_slice(b",\"sig\":\"");
    body.extend_from_slice(sig.as_bytes());
    body.extend_from_slice(b"\"}");
    Ok(body)
}

/// Convertit puis signe des entrées du journal : ce que le relais envoie.
pub fn signed_batch_from_entries(
    entries: &[Entry],
    node_id: &str,
    node_kind: NodeKind,
    key: &SigningKey,
) -> Result<Vec<u8>, BatchError> {
    let events = entries
        .iter()
        .map(|e| Envelope::from_ledger_entry(e, node_id, node_kind))
        .collect::<Result<Vec<_>, _>>()?;
    signed_batch(&events, node_id, key)
}

#[cfg(test)]
mod tests;
