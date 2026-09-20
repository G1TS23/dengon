//! `dengon-ffi` — surface FFI de dengon exposée via UniFFI.
//!
//! Génère les bindings Kotlin consommés par l'application Android ; Swift est
//! reporté en v2 (`docs/synthese/04-architecture.md` §5). C'est le pont entre
//! le cœur Rust et `android/.../ble/AndroidTransport.kt`.
//!
//! Contrat v0 (US-106) : fichier `src/dengon.udl`, couvrant `send_message`,
//! `poll_events`, `on_peer_connected`, identité + QR. L'implémentation
//! ci-dessous est un **bouchon en mémoire** — canned, pas branché sur
//! `dengon-core` (aucun module réel n'existe encore côté `identity`/`sync`,
//! voir US-108/US-205/US-301) — mais elle passe par le **vrai** scaffolding
//! UniFFI généré depuis le `.udl` : c'est ce qui prouve que le contrat est
//! valide, pas juste une intention en Markdown. `AndroidTransport` (US-213)
//! et le vrai FFI (US-302) remplaceront ce bouchon sans changer la forme du
//! contrat — toute évolution de `dengon.udl` après le gel passe par une
//! réunion d'équipe.
//!
//! # Note sur `unsafe`
//!
//! UniFFI génère du code `unsafe`. Le lint `unsafe_code = "deny"` défini dans
//! `[workspace.lints.rust]` est donc neutralisé ici par
//! `#![allow(unsafe_code)]`. C'est précisément pour cela qu'il est en `deny`
//! et non en `forbid`, qui serait inviolable.
//!
//! # État
//!
//! Squelette (`version()`) livré par l'US-104. Contrat FFI v0 + bouchon
//! livrés par l'US-106.

#![allow(unsafe_code)]

use std::sync::{Mutex, MutexGuard, PoisonError};

uniffi::include_scaffolding!("dengon");

/// Version de la surface FFI, destinée à être exposée aux plateformes hôtes.
pub fn version() -> String {
    format!(
        "{} (protocole v{})",
        env!("CARGO_PKG_VERSION"),
        dengon_core::PROTOCOL_VERSION
    )
}

// ---------------------------------------------------------------------------
// Types du contrat (miroirs Rust de `dengon.udl`).
// ---------------------------------------------------------------------------

/// Identité d'un nœud : `peer_id` + pseudo + clés publiques.
///
/// **Placeholder cryptographique** : `pub_static`/`pub_sign` ne sont pas de
/// vraies clés X25519/Ed25519 ici — `identity`/`crypto` n'existent pas encore
/// côté `dengon-core` (US-108/US-205). Le contrat v0 fige la FORME du type
/// FFI, pas son contenu cryptographique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub peer_id: String,
    pub pseudo: String,
    pub pub_static: Vec<u8>,
    pub pub_sign: Vec<u8>,
}

/// Statuts MVP du cycle de vie d'un message
/// (`docs/synthese/07-cycle-de-vie-et-statuts.md` §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStatus {
    Queued,
    InFlight,
    Delivered,
    Read,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub msg_uuid: String,
    pub conv_id: String,
    pub author_peer_id: String,
    pub body: String,
    pub outgoing: bool,
    pub sent_ms: u64,
    pub status: MessageStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub conv_id: String,
    pub peer_id: String,
    pub peer_pseudo: String,
    pub last_message: Option<Message>,
    pub unread_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeEvent {
    MessageReceived { message: Message },
    StatusChanged { msg_uuid: String, status: MessageStatus },
    PeerConnected { peer_id: String },
    PeerDisconnected { peer_id: String },
}

/// Erreurs de la surface FFI. `Internal` couvre le v0 (contenu peu détaillé
/// pour l'instant) ; `UnknownPeer`/`NotConnected` sont réservées pour la vraie
/// logique de routage (US-301).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DengonError {
    UnknownPeer,
    NotConnected,
    Internal,
}

impl std::fmt::Display for DengonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DengonError::UnknownPeer => write!(f, "pair inconnu"),
            DengonError::NotConnected => write!(f, "aucun pair connecté"),
            DengonError::Internal => write!(f, "erreur interne"),
        }
    }
}

impl std::error::Error for DengonError {}

// ---------------------------------------------------------------------------
// `DengonNode` — bouchon en mémoire (pas de persistance, pas de radio).
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct NodeState {
    messages: Vec<Message>,
    conversations: Vec<Conversation>,
    pending_events: Vec<NodeEvent>,
    connected_peers: Vec<String>,
}

#[derive(Debug)]
pub struct DengonNode {
    identity: Identity,
    state: Mutex<NodeState>,
}

impl DengonNode {
    pub fn new(identity: Identity) -> Self {
        Self {
            identity,
            state: Mutex::new(NodeState::default()),
        }
    }

    /// Toujours `Ok` dans ce bouchon : la vraie résolution de contact / le
    /// vrai store-and-forward arrivent avec `sync`/`store` (US-301). Le type
    /// `Result` reste dans le contrat pour ne pas casser l'appelant quand ces
    /// erreurs deviendront réelles.
    pub fn send_message(&self, dest_peer_id: String, body: String) -> Result<String, DengonError> {
        let mut state = self.lock_state();

        let msg_uuid = format!("msg-{}", state.messages.len());
        let conv_id = format!("conv-{dest_peer_id}");
        let status = if state.connected_peers.iter().any(|p| p == &dest_peer_id) {
            MessageStatus::InFlight
        } else {
            MessageStatus::Queued
        };

        let message = Message {
            msg_uuid: msg_uuid.clone(),
            conv_id: conv_id.clone(),
            author_peer_id: self.identity.peer_id.clone(),
            body,
            outgoing: true,
            sent_ms: 0,
            status,
        };
        state.messages.push(message.clone());

        match state.conversations.iter_mut().find(|conv| conv.conv_id == conv_id) {
            Some(conv) => conv.last_message = Some(message),
            None => state.conversations.push(Conversation {
                conv_id,
                peer_pseudo: dest_peer_id.clone(),
                peer_id: dest_peer_id,
                last_message: Some(message),
                unread_count: 0,
            }),
        }

        Ok(msg_uuid)
    }

    pub fn poll_events(&self) -> Vec<NodeEvent> {
        self.lock_state().pending_events.drain(..).collect()
    }

    pub fn on_peer_connected(&self, peer_id: String) {
        let mut state = self.lock_state();
        if !state.connected_peers.iter().any(|p| p == &peer_id) {
            state.connected_peers.push(peer_id.clone());
            state.pending_events.push(NodeEvent::PeerConnected { peer_id });
        }
    }

    pub fn list_conversations(&self) -> Vec<Conversation> {
        self.lock_state().conversations.clone()
    }

    pub fn list_messages(&self, conv_id: String) -> Vec<Message> {
        self.lock_state()
            .messages
            .iter()
            .filter(|message| message.conv_id == conv_id)
            .cloned()
            .collect()
    }

    /// Un mutex empoisonné (panique pendant qu'il était tenu) ne doit pas
    /// faire perdre les données déjà écrites : on récupère le contenu plutôt
    /// que de propager la panique à chaque appel suivant.
    fn lock_state(&self) -> MutexGuard<'_, NodeState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

// ---------------------------------------------------------------------------
// Identité + QR (fonctions libres du namespace `dengon`).
// ---------------------------------------------------------------------------

/// **Placeholder cryptographique** (voir doc de [`Identity`]) : dérive des
/// octets déterministes à partir du pseudo, pas de vraie génération de clés.
pub fn generate_identity(pseudo: String) -> Identity {
    let mut pub_static = vec![0_u8; 32];
    let mut pub_sign = vec![0_u8; 32];
    for (i, byte) in pseudo.bytes().enumerate() {
        pub_static[i % 32] ^= byte;
        pub_sign[i % 32] ^= byte.wrapping_add(1);
    }
    let peer_id = to_hex(&pub_static[..8]);
    Identity {
        peer_id,
        pseudo,
        pub_static,
        pub_sign,
    }
}

/// Format `docs/synthese/06-securite.md` §identité :
/// `dengon:v1:<base64url(pseudo_len:u8 ‖ pseudo ‖ pub_static:32 ‖ pub_sign:32)>`.
pub fn identity_qr_code(identity: Identity) -> String {
    let pseudo_bytes = identity.pseudo.as_bytes();
    // Clampé à 255 o : longueur encodée sur un seul octet. Un pseudo aussi
    // long n'a pas de sens produit ; la vraie validation viendra avec
    // `identity` (US-205).
    let pseudo_len = pseudo_bytes.len().min(255);

    let capacity = 1 + pseudo_len + identity.pub_static.len() + identity.pub_sign.len();
    let mut payload = Vec::with_capacity(capacity);
    #[allow(clippy::cast_possible_truncation)] // borné par .min(255) juste au-dessus
    payload.push(pseudo_len as u8);
    payload.extend_from_slice(&pseudo_bytes[..pseudo_len]);
    payload.extend_from_slice(&identity.pub_static);
    payload.extend_from_slice(&identity.pub_sign);

    format!("dengon:v1:{}", encode_base64url(&payload))
}

pub fn identity_from_qr_code(qr_code: String) -> Result<Identity, DengonError> {
    let encoded = qr_code.strip_prefix("dengon:v1:").ok_or(DengonError::Internal)?;
    let payload = decode_base64url(encoded)?;

    let pseudo_len = usize::from(*payload.first().ok_or(DengonError::Internal)?);
    let mut offset = 1;

    let pseudo_bytes = payload.get(offset..offset + pseudo_len).ok_or(DengonError::Internal)?;
    offset += pseudo_len;
    let pub_static = payload.get(offset..offset + 32).ok_or(DengonError::Internal)?.to_vec();
    offset += 32;
    let pub_sign = payload.get(offset..offset + 32).ok_or(DengonError::Internal)?.to_vec();

    let pseudo = String::from_utf8(pseudo_bytes.to_vec())
        .map_err(|_utf8_err| DengonError::Internal)?;
    let peer_id = to_hex(&pub_static[..8]);

    Ok(Identity {
        peer_id,
        pseudo,
        pub_static,
        pub_sign,
    })
}

/// Code de vérification 60 chiffres, ordre-indépendant.
///
/// **Placeholder** : `docs/synthese/06-securite.md` demande
/// `SHA-512(min(fpA,fpB) ‖ max(fpA,fpB))`, indisponible ici (pas de `crypto`
/// avant US-108/US-203). Remplacé par un mélange FNV-1a — même FORME (12
/// groupes de 5 chiffres, même code des deux côtés), contenu non
/// cryptographique. Ne pas comparer à un futur calcul basé sur SHA-512 : ce
/// n'est pas le même algorithme, seulement le même contrat de sortie.
pub fn verification_code(local: Identity, remote: Identity) -> String {
    let fp_local = fingerprint(&local);
    let fp_remote = fingerprint(&remote);
    let (fp_a, fp_b) = if fp_local <= fp_remote {
        (fp_local, fp_remote)
    } else {
        (fp_remote, fp_local)
    };

    (0..12)
        .map(|i| format!("{:05}", placeholder_material(&fp_a, &fp_b, i)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn fingerprint(identity: &Identity) -> Vec<u8> {
    let mut fp = identity.pub_static.clone();
    fp.extend_from_slice(&identity.pub_sign);
    fp
}

fn placeholder_material(fp_a: &[u8], fp_b: &[u8], group_index: u64) -> u32 {
    // FNV-1a, salé par l'index de groupe pour produire 12 valeurs
    // décorrélées à partir des deux mêmes empreintes.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325 ^ group_index;
    for byte in fp_a.iter().chain(fp_b.iter()) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    #[allow(clippy::cast_possible_truncation)] // % 100_000 tient sur 32 bits
    let groupe = (hash % 100_000) as u32;
    groupe
}

// ---------------------------------------------------------------------------
// base64url sans padding (RFC 4648 §5) — pas de dépendance externe pour ça
// seul : `dengon-ffi` n'a qu'UniFFI comme dépendance non-interne.
// ---------------------------------------------------------------------------

const BASE64URL_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn encode_base64url(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied();
        let b2 = chunk.get(2).copied();

        let c0 = b0 >> 2;
        let c1 = ((b0 & 0b0000_0011) << 4) | (b1.unwrap_or(0) >> 4);
        out.push(BASE64URL_ALPHABET[c0 as usize] as char);
        out.push(BASE64URL_ALPHABET[c1 as usize] as char);

        if let Some(b1) = b1 {
            let c2 = ((b1 & 0b0000_1111) << 2) | (b2.unwrap_or(0) >> 6);
            out.push(BASE64URL_ALPHABET[c2 as usize] as char);
        }
        if let Some(b2) = b2 {
            let c3 = b2 & 0b0011_1111;
            out.push(BASE64URL_ALPHABET[c3 as usize] as char);
        }
    }
    out
}

fn decode_base64url(input: &str) -> Result<Vec<u8>, DengonError> {
    fn sixbits(byte: u8) -> Result<u8, DengonError> {
        match byte {
            b'A'..=b'Z' => Ok(byte - b'A'),
            b'a'..=b'z' => Ok(byte - b'a' + 26),
            b'0'..=b'9' => Ok(byte - b'0' + 52),
            b'-' => Ok(62),
            b'_' => Ok(63),
            _ => Err(DengonError::Internal),
        }
    }

    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3 + 3);

    for chunk in bytes.chunks(4) {
        let v0 = sixbits(chunk[0])?;
        let v1 = sixbits(*chunk.get(1).ok_or(DengonError::Internal)?)?;
        out.push((v0 << 2) | (v1 >> 4));

        let Some(&raw2) = chunk.get(2) else { continue };
        let v2 = sixbits(raw2)?;
        out.push((v1 << 4) | (v2 >> 2));

        let Some(&raw3) = chunk.get(3) else { continue };
        let v3 = sixbits(raw3)?;
        out.push((v2 << 6) | v3);
    }

    Ok(out)
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_version_expose_le_numero_de_protocole() {
        assert!(super::version().contains("protocole v1"));
    }

    #[test]
    fn base64url_fait_un_aller_retour() -> Result<(), DengonError> {
        let donnees: &[u8] = b"dengon hello mesh 0123456789 !!";
        let encode = encode_base64url(donnees);
        let decode = decode_base64url(&encode)?;
        assert_eq!(decode, donnees);
        Ok(())
    }

    #[test]
    fn qr_code_fait_un_aller_retour() -> Result<(), DengonError> {
        let identite = generate_identity("alice".to_owned());
        let qr = identity_qr_code(identite.clone());
        assert!(qr.starts_with("dengon:v1:"));

        let decodee = identity_from_qr_code(qr)?;
        assert_eq!(decodee, identite);
        Ok(())
    }

    #[test]
    fn le_code_de_verification_est_le_meme_dans_les_deux_sens() {
        let alice = generate_identity("alice".to_owned());
        let bob = generate_identity("bob".to_owned());

        let cote_alice = verification_code(alice.clone(), bob.clone());
        let cote_bob = verification_code(bob, alice);

        assert_eq!(cote_alice, cote_bob);
        assert_eq!(cote_alice.split(' ').count(), 12);
    }

    #[test]
    fn envoyer_un_message_cree_une_conversation_et_signale_le_pair() -> Result<(), DengonError> {
        let identite = generate_identity("alice".to_owned());
        let node = DengonNode::new(identite);

        node.on_peer_connected("bob".to_owned());
        let msg_uuid = node.send_message("bob".to_owned(), "salut".to_owned())?;

        let evenements = node.poll_events();
        let a_vu_bob_connecte = evenements.iter().any(|event| {
            matches!(event, NodeEvent::PeerConnected { peer_id } if peer_id == "bob")
        });
        assert!(a_vu_bob_connecte);

        let messages = node.list_messages("conv-bob".to_owned());
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].msg_uuid, msg_uuid);
        assert_eq!(messages[0].status, MessageStatus::InFlight);

        assert_eq!(node.list_conversations().len(), 1);
        Ok(())
    }
}
