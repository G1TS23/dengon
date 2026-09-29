//! Émission d'événements d'observabilité (US-208).
//!
//! Référence : `docs/powl/08-observability-events.md` (catalogue complet
//! par domaine), `docs/synthese/09-dashboard-et-donnees.md` §9,
//! `contracts/tools/catalogue.py` + `contracts/events/CANONICAL.md`
//! (source de vérité côté contrat, Python — ce module produit les mêmes
//! octets canoniques pour les mêmes événements, vérifié ci-dessous
//! octet à octet contre trois fixtures golden réelles de l'US-107).
//!
//! # Redaction obligatoire avant émission
//!
//! `docs/powl/08` §1 point 3 : jamais de `msg_uuid` (l'identifiant brut,
//! 32 o), jamais de texte de message, jamais de destinataire en clair dans
//! un événement — seulement `msg_log_id = hex(SHA-256(msg_uuid)[0..8])`.
//! Ce module applique la règle **structurellement**, pas seulement par
//! convention : les fonctions de construction (`pkt_seen`, `msg_queued`,
//! `peer_connected`, …) n'acceptent que des `MsgLogId` déjà redactés (8 o)
//! ou des `PeerId` (déjà tronqués à 8 o par `protocol::PeerId`) — jamais de
//! `MsgId` brut (32 o, `protocol::MsgId`) ni de `&str` de texte libre.
//! [`msg_log_id`] est le seul point de passage du brut vers le redacté ;
//! voir `un_msg_uuid_brut_ne_peut_jamais_apparaitre_dans_un_evenement`
//! pour la preuve.
//!
//! # Portée réelle de ce module (US-208)
//!
//! Ce module fournit le **mécanisme** d'émission (catalogue, redaction,
//! sérialisation canonique) et quatre constructeurs représentatifs
//! (`pkt_seen`, `pkt_relayed`, `msg_queued`, `peer_connected`) — pas
//! l'intégralité des 28 événements du catalogue, et **aucun site
//! d'appel réel** : `sync::routing`/`sync::inventory` (US-209/US-210),
//! qui produiraient réellement `pkt.*`, ne sont pas encore livrés à
//! l'heure où ce module est écrit. Écart consigné dans
//! `03-ecarts-conception.md`.

extern crate alloc;

pub mod batch;
pub mod canonical;
pub mod catalog;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

pub use canonical::Value;

use crate::protocol::PeerId;

/// Identifiant de message **déjà redacté** : `SHA-256(msg_uuid)[0..8]`,
/// jamais le `MsgId` brut (32 o). Voir la note de module.
pub type MsgLogId = [u8; 8];

/// `SHA-256(msg_uuid)[0..8]` — jamais `msg_uuid` lui-même. Seul endroit du
/// module qui manipule un identifiant brut ; tout le reste de l'API ne
/// connaît que des [`MsgLogId`] déjà redactés.
pub fn msg_log_id(msg_uuid: &[u8]) -> MsgLogId {
    let digest = Sha256::digest(msg_uuid);
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[0..8]);
    out
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(DIGITS[(b >> 4) as usize] as char);
        s.push(DIGITS[(b & 0x0f) as usize] as char);
    }
    s
}

/// `relay` ou `client` — `docs/powl/08` §1, champ `node_kind` de
/// l'enveloppe commune.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Relay,
    Client,
}

impl NodeKind {
    fn as_str(self) -> &'static str {
        match self {
            NodeKind::Relay => "relay",
            NodeKind::Client => "client",
        }
    }
}

/// Une enveloppe d'événement (`docs/powl/08` §1, forme commune à tout
/// événement) — sans `sig` : la signature porte sur le **batch** entier
/// (`CANONICAL.md` §2), pas sur un événement isolé, donc hors du périmètre
/// de ce type.
#[derive(Debug, Clone, PartialEq)]
pub struct Envelope {
    pub event_id: String,
    pub node_id: String,
    pub node_kind: NodeKind,
    pub seq: u64,
    pub ts_ms: u64,
    pub name: &'static str,
    pub payload: Value,
}

impl Envelope {
    /// `event_id = hex(SHA-256(node_id ‖ seq_big_endian_u64))`
    /// (`docs/powl/08` §1 ; `contracts/tools/catalogue.py::event_id`).
    pub fn new(
        node_id: &str,
        node_kind: NodeKind,
        seq: u64,
        ts_ms: u64,
        name: &'static str,
        payload: Value,
    ) -> Self {
        let mut buf = Vec::with_capacity(node_id.len() + 8);
        buf.extend_from_slice(node_id.as_bytes());
        buf.extend_from_slice(&seq.to_be_bytes());
        let digest = Sha256::digest(&buf);
        Envelope {
            event_id: hex(&digest),
            node_id: String::from(node_id),
            node_kind,
            seq,
            ts_ms,
            name,
            payload,
        }
    }

    /// L'enveloppe (`event_id`/`node_id`/`node_kind`/`seq`/`ts_ms`/`name`/
    /// `payload`) en JSON canonique — PAS le batch : pas de `sig`, la
    /// couche appelante signe l'ensemble du batch (`CANONICAL.md` §2), pas
    /// un événement isolé.
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        self.to_value().to_canonical_bytes()
    }

    /// L'enveloppe sous forme de [`Value`] — ce qu'un batch range dans son
    /// tableau `events` (US-309, [`batch`]).
    pub fn to_value(&self) -> Value {
        let mut obj = BTreeMap::new();
        obj.insert(String::from("event_id"), Value::Str(self.event_id.clone()));
        obj.insert(String::from("node_id"), Value::Str(self.node_id.clone()));
        obj.insert(
            String::from("node_kind"),
            Value::Str(String::from(self.node_kind.as_str())),
        );
        // `try_from` + saturation plutôt qu'un cast brut (même style que
        // `ledger`/`store`, `cast_possible_wrap` actif workspace-wide) :
        // `seq`/`ts_ms` dépassant `i64::MAX` n'a aucun sens pratique ici
        // (ts_ms en millisecondes depuis epoch, seq un compteur de
        // journal) — saturer évite un panic sans prétendre gérer un cas
        // qui ne se produira jamais.
        obj.insert(
            String::from("seq"),
            Value::Int(i64::try_from(self.seq).unwrap_or(i64::MAX)),
        );
        obj.insert(
            String::from("ts_ms"),
            Value::Int(i64::try_from(self.ts_ms).unwrap_or(i64::MAX)),
        );
        obj.insert(String::from("name"), Value::Str(String::from(self.name)));
        obj.insert(String::from("payload"), self.payload.clone());
        Value::Object(obj)
    }
}

fn peer_hex(peer: PeerId) -> String {
    hex(&peer)
}

/// `pkt.seen` — `docs/powl/08` §2. `msg_log_id` doit déjà être redacté
/// (voir [`msg_log_id`]), jamais un `MsgId` brut.
#[allow(clippy::too_many_arguments)] // reflète 1:1 les champs du payload catalogue
pub fn pkt_seen(
    msg_log_id: MsgLogId,
    pkt_type: u8,
    ttl_in: u8,
    size_bucket: u16,
    from_peer: PeerId,
    rssi: Option<i8>,
) -> Value {
    let mut obj = BTreeMap::new();
    obj.insert(String::from("msg_log_id"), Value::Str(hex(&msg_log_id)));
    obj.insert(String::from("type"), Value::Int(i64::from(pkt_type)));
    obj.insert(String::from("ttl_in"), Value::Int(i64::from(ttl_in)));
    obj.insert(
        String::from("size_bucket"),
        Value::Int(i64::from(size_bucket)),
    );
    obj.insert(String::from("from_peer"), Value::Str(peer_hex(from_peer)));
    if let Some(rssi) = rssi {
        obj.insert(String::from("rssi"), Value::Int(i64::from(rssi)));
    }
    Value::Object(obj)
}

/// `pkt.relayed` — `docs/powl/08` §2.
#[allow(clippy::too_many_arguments)]
pub fn pkt_relayed(
    msg_log_id: MsgLogId,
    pkt_type: u8,
    ttl_in: u8,
    ttl_out: u8,
    fanout: u8,
    from_peer: PeerId,
) -> Value {
    let mut obj = BTreeMap::new();
    obj.insert(String::from("msg_log_id"), Value::Str(hex(&msg_log_id)));
    obj.insert(String::from("type"), Value::Int(i64::from(pkt_type)));
    obj.insert(String::from("ttl_in"), Value::Int(i64::from(ttl_in)));
    obj.insert(String::from("ttl_out"), Value::Int(i64::from(ttl_out)));
    obj.insert(String::from("fanout"), Value::Int(i64::from(fanout)));
    obj.insert(String::from("from_peer"), Value::Str(peer_hex(from_peer)));
    Value::Object(obj)
}

/// `msg.queued` — `docs/powl/08` §4. `msg_log_id`/`conv_hash` doivent déjà
/// être redactés (voir [`msg_log_id`] ; `conv_hash` suit le même schéma,
/// `SHA-256(min(peerA,peerB) ‖ max(peerA,peerB))[0..8]`, calculé par la
/// couche appelante — hors périmètre de ce module).
pub fn msg_queued(msg_log_id: MsgLogId, conv_hash: [u8; 8]) -> Value {
    let mut obj = BTreeMap::new();
    obj.insert(String::from("msg_log_id"), Value::Str(hex(&msg_log_id)));
    obj.insert(String::from("conv_hash"), Value::Str(hex(&conv_hash)));
    Value::Object(obj)
}

/// `central` ou `peripheral` — `docs/powl/08` §5, champ `role` de
/// `peer.connected`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Central,
    Peripheral,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::Central => "central",
            Role::Peripheral => "peripheral",
        }
    }
}

/// `peer.connected` — `docs/powl/08` §5.
pub fn peer_connected(peer: PeerId, rssi: i8, role: Role) -> Value {
    let mut obj = BTreeMap::new();
    obj.insert(String::from("peer"), Value::Str(peer_hex(peer)));
    obj.insert(String::from("rssi"), Value::Int(i64::from(rssi)));
    obj.insert(
        String::from("role"),
        Value::Str(String::from(role.as_str())),
    );
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_decode(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex valide dans un test"))
            .collect()
    }

    fn peer(hex_str: &str) -> PeerId {
        let bytes = hex_decode(hex_str);
        let mut out = [0u8; 8];
        out.copy_from_slice(&bytes);
        out
    }

    fn eight(hex_str: &str) -> [u8; 8] {
        peer(hex_str)
    }

    /// Fixture golden `contracts/events/fixtures/01-pkt-seen.json` (US-107),
    /// événement unique de son batch. Octets attendus calculés directement
    /// avec `contracts/tools/catalogue.py::canonical_json` sur les mêmes
    /// champs — comparaison octet à octet, pas juste "mêmes valeurs après
    /// re-parsing" (critère d'acceptation US-208).
    #[test]
    fn pkt_seen_produit_les_memes_octets_que_la_fixture_golden_01() {
        let payload = pkt_seen(
            eight("4d5e6f7a8b9c0d1e"),
            3,
            7,
            512,
            peer("a1b2c3d4e5f60718"),
            Some(-63),
        );
        let env = Envelope::new(
            "relay-3f2a9c",
            NodeKind::Relay,
            1001,
            1_725_800_000_010,
            "pkt.seen",
            payload,
        );
        assert_eq!(
            env.event_id,
            "e8662efe39b50ec87a5c81d5f8eb136f9f4ac88b7364ad269ccd6b0119cedb5d"
        );
        let expected = b"{\"event_id\":\"e8662efe39b50ec87a5c81d5f8eb136f9f4ac88b7364ad269ccd6b0119cedb5d\",\"name\":\"pkt.seen\",\"node_id\":\"relay-3f2a9c\",\"node_kind\":\"relay\",\"payload\":{\"from_peer\":\"a1b2c3d4e5f60718\",\"msg_log_id\":\"4d5e6f7a8b9c0d1e\",\"rssi\":-63,\"size_bucket\":512,\"ttl_in\":7,\"type\":3},\"seq\":1001,\"ts_ms\":1725800000010}";
        assert_eq!(env.to_canonical_bytes(), expected);
    }

    /// Fixture golden `contracts/events/fixtures/10-msg-queued.json`.
    #[test]
    fn msg_queued_produit_les_memes_octets_que_la_fixture_golden_10() {
        let payload = msg_queued(eight("1122334455667788"), eight("0011223344556677"));
        let env = Envelope::new(
            "client-9c1d84",
            NodeKind::Client,
            5002,
            1_725_800_000_110,
            "msg.queued",
            payload,
        );
        assert_eq!(
            env.event_id,
            "80dc07068c5686c310f034b5351f5ea875995e2dffc61712cdc6eaa11f602b2c"
        );
        let expected = b"{\"event_id\":\"80dc07068c5686c310f034b5351f5ea875995e2dffc61712cdc6eaa11f602b2c\",\"name\":\"msg.queued\",\"node_id\":\"client-9c1d84\",\"node_kind\":\"client\",\"payload\":{\"conv_hash\":\"0011223344556677\",\"msg_log_id\":\"1122334455667788\"},\"seq\":5002,\"ts_ms\":1725800000110}";
        assert_eq!(env.to_canonical_bytes(), expected);
    }

    /// Fixture golden `contracts/events/fixtures/16-peer-connected-disconnected.json`
    /// (premier événement du batch, `peer.connected`).
    #[test]
    fn peer_connected_produit_les_memes_octets_que_la_fixture_golden_16() {
        let payload = peer_connected(peer("a1b2c3d4e5f60718"), -58, Role::Peripheral);
        let env = Envelope::new(
            "relay-3f2a9c",
            NodeKind::Relay,
            1012,
            1_725_800_001_100,
            "peer.connected",
            payload,
        );
        assert_eq!(
            env.event_id,
            "ce470fe9aba1e205e473e011cd1fe2116626421d8fdde934f3b70eb4db21a25e"
        );
        let expected = b"{\"event_id\":\"ce470fe9aba1e205e473e011cd1fe2116626421d8fdde934f3b70eb4db21a25e\",\"name\":\"peer.connected\",\"node_id\":\"relay-3f2a9c\",\"node_kind\":\"relay\",\"payload\":{\"peer\":\"a1b2c3d4e5f60718\",\"role\":\"peripheral\",\"rssi\":-58},\"seq\":1012,\"ts_ms\":1725800001100}";
        assert_eq!(env.to_canonical_bytes(), expected);
    }

    #[test]
    fn un_msg_uuid_brut_ne_peut_jamais_apparaitre_dans_un_evenement() {
        // Critère d'acceptation US-208 : « aucun msg_uuid, texte de
        // message ou identifiant de destinataire ne peut sortir, quelle
        // que soit l'entrée ». `msg_log_id` est le SEUL point d'entrée
        // pour un identifiant de message dans ce module — ce test prouve
        // que la sortie ne contient jamais l'identifiant brut qui y est
        // entré, sur plusieurs événements distincts.
        let msg_uuid: &[u8] = b"un-identifiant-de-message-tres-identifiable-32-octets!!";
        let log_id = msg_log_id(msg_uuid);

        let envelopes = [
            Envelope::new(
                "relay-3f2a9c",
                NodeKind::Relay,
                1,
                1_000,
                "pkt.seen",
                pkt_seen(log_id, 3, 7, 512, [0xAA; 8], None),
            ),
            Envelope::new(
                "relay-3f2a9c",
                NodeKind::Relay,
                2,
                2_000,
                "pkt.relayed",
                pkt_relayed(log_id, 3, 7, 6, 2, [0xAA; 8]),
            ),
            Envelope::new(
                "client-9c1d84",
                NodeKind::Client,
                3,
                3_000,
                "msg.queued",
                msg_queued(log_id, [0xBB; 8]),
            ),
        ];

        for env in &envelopes {
            let bytes = env.to_canonical_bytes();
            // Ni les octets bruts de msg_uuid...
            assert!(
                !contains(&bytes, msg_uuid),
                "msg_uuid brut trouvé dans l'événement {}",
                env.name
            );
            // ...ni sa représentation hex (au cas où une future
            // modification l'encoderait en hex plutôt que de le copier
            // tel quel).
            let msg_uuid_hex = hex(msg_uuid);
            assert!(
                !bytes_contains_str(&bytes, &msg_uuid_hex),
                "hex(msg_uuid) trouvé dans l'événement {}",
                env.name
            );
            // Le msg_log_id redacté, lui, DOIT apparaître — sinon le test
            // ne prouverait rien (un événement vide passerait aussi).
            assert!(
                bytes_contains_str(&bytes, &hex(&log_id)),
                "msg_log_id absent de l'événement {} — le test serait vide de sens",
                env.name
            );
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len().max(1)).any(|w| w == needle)
    }

    fn bytes_contains_str(haystack: &[u8], needle: &str) -> bool {
        contains(haystack, needle.as_bytes())
    }
}
