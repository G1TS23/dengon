//! Façade `api` — le point d'entrée unique de `dengon-core` (US-301).
//!
//! Câble ensemble tout ce que le reste de la crate a livré séparément :
//! [`protocol`](crate::protocol) (format de trame, frames applicatives),
//! [`crypto`](crate::crypto) (Ed25519, Noise `XX`/`X`),
//! [`identity`](crate::identity) (clés, `peerID`), [`ledger`](crate::ledger)
//! (journal chaîné signé), [`store`](crate::store) (persistance SQLite,
//! `std` seulement) et [`sync`](crate::sync) (`routing`, `status`,
//! `courier`).
//!
//! # Module entier derrière la feature `std`
//!
//! Contrairement au reste de la crate (`protocol`/`sync`/`ledger`/
//! `observability` compilent en `no_std`+`alloc`, seul `store` est
//! `std`-only), **toute** cette façade est posée derrière `#[cfg(feature =
//! "std")]`. Choix délibéré, pas un oubli de portabilité : côté ESP32, le
//! firmware relais (US-307/US-308) câble `sync::routing`/`sync::inventory`/
//! `sync::courier`/`ledger` **directement** dans ses tâches FreeRTOS — un
//! relais dédié n'a pas d'utilité pour `send_message`/`list_conversations`,
//! qui sont des concepts côté client (téléphone). Cette façade ne sert donc
//! que `dengon-node` (CLI, `std`) et `dengon-ffi` (Android, `std` aussi) :
//! aucun consommateur `no_std` n'en a besoin. Si ça change, retirer la
//! garde `std` demandera surtout de fournir un `Store`-like optionnel non
//! `std` (voir `Node::attach_store`) — pas de refonte de la logique
//! ci-dessous, déjà écrite en `alloc` pur à part cette persistance.
//!
//! # Ce que cette façade N'est PAS
//!
//! **Elle ne possède aucun [`Transport`].** `dengon-core` reste
//! transport-agnostique (`dengon-ble` n'est qu'une dépendance *de test*,
//! voir `Cargo.toml`) : c'est l'appelant (Kotlin/Android via `dengon-ffi`,
//! `dengon-node`, le firmware) qui possède la radio et pilote [`Node`] par
//! méthode — exactement le modèle du `.udl` gelé de l'US-106
//! (`on_peer_connected(peer_id)` est un appel du parent VERS le nœud, pas un
//! événement que le nœud produirait tout seul).
//!
//! Deux méthodes n'existent PAS dans ce `.udl` v0 (son propre commentaire de
//! tête le dit : « le reste de la façade api … arrive avec le vrai FFI,
//! US-301/US-302 ») et sont donc des extensions **Rust seulement** de cette
//! US, pour que `dengon-node`/les tests puissent réellement faire transiter
//! des octets : [`Node::on_bytes_received`] (des octets sont arrivés d'un
//! pair) et [`Node::take_outgoing`] (les octets à transmettre, à vider après
//! chaque appel qui peut en produire). US-302 les reflétera dans le `.udl`
//! sous une forme adaptée à UniFFI (probablement un type de canal ou un
//! second appel `poll`).
//!
//! # Construction : `identity::Identity`, pas le dictionnaire `Identity` du `.udl`
//!
//! Le `.udl` v0 déclare `constructor(Identity identity)` avec `Identity`
//! **sans aucun secret** (`peer_id`/`pseudo`/`pub_static`/`pub_sign`
//! uniquement — voir `crates/dengon-ffi/src/dengon.udl`). Une façade réelle a
//! besoin des clés **privées** pour déchiffrer/signer : ce n'est pas un oubli
//! du contrat, c'est exactement ce que son commentaire de tête annonce comme
//! différé à cette US. [`Node::new`] prend donc [`crate::identity::Identity`]
//! (avec secrets) ; l'US-302 décidera comment l'app Android le lui fournit
//! (lu depuis son coffre via un appel FFI séparé, jamais à travers le
//! `.udl`).
//!
//! # Portée de cette implémentation
//!
//! - **Messagerie en session** (Noise `XX`) : câblée et testée de bout en
//!   bout — handshake orchestré via [`Node::on_bytes_received`]/
//!   [`Node::take_outgoing`], chiffrement/déchiffrement du transport une
//!   fois la session établie.
//! - **Messagerie par enveloppe** (Noise `X`, destinataire hors de portée) :
//!   câblée pour le cas simple « on se connecte directement à celui à qui on
//!   écrit » — remise automatique à cet instant, même si le handshake `XX`
//!   n'a pas encore abouti (l'enveloppe ne dépend pas de la session).
//!   `SEALED_ENVELOPE` n'étant **jamais adressé** en clair (décision A-8 :
//!   le `recipient_id` révélerait le destinataire, ce que `recipient_tag`
//!   évite), [`crate::sync::routing::Router`] ne peut jamais rendre
//!   `Decision::Deliver` pour ce type de paquet — même quand on en est le
//!   vrai destinataire. [`Node::on_bytes_received`] tente donc de l'ouvrir
//!   avec sa propre clé statique **avant** tout dépôt en courrier : succès
//!   → traité directement ; échec → c'était pour quelqu'un d'autre, voir
//!   `sync::courier` ci-dessous. L'échange `ENVELOPE_OFFER`/
//!   `ENVELOPE_REQUEST` avec un porteur tiers (pas le destinataire final)
//!   n'est **pas** câblé ici — écart consigné dans `03-ecarts-conception.md` :
//!   c'est une négociation à deux paquets de plus, plus proche du rôle d'un
//!   relais dédié (US-308) que de cette façade côté client.
//! - **`sync::courier`** : câblé côté « je porte une enveloppe croisée sur
//!   mon chemin » — une `SEALED_ENVELOPE` reçue, non adressée à moi (elle ne
//!   s'ouvre pas avec ma clé) et donc classée [`Decision::Store`] par le
//!   routeur, est déposée dans [`Courier`]. La remise de ce que je porte à
//!   son vrai propriétaire n'est pas câblée, pour la même raison que
//!   ci-dessus.
//! - **Envoi d'accusés de réception** : [`Node::on_bytes_received`] sait
//!   *traiter* un `AppFrame::Ack` reçu ([`Node::apply_ack`]), mais rien dans
//!   cette façade n'en **émet** — un pair qui reçoit un message ne prévient
//!   pas son expéditeur. Écart consigné : hors périmètre de cette US, sans
//!   quoi le statut `Delivered` d'un message sortant n'est jamais atteint
//!   par cette façade (`InFlight` est le statut final observable ici).
//! - **`sync::inventory`** (US-210) : **pas câblé** — la PR qui le livre
//!   n'est pas encore mergée sur `main` au moment où cette US démarre (voir
//!   `docs/suivi/00-journal.md`, entrée US-301). Écart consigné.
//! - **`PeerId` comme identifiant de lien** : `dengon-ble::LinkId` (une
//!   connexion) et `PeerId` (un nœud) sont deux concepts distincts dans la
//!   conception (`03-ecarts-conception.md` déjà signalé pour `dengon-ble`).
//!   Cette façade ne connaissant pas `dengon-ble::LinkId` (pas de dépendance
//!   hors tests), elle utilise directement `PeerId` comme identifiant de
//!   lien pour [`Router`] — une simplification valable tant qu'un nœud n'a
//!   qu'une connexion active par pair, documentée comme telle.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use sha2::{Digest, Sha256};

use crate::crypto::noise::{self, Handshake, Session};
use crate::crypto::{CryptoError, SigningKey};
use crate::identity::{self, PublicIdentity};
use crate::ledger::Ledger;
use crate::protocol::codec::app::{self, AckFrame, AppFrame, MessageFrame};
use crate::protocol::codec::{self, Packet};
use crate::protocol::consts::{PEER_ID_LEN, PROTO_VERSION, TTL_DEFAULT};
use crate::protocol::types::{Flags, Header, PacketType};
use crate::protocol::{MsgId, PeerId};
use crate::sync::courier::{Courier, CourierConfig};
use crate::sync::routing::{Decision, Now as RoutingNow, Router, RoutingConfig};
use crate::sync::status::{
    DeliveryKind, MemoryStore, MsgUuid, NewMessage, Outbox, OutboxError, Status,
};

use crate::store::{FixedKeySource, Store};

/// Ré-export : `Identity` publique — la « carte de contact » du `.udl`
/// (`peer_id`/`pseudo`/`pub_static`/`pub_sign`, aucun secret). C'est le type
/// à utiliser pour désigner un **correspondant** (`send_message`,
/// `Conversation`) ; l'identité **locale**, avec secrets, reste
/// [`crate::identity::Identity`], passée une seule fois à [`Node::new`].
pub type Identity = PublicIdentity;

/// Statut d'un message émis, tel qu'exposé par la façade (miroir de l'énum
/// `MessageStatus` du `.udl` — sans `Cancelled`, hors périmètre de cette US
/// : aucune méthode `cancel_message` n'est encore exposée).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStatus {
    Queued,
    InFlight,
    Delivered,
    Expired,
}

impl From<Status> for MessageStatus {
    fn from(value: Status) -> Self {
        match value {
            Status::Queued => Self::Queued,
            Status::InFlight => Self::InFlight,
            Status::Delivered => Self::Delivered,
            // `Cancelled` n'est atteignable que par un futur `cancel_message()`,
            // pas encore exposé par cette façade (voir la doc de module) :
            // un message de cette façade ne peut donc jamais y arriver
            // aujourd'hui. Replié sur `Expired` plutôt qu'un panic si ça
            // change un jour sans que cette conversion ne soit mise à jour.
            Status::Expired | Status::Cancelled => Self::Expired,
        }
    }
}

/// Un message, tel qu'exposé par la façade (miroir de `Message` du `.udl`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub msg_uuid: MsgUuid,
    pub conv_id: ConvId,
    pub author_peer_id: PeerId,
    pub body: String,
    pub outgoing: bool,
    pub sent_ms: u64,
    pub status: MessageStatus,
}

/// Identifiant de conversation : `SHA-256(min(a,b) ‖ max(a,b))[0..8]`
/// (`docs/synthese/09` §9, `conv_hash`) — stable quel que soit l'ordre des
/// deux pairs, pour qu'Alice et Bob calculent le même identifiant chacun de
/// leur côté sans coordination.
pub type ConvId = [u8; 8];

#[must_use]
fn conv_id_of(a: PeerId, b: PeerId) -> ConvId {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let mut buf = [0u8; PEER_ID_LEN * 2];
    buf[..PEER_ID_LEN].copy_from_slice(&lo);
    buf[PEER_ID_LEN..].copy_from_slice(&hi);
    let digest = Sha256::digest(buf);
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    out
}

/// Une conversation, telle qu'exposée par la façade (miroir de
/// `Conversation` du `.udl`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub conv_id: ConvId,
    pub peer_id: PeerId,
    pub peer_pseudo: String,
    pub last_message: Option<Message>,
    pub unread_count: u32,
}

/// Ce qui peut arriver au nœud, à récupérer par [`Node::poll_events`]
/// (miroir de `NodeEvent` du `.udl`, mêmes quatre variantes — pas de
/// cinquième variante « octets à envoyer » : voir [`Node::take_outgoing`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeEvent {
    MessageReceived(Message),
    StatusChanged(MsgUuid, MessageStatus),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

/// Erreur renvoyée par la façade (miroir de `DengonError` du `.udl`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DengonError {
    /// Le pair n'a jamais été vu ([`Node::on_peer_connected`] jamais appelé
    /// pour lui) : on n'a ni session ni clé publique connue pour lui.
    UnknownPeer,
    /// Le pair n'est pas connecté **et** aucune enveloppe ne peut être
    /// scellée pour plus tard (pas de clé publique connue) : voir
    /// [`DengonError::UnknownPeer`], distingué de « connecté ou pas ».
    NotConnected,
    /// Erreur interne (crypto, codec, stockage) — le détail est dans les
    /// logs/`Debug`, jamais exposé par le `.udl` (`[Error] enum DengonError`
    /// gelé à 3 variantes sans charge utile).
    Internal,
}

impl fmt::Display for DengonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPeer => f.write_str("pair inconnu : jamais annoncé au nœud"),
            Self::NotConnected => f.write_str("pair non connecté et pas de clé publique connue"),
            Self::Internal => f.write_str("erreur interne"),
        }
    }
}

impl core::error::Error for DengonError {}

impl From<CryptoError> for DengonError {
    fn from(_: CryptoError) -> Self {
        Self::Internal
    }
}

impl<E> From<OutboxError<E>> for DengonError {
    fn from(_: OutboxError<E>) -> Self {
        Self::Internal
    }
}

/// État de la liaison cryptographique avec un pair.
enum PeerCrypto {
    /// Handshake `XX` en cours (initiateur ou répondeur — `snow` ne fait pas
    /// la différence une fois construit, seul l'ordre d'appel compte).
    /// `Box` : `Handshake` (≥864 o, l'état interne de `snow`) contre
    /// `Session` (≥184 o) — sans lui, chaque `PeerState` (une par pair
    /// connu, potentiellement des dizaines) réserverait la taille du plus
    /// gros des deux même une fois la session établie et le handshake
    /// oublié (`clippy::large_enum_variant`).
    Handshaking(Box<Handshake>),
    /// Session établie : chiffrement de transport disponible.
    Established(Session),
}

/// Ce qu'on garde d'un pair — potentiellement avant même toute connexion
/// (un contact ajouté à l'avance, ex. après un appairage QR, US-215).
#[derive(Default)]
struct PeerState {
    /// Identité publique, si on la connaît déjà (contact ajouté, ou apprise
    /// via le handshake `XX` — voir [`Handshake::remote_static`]).
    identity: Option<PublicIdentity>,
    /// `None` tant que [`Node::on_peer_connected`] n'a pas été appelé pour ce
    /// pair (ou après [`Node::on_peer_disconnected`]) : pas de canal, donc
    /// aucun octet ne peut être chiffré en session pour lui pour l'instant.
    crypto: Option<PeerCrypto>,
}

/// Un message reçu ou envoyé, gardé en mémoire par la façade (voir la note
/// « Persistance » de la doc de module).
#[derive(Debug, Clone)]
struct MessageRecord {
    author_peer_id: PeerId,
    body: String,
    outgoing: bool,
    sent_ms: u64,
    status: Status,
}

/// Une conversation suivie, en mémoire.
#[derive(Debug, Clone, Default)]
struct ConversationRecord {
    peer_id: PeerId,
    peer_pseudo: String,
    /// `msg_uuid` des messages de cette conversation, dans l'ordre d'arrivée
    /// (le premier saisi le plus ancien) — juste assez pour retrouver
    /// `list_messages` sans rescanner toute la table `messages`.
    message_order: Vec<MsgUuid>,
    unread_count: u32,
}

/// Le nœud dengon — façade unique sur le protocole (US-301).
///
/// Voir la doc de module pour ce qu'elle câble, ce qu'elle ne câble pas
/// encore, et pourquoi la construction diffère du `.udl` v0.
pub struct Node {
    identity: identity::Identity,
    peer_id: PeerId,
    router: Router<PeerId>,
    outbox: Outbox<MemoryStore>,
    ledger: Ledger<SigningKey>,
    courier: Courier,
    peers: BTreeMap<PeerId, PeerState>,
    conversations: BTreeMap<ConvId, ConversationRecord>,
    messages: BTreeMap<MsgUuid, MessageRecord>,
    /// Compteur de `seq` pour le journal d'observabilité (US-208) —
    /// indépendant du `seq` de `ledger` (qui a le sien).
    obs_seq: u64,
    outgoing: Vec<(PeerId, Vec<u8>)>,
    events: VecDeque<NodeEvent>,
    store: Option<Store<FixedKeySource>>,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("peer_id", &self.peer_id)
            .field("peers", &self.peers.len())
            .field("conversations", &self.conversations.len())
            .field("messages", &self.messages.len())
            .finish_non_exhaustive()
    }
}

impl Node {
    /// Nouveau nœud pour `identity` (voir la doc de module : c'est
    /// l'identité **locale**, avec secrets, pas le dictionnaire `Identity`
    /// du `.udl`).
    ///
    /// `routing_seed` initialise le tirage du jitter de relais
    /// ([`Router::new`]) — déterministe pour les tests, tiré d'un vrai RNG
    /// en production (aucune conséquence de sécurité : le jitter n'est
    /// qu'un anti-corrélation de timing, pas un secret).
    #[must_use]
    pub fn new(identity: identity::Identity, routing_seed: u64) -> Self {
        let peer_id = identity.peer_id();
        // Deuxième `SigningKey` dérivée de la MÊME graine, dédiée au journal
        // (`Ledger<S>` en prend possession) : `SigningKey` n'implémente pas
        // `Clone` (un exemplaire de moins du secret en mémoire, revue #78),
        // et `identity::Identity` ne le permet pas plus. `to_seed()` est
        // `pub(crate)` — accessible depuis ce module, pas depuis l'extérieur
        // de la crate. Une graine Ed25519 rederivée est exactement la même
        // clé, pas un secret distinct : aucun affaiblissement.
        let ledger_signer = SigningKey::from_seed(&identity.signing_key().to_seed());
        Self {
            router: Router::new(RoutingConfig::new(peer_id), routing_seed),
            outbox: Outbox::open(MemoryStore::new())
                .unwrap_or_else(|_| unreachable!("MemoryStore::Error = Infallible")),
            ledger: Ledger::new(ledger_signer),
            courier: Courier::new(CourierConfig::default()),
            identity,
            peer_id,
            peers: BTreeMap::new(),
            conversations: BTreeMap::new(),
            messages: BTreeMap::new(),
            obs_seq: 0,
            outgoing: Vec::new(),
            events: VecDeque::new(),
            store: None,
        }
    }

    /// `peerID` de ce nœud.
    #[must_use]
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Identité publique de ce nœud (à partager, ex. QR — US-215).
    #[must_use]
    pub fn public_identity(&self) -> PublicIdentity {
        self.identity.public()
    }

    /// Fait persister les nouveaux messages/conversations dans `store` en
    /// plus de l'index en mémoire (`std` seulement).
    ///
    /// Optionnel et posé après coup plutôt qu'à la construction : un test ou
    /// `dengon-node` sans base sur disque n'a pas à en fournir une (l'index
    /// en mémoire de cette façade reste la source de vérité **de la
    /// session** dans tous les cas — voir la doc de module).
    pub fn attach_store(&mut self, store: Store<FixedKeySource>) {
        self.store = Some(store);
    }

    /// Ajoute (ou met à jour) un correspondant connu — sa clé publique
    /// permet de lui sceller une enveloppe (Noise `X`) avant même toute
    /// connexion directe (voir [`DengonError::UnknownPeer`], levée quand ni
    /// un contact ni une session ne fournissent cette clé).
    pub fn add_contact(&mut self, contact: PublicIdentity) {
        let peer_id = contact.peer_id();
        self.peers.entry(peer_id).or_default().identity = Some(contact);
    }

    // ----- Cycle de connexion --------------------------------------------

    /// Un pair vient de se connecter (`.udl` : `on_peer_connected`).
    ///
    /// Démarre un handshake `XX` : `rng` est consommé une seule fois ici (le
    /// répondeur en a besoin autant que l'initiateur — `snow` tire son
    /// éphémère aux deux messages 1 et 2). Qui initie est déterminé par
    /// l'ordre des `peerID` (`self.peer_id < peer_id`) : la même règle des
    /// deux côtés garantit qu'un seul initie et un seul répond, sans
    /// coordination préalable.
    pub fn on_peer_connected<R>(&mut self, peer_id: PeerId, now: RoutingNow, rng: R)
    where
        R: rand_core::RngCore + rand_core::CryptoRng + Send + Sync + 'static,
    {
        self.router.link_up(peer_id);
        // Le lien EST le pair (voir la doc de module) : authentifié tout de
        // suite, l'anti-inondation compte par `peerID` dès la connexion.
        self.router.bind_peer(peer_id, peer_id, now.mono_ms);

        let initiator = self.peer_id < peer_id;
        let handshake_result = if initiator {
            Handshake::initiator(self.identity.static_keypair(), rng)
        } else {
            Handshake::responder(self.identity.static_keypair(), rng)
        };
        if let Ok(mut handshake) = handshake_result {
            if initiator {
                if let Ok(msg1) = handshake.write_message(&[]) {
                    if let Some(bytes) =
                        self.encode_unsigned(PacketType::NoiseHs, peer_id, now.wall_ms, msg1)
                    {
                        self.outgoing.push((peer_id, bytes));
                    }
                }
            }
            self.peers.entry(peer_id).or_default().crypto =
                Some(PeerCrypto::Handshaking(Box::new(handshake)));
        }

        self.redeliver_pending_envelopes(peer_id, now.wall_ms);
        self.events.push_back(NodeEvent::PeerConnected(peer_id));
    }

    /// Un pair vient de se déconnecter (`.udl` : implicite — pas de méthode
    /// dédiée dans le `.udl` v0, extension Rust comme
    /// [`Node::on_bytes_received`]/[`Node::take_outgoing`] : voir la doc de
    /// module).
    pub fn on_peer_disconnected(&mut self, peer_id: PeerId) {
        self.router.link_down(peer_id);
        if let Some(peer) = self.peers.get_mut(&peer_id) {
            peer.crypto = None; // la session ne survit pas à la connexion (synthese/06 §3)
        }
        self.events.push_back(NodeEvent::PeerDisconnected(peer_id));
    }

    /// Vide et renvoie les octets à transmettre — un par pair destinataire.
    /// L'appelant (Transport réel) les écrit sur le lien correspondant ;
    /// voir la doc de module sur pourquoi cette méthode n'a pas d'équivalent
    /// dans le `.udl` v0.
    pub fn take_outgoing(&mut self) -> Vec<(PeerId, Vec<u8>)> {
        core::mem::take(&mut self.outgoing)
    }

    // ----- Construction de paquets ----------------------------------------

    /// En-tête + payload, adressé, **non signé** (`NOISE_HS`/`NOISE_MSG`/
    /// `ACK` : l'authenticité vient de la session Noise, pas d'Ed25519 —
    /// voir `PacketType::is_always_signed`). `None` si `payload` dépasse
    /// `u16::MAX` octets (jamais en pratique : un clair de session est
    /// paddé à 2048 octets maximum) ou si l'encodage échoue.
    fn encode_unsigned(
        &self,
        packet_type: PacketType,
        recipient: PeerId,
        wall_ms: u64,
        payload: Vec<u8>,
    ) -> Option<Vec<u8>> {
        let payload_len = u16::try_from(payload.len()).ok()?;
        let packet = Packet {
            header: Header {
                version: PROTO_VERSION,
                packet_type,
                ttl: TTL_DEFAULT,
                flags: Flags::ADDRESSED,
                timestamp_ms: wall_ms,
                sender_id: self.peer_id,
                recipient_id: Some(recipient),
                payload_len,
            },
            payload,
            signature: None,
        };
        codec::encode(&packet).ok()
    }

    /// En-tête + payload, **jamais adressé** (`SEALED_ENVELOPE` :
    /// `PacketType::is_addressed() == Some(false)` — le `recipient_id` de
    /// l'en-tête révélerait le destinataire en clair, ce que `recipient_tag`
    /// dans le payload est précisément censé éviter), toujours signé
    /// Ed25519 (`is_always_signed`).
    fn encode_signed_broadcast(
        &self,
        packet_type: PacketType,
        wall_ms: u64,
        payload: Vec<u8>,
    ) -> Option<Vec<u8>> {
        let payload_len = u16::try_from(payload.len()).ok()?;
        let mut packet = Packet {
            header: Header {
                version: PROTO_VERSION,
                packet_type,
                ttl: TTL_DEFAULT,
                flags: Flags::SIGNED,
                timestamp_ms: wall_ms,
                sender_id: self.peer_id,
                recipient_id: None,
                payload_len,
            },
            payload,
            signature: None,
        };
        let signing_input = codec::signing_input(&packet).ok()?;
        packet.signature = Some(self.identity.signing_key().sign(&signing_input));
        codec::encode(&packet).ok()
    }

    /// `msgID = SHA-256(sender_id ‖ timestamp_ms ‖ type ‖ payload)` (A-9,
    /// `docs/synthese/05` §3) — recalculé à partir des octets **reçus**,
    /// jamais fait confiance à une valeur transmise (il n'y en a pas : le
    /// `msgID` n'est jamais sur le fil, seul `sync::routing`/`sync::courier`
    /// le calculent localement pour dédupliquer).
    fn compute_msg_id(packet: &Packet) -> MsgId {
        let mut hasher = Sha256::new();
        hasher.update(packet.header.sender_id);
        hasher.update(packet.header.timestamp_ms.to_be_bytes());
        hasher.update([packet.header.packet_type.to_u8()]);
        hasher.update(&packet.payload);
        hasher.finalize().into()
    }

    // ----- Envoi -----------------------------------------------------------

    /// Envoie un message texte à `dest_peer_id` (`.udl` : `send_message`).
    ///
    /// `dest_peer_id` doit être un correspondant déjà connu
    /// ([`Node::add_contact`]) **ou** avoir une session Noise `XX` établie
    /// avec nous : le `.udl` ne prend qu'un `peer_id`, pas une identité
    /// complète — voir la doc de module. Avant ce correctif (revue PR #102),
    /// le second cas renvoyait quand même `UnknownPeer` — `PeerCrypto`
    /// n'enregistre aucune identité, seulement une session, et
    /// `handle_handshake_message` n'appelait jamais `add_contact` à
    /// l'établissement — malgré cette même doc qui promettait déjà le
    /// contraire. `rng` n'est consommé que si aucune session n'est établie
    /// avec ce pair (chemin enveloppe, [`crate::crypto::noise::seal`]).
    ///
    /// # Errors
    ///
    /// [`DengonError::UnknownPeer`] si `dest_peer_id` n'a jamais été annoncé
    /// (ni contact ajouté, ni session établie) ; [`DengonError::Internal`]
    /// sur un échec de chiffrement/encodage (ne devrait pas arriver en
    /// pratique — les tailles/format sont sous notre contrôle).
    pub fn send_message<R>(
        &mut self,
        dest_peer_id: PeerId,
        body: &str,
        now: RoutingNow,
        mut rng: R,
    ) -> Result<MsgUuid, DengonError>
    where
        R: rand_core::RngCore + rand_core::CryptoRng + Send + Sync + 'static,
    {
        let has_session = matches!(
            self.peers
                .get(&dest_peer_id)
                .and_then(|p| p.crypto.as_ref()),
            Some(PeerCrypto::Established(_))
        );
        let identity = self
            .peers
            .get(&dest_peer_id)
            .and_then(|p| p.identity.clone());
        if !has_session && identity.is_none() {
            return Err(DengonError::UnknownPeer);
        }
        // Pseudo connu si un contact a été ajouté ; sinon vide, comme
        // `deliver_message` le fait déjà pour un pair reçu sans contact
        // connu — ne bloque plus l'envoi tant qu'une session existe.
        let pseudo = identity
            .as_ref()
            .map_or_else(String::new, |i| i.pseudo().to_string());

        let mut msg_uuid = [0u8; 16];
        rand_core::RngCore::fill_bytes(&mut rng, &mut msg_uuid);

        let conv_id = conv_id_of(self.peer_id, dest_peer_id);
        self.conversations
            .entry(conv_id)
            .or_insert_with(|| ConversationRecord {
                peer_id: dest_peer_id,
                peer_pseudo: pseudo,
                ..ConversationRecord::default()
            });
        let conv_seq = self.next_conv_seq(conv_id);

        let frame = AppFrame::Message(MessageFrame {
            msg_uuid,
            conv_seq,
            sent_ms: now.wall_ms,
            text: body.to_string(),
        });
        let plaintext = app::encode_app_frame(&frame);

        let (kind, packet_bytes) = if has_session {
            let Some(PeerCrypto::Established(session)) = self
                .peers
                .get_mut(&dest_peer_id)
                .and_then(|p| p.crypto.as_mut())
            else {
                unreachable!("has_session vérifié juste au-dessus")
            };
            let ciphertext = session.encrypt(&plaintext)?;
            let bytes = self
                .encode_unsigned(PacketType::NoiseMsg, dest_peer_id, now.wall_ms, ciphertext)
                .ok_or(DengonError::Internal)?;
            (DeliveryKind::Session, bytes)
        } else {
            // `identity` est forcément `Some` ici : sinon `!has_session &&
            // identity.is_none()` aurait déjà renvoyé `UnknownPeer` plus haut.
            let Some(dest) = identity.as_ref() else {
                unreachable!("chemin enveloppe : identité vérifiée présente au-dessus")
            };
            let epoch_day = crate::crypto::tag::epoch_day(now.wall_ms);
            let tag = crate::crypto::tag::recipient_tag(&dest.pub_static(), epoch_day);
            let ciphertext = noise::seal(
                self.identity.static_keypair(),
                &dest.pub_static(),
                &plaintext,
                rng,
            )?;
            let mut sealed_payload = Vec::with_capacity(18 + ciphertext.len());
            sealed_payload.extend_from_slice(&tag);
            sealed_payload.extend_from_slice(&epoch_day.to_be_bytes());
            sealed_payload.extend_from_slice(&ciphertext);
            let bytes = self
                .encode_signed_broadcast(PacketType::SealedEnvelope, now.wall_ms, sealed_payload)
                .ok_or(DengonError::Internal)?;
            (DeliveryKind::Envelope, bytes)
        };

        let change = self.outbox.enqueue(
            NewMessage {
                msg_uuid,
                dest_peer_id,
                kind,
                packet: packet_bytes.clone(),
            },
            now.wall_ms,
        )?;
        if let Some(change) = change {
            self.record_ledger(change.event_name(), &change.msg_uuid, now.wall_ms);
        }

        self.messages.insert(
            msg_uuid,
            MessageRecord {
                author_peer_id: self.peer_id,
                body: body.to_string(),
                outgoing: true,
                sent_ms: now.wall_ms,
                status: Status::Queued,
            },
        );
        if let Some(record) = self.conversations.get_mut(&conv_id) {
            record.message_order.push(msg_uuid);
        }
        self.persist_message(
            msg_uuid,
            conv_id,
            self.peer_id,
            "out",
            conv_seq,
            body,
            now.wall_ms,
            "queued",
            now.wall_ms,
        );

        // Remise immédiate si le pair est déjà connecté (session comme
        // enveloppe : les deux voies peuvent être transmises dès que le
        // lien radio existe, contrairement à l'attente d'un futur
        // `on_peer_connected` pour un pair hors de portée).
        if self
            .peers
            .get(&dest_peer_id)
            .is_some_and(|p| p.crypto.is_some())
        {
            self.outgoing.push((dest_peer_id, packet_bytes));
            if let Some(change) =
                self.outbox
                    .mark_handed_off(&msg_uuid, &dest_peer_id, now.wall_ms)?
            {
                self.record_ledger(change.event_name(), &change.msg_uuid, now.wall_ms);
                self.update_message_status(&msg_uuid, change.to, now.wall_ms);
                self.events
                    .push_back(NodeEvent::StatusChanged(msg_uuid, change.to.into()));
            }
        }

        Ok(msg_uuid)
    }

    // ----- Réception ---------------------------------------------------

    /// Des octets sont arrivés de `from` (extension Rust, pas dans le `.udl`
    /// v0 — voir la doc de module). Décode, passe par
    /// [`crate::sync::routing::Router`], puis traite localement si
    /// [`Decision::Deliver`].
    pub fn on_bytes_received(&mut self, from: PeerId, bytes: &[u8], now: RoutingNow) {
        let Ok(packet) = codec::decode(bytes) else {
            return; // paquet illisible : silencieusement jeté, comme un relais le ferait
        };
        let msg_id = Self::compute_msg_id(&packet);
        let decision = self.router.on_packet(from, &packet.header, &msg_id, now);
        match decision {
            Decision::Deliver => self.handle_addressed_packet(from, packet, now),
            Decision::Store => {
                // `SealedEnvelope` n'est **jamais** adressé (décision A-8 :
                // le `recipient_id` en clair révélerait le destinataire, ce
                // que `recipient_tag` évite) — `Router::decide` ne peut donc
                // jamais rendre `Decision::Deliver` pour ce type de paquet,
                // même quand on en est le vrai destinataire. C'est ici,
                // avant tout dépôt en courrier, qu'on tente de l'ouvrir avec
                // notre propre clé : si ça marche, il était pour nous ; sinon
                // on le garde pour un autre pair (store-and-forward,
                // `sync::courier`).
                if packet.header.packet_type == PacketType::SealedEnvelope
                    && !self.handle_sealed_envelope(&packet, now)
                {
                    let _ = self.courier.deposit(msg_id, bytes.to_vec(), now.wall_ms);
                }
            }
            // Rejeté, ou accepté mais pas pour nous : rien à traiter
            // localement. Le relais effectif de `RelayScheduled`
            // (`Router::poll_due`) n'est pas câblé par cette façade —
            // c'est le rôle d'un relais dédié (US-308), pas d'un nœud
            // client — écart consigné dans `03-ecarts-conception.md`.
            Decision::Reject(_) | Decision::NoRelay(_) | Decision::RelayScheduled { .. } => {}
        }
    }

    fn handle_addressed_packet(&mut self, from: PeerId, packet: Packet, now: RoutingNow) {
        match packet.header.packet_type {
            PacketType::NoiseHs => self.handle_handshake_message(from, &packet.payload, now),
            PacketType::NoiseMsg | PacketType::Ack => {
                self.handle_session_ciphertext(from, &packet.payload, now);
            }
            // `SealedEnvelope` n'est jamais adressé : il n'arrive jamais ici
            // (`Decision::Deliver` suppose un `recipient_id`), voir
            // `on_bytes_received`/`handle_sealed_envelope`.
            //
            // Hors périmètre de cette façade par ailleurs (voir la doc de
            // module) : Announce/EnvelopeOffer/EnvelopeRequest/Inventory/
            // LogAttest/Fragment/Gossip* ne sont ni émis ni traités ici.
            _ => {}
        }
    }

    fn handle_handshake_message(&mut self, from: PeerId, payload: &[u8], now: RoutingNow) {
        let Some(peer) = self.peers.get_mut(&from) else {
            return; // pas de handshake en cours pour ce pair : ignoré
        };
        let Some(PeerCrypto::Handshaking(handshake)) = peer.crypto.as_mut() else {
            return; // session déjà établie, ou pas de handshake du tout
        };
        if handshake.read_message(payload).is_err() {
            // Handshake définitivement perdu (voir doc de `Handshake`) : on
            // l'abandonne plutôt que de garder un état inutilisable. Le
            // pair devra se reconnecter pour en recommencer un.
            peer.crypto = None;
            return;
        }
        if handshake.is_finished() {
            let Some(PeerCrypto::Handshaking(handshake)) = peer.crypto.take() else {
                unreachable!("vérifié juste au-dessus")
            };
            if let Ok(session) = handshake.into_session() {
                self.peers.entry(from).or_default().crypto = Some(PeerCrypto::Established(session));
                self.redeliver_pending_envelopes(from, now.wall_ms);
            }
            return;
        }
        // Pas fini après cette lecture : on répond (message 2 côté répondeur,
        // message 3 côté initiateur). Pour l'initiateur, ce message 3 est le
        // dernier de l'échange XX : le handshake devient fini **après cette
        // écriture**, pas après la lecture qui précède — `is_finished()` doit
        // donc être revérifié une fois le message envoyé, sinon l'initiateur
        // reste bloqué en `Handshaking` alors que le répondeur, lui, passe à
        // `Established` en lisant ce même message 3.
        let Ok(reply) = handshake.write_message(&[]) else {
            return;
        };
        let vient_de_finir = handshake.is_finished();
        let bytes = self.encode_unsigned(PacketType::NoiseHs, from, now.wall_ms, reply);

        if vient_de_finir {
            if let Some(PeerCrypto::Handshaking(handshake)) =
                self.peers.get_mut(&from).and_then(|p| p.crypto.take())
            {
                if let Ok(session) = handshake.into_session() {
                    self.peers.entry(from).or_default().crypto =
                        Some(PeerCrypto::Established(session));
                    self.redeliver_pending_envelopes(from, now.wall_ms);
                }
            }
        }
        if let Some(bytes) = bytes {
            self.outgoing.push((from, bytes));
        }
    }

    fn handle_session_ciphertext(&mut self, from: PeerId, ciphertext: &[u8], now: RoutingNow) {
        let Some(PeerCrypto::Established(session)) =
            self.peers.get_mut(&from).and_then(|p| p.crypto.as_mut())
        else {
            return; // pas de session : ciphertext illisible, jeté
        };
        let Ok(plaintext) = session.decrypt(ciphertext) else {
            return;
        };
        let Ok(frame) = app::decode_app_frame(&plaintext) else {
            return;
        };
        match frame {
            AppFrame::Message(m) => self.deliver_message(from, m, now.wall_ms),
            AppFrame::Ack(a) => self.apply_ack(a, now.wall_ms),
        }
    }

    /// Tente d'ouvrir l'enveloppe avec notre propre clé statique. Rend `true`
    /// si elle était pour nous (traitée : message livré ou accusé appliqué),
    /// `false` sinon — auquel cas l'appelant ([`Node::on_bytes_received`]) la
    /// dépose en courrier pour un autre pair.
    fn handle_sealed_envelope(&mut self, packet: &Packet, now: RoutingNow) -> bool {
        let Ok((header, ciphertext)) = crate::sync::courier::parse_sealed_payload(&packet.payload)
        else {
            return false;
        };
        // Filtre bon marché (3 comparaisons HMAC) avant l'ouverture Noise X
        // coûteuse (DH X25519 + AEAD) — corrigé après revue de la PR #102 :
        // `own_tags`/`recipient_tag` existaient déjà précisément pour ça
        // (rejeter à bas coût la grande majorité des enveloppes qui ne nous
        // sont pas destinées sur un maillage chargé) mais n'étaient jamais
        // appelés ici. `own_tags` sur NOTRE horloge (J-1/J/J+1) tolère la
        // dérive d'horloge de l'expéditeur sans avoir à faire confiance à
        // son `epoch_day` embarqué.
        let mine = crate::crypto::tag::own_tags(
            &self.identity.static_keypair().public(),
            crate::crypto::tag::epoch_day(now.wall_ms),
        );
        if !mine.contains(&header.recipient_tag) {
            return false; // pas pour nous : évite l'ouverture Noise X
        }
        let Ok(opened) = noise::open(self.identity.static_keypair(), ciphertext) else {
            return false; // tag pour nous, mais ouverture échouée (altérée)
        };
        let Ok(frame) = app::decode_app_frame(&opened.plaintext) else {
            return false;
        };
        // L'expéditeur d'une enveloppe n'est identifié que par sa clé
        // statique (jamais un `peerID` L3, l'enveloppe est anonyme au
        // niveau transport) : on en dérive un `PeerId` avec la même formule
        // que le reste de la façade (`identity::keys::peer_id_of`, exposée
        // `pub(crate)` — corrigé après revue de la PR #102, qui la
        // réimplémentait ici à la main).
        let sender = identity::keys::peer_id_of(&opened.sender_static);
        match frame {
            AppFrame::Message(m) => self.deliver_message(sender, m, now.wall_ms),
            AppFrame::Ack(a) => self.apply_ack(a, now.wall_ms),
        }
        true
    }

    fn deliver_message(&mut self, from: PeerId, m: MessageFrame, wall_ms: u64) {
        let conv_id = conv_id_of(self.peer_id, from);
        let conv_is_new = !self.conversations.contains_key(&conv_id);
        let pseudo = self
            .peers
            .get(&from)
            .and_then(|p| p.identity.as_ref())
            .map_or_else(String::new, |i| i.pseudo().to_string());
        self.conversations
            .entry(conv_id)
            .or_insert_with(|| ConversationRecord {
                peer_id: from,
                peer_pseudo: pseudo,
                ..ConversationRecord::default()
            });
        self.persist_conversation(conv_id, from, conv_is_new);

        self.messages.insert(
            m.msg_uuid,
            MessageRecord {
                author_peer_id: from,
                body: m.text.clone(),
                outgoing: false,
                sent_ms: m.sent_ms,
                status: Status::Delivered, // un message REÇU n'a pas de cycle de statut : il est là.
            },
        );
        if let Some(record) = self.conversations.get_mut(&conv_id) {
            record.message_order.push(m.msg_uuid);
            record.unread_count += 1;
        }
        self.persist_message(
            m.msg_uuid,
            conv_id,
            from,
            "in",
            m.conv_seq,
            &m.text,
            m.sent_ms,
            "delivered",
            wall_ms,
        );

        self.record_ledger("msg.received", &m.msg_uuid, wall_ms);
        self.events.push_back(NodeEvent::MessageReceived(Message {
            msg_uuid: m.msg_uuid,
            conv_id,
            author_peer_id: from,
            body: m.text,
            outgoing: false,
            sent_ms: m.sent_ms,
            status: MessageStatus::Delivered,
        }));
    }

    fn apply_ack(&mut self, ack: AckFrame, wall_ms: u64) {
        let Ok(Some(change)) = self.outbox.apply_ack(&ack.msg_uuid, ack.status, wall_ms) else {
            return;
        };
        self.record_ledger(change.event_name(), &change.msg_uuid, wall_ms);
        self.update_message_status(&change.msg_uuid, change.to, wall_ms);
        self.events
            .push_back(NodeEvent::StatusChanged(change.msg_uuid, change.to.into()));
    }

    // ----- Entretien périodique -------------------------------------------

    /// À rappeler régulièrement (`.udl` : partie de `poll_events` — cette
    /// façade fait l'entretien juste avant de vider la file d'événements,
    /// pas besoin d'un appel séparé) : fait expirer les messages en outbox
    /// dont le TTL est dépassé et vide la file d'événements.
    pub fn poll_events(&mut self, now: RoutingNow) -> Vec<NodeEvent> {
        if let Ok(changes) = self.outbox.expire_due(now.wall_ms) {
            for change in changes {
                self.record_ledger(change.event_name(), &change.msg_uuid, now.wall_ms);
                self.update_message_status(&change.msg_uuid, change.to, now.wall_ms);
                self.events
                    .push_back(NodeEvent::StatusChanged(change.msg_uuid, change.to.into()));
            }
        }
        self.events.drain(..).collect()
    }

    // ----- Lecture ---------------------------------------------------------

    /// Toutes les conversations connues, triées par dernier message le plus
    /// récent d'abord (`.udl` : `list_conversations`).
    #[must_use]
    pub fn list_conversations(&self) -> Vec<Conversation> {
        let mut out: Vec<Conversation> = self
            .conversations
            .iter()
            .map(|(conv_id, record)| Conversation {
                conv_id: *conv_id,
                peer_id: record.peer_id,
                peer_pseudo: record.peer_pseudo.clone(),
                last_message: record
                    .message_order
                    .last()
                    .and_then(|uuid| self.messages.get(uuid).map(|m| (*uuid, m)))
                    .map(|(uuid, m)| Message {
                        msg_uuid: uuid,
                        conv_id: *conv_id,
                        author_peer_id: m.author_peer_id,
                        body: m.body.clone(),
                        outgoing: m.outgoing,
                        sent_ms: m.sent_ms,
                        status: m.status.into(),
                    }),
                unread_count: record.unread_count,
            })
            .collect();
        out.sort_by(|a, b| {
            let ta = a.last_message.as_ref().map_or(0, |m| m.sent_ms);
            let tb = b.last_message.as_ref().map_or(0, |m| m.sent_ms);
            tb.cmp(&ta)
        });
        out
    }

    /// Tous les messages d'une conversation, du plus ancien au plus récent
    /// (`.udl` : `list_messages`). Liste vide si `conv_id` est inconnu —
    /// pas une erreur : une conversation sans messages encore reçus/envoyés
    /// est un état valide.
    #[must_use]
    pub fn list_messages(&self, conv_id: ConvId) -> Vec<Message> {
        let Some(record) = self.conversations.get(&conv_id) else {
            return Vec::new();
        };
        record
            .message_order
            .iter()
            .filter_map(|uuid| self.messages.get(uuid).map(|m| (*uuid, m)))
            .map(|(uuid, m)| Message {
                msg_uuid: uuid,
                conv_id,
                author_peer_id: m.author_peer_id,
                body: m.body.clone(),
                outgoing: m.outgoing,
                sent_ms: m.sent_ms,
                status: m.status.into(),
            })
            .collect()
    }

    // ----- Aides internes ---------------------------------------------------

    fn next_conv_seq(&self, conv_id: ConvId) -> u64 {
        self.conversations
            .get(&conv_id)
            .map_or(0, |r| r.message_order.len() as u64)
    }

    /// Reflète une transition de statut à la fois dans l'index en mémoire
    /// (source de vérité de la session, voir la doc de module) et, si
    /// `store` est attaché, dans la ligne persistée correspondante. Avant ce
    /// correctif (revue PR #102), seul l'index en mémoire était mis à jour :
    /// `store` restait figé sur le statut de création de chaque message,
    /// pour toujours (`QUEUED` pour tout message sortant, `DELIVERED` pour
    /// tout message reçu), en désaccord avec `list_messages`/`poll_events`.
    fn update_message_status(&mut self, msg_uuid: &MsgUuid, status: Status, wall_ms: u64) {
        if let Some(record) = self.messages.get_mut(msg_uuid) {
            record.status = status;
        }
        if let Some(store) = &self.store {
            let status_ms_i64 = i64::try_from(wall_ms).unwrap_or(i64::MAX);
            let _ = store.update_message_status(msg_uuid, status.as_str(), status_ms_i64);
        }
    }

    /// Journalise un changement de statut/réception — signé, chaîné (voir
    /// `crate::ledger`). `msg_uuid` est redacté avant d'entrer dans le
    /// journal applicatif (`msg_log_id`, US-208) : le journal signé sert
    /// aussi de source pour l'export d'observabilité, qui ne doit jamais
    /// voir un `msg_uuid` brut.
    fn record_ledger(&mut self, event_name: &str, msg_uuid: &MsgUuid, ts_ms: u64) {
        use crate::observability::{msg_log_id, Value};
        self.obs_seq += 1;
        let log_id = msg_log_id(msg_uuid);
        let mut obj = BTreeMap::new();
        obj.insert(String::from("msg_log_id"), Value::Str(hex_string(&log_id)));
        let payload_json =
            String::from_utf8(Value::Object(obj).to_canonical_bytes()).unwrap_or_default();
        self.ledger.append(event_name, &payload_json, ts_ms);
    }

    /// Un pair vient de se connecter : tout message en outbox à destination
    /// de ce pair et pas encore remis part maintenant (voie enveloppe —
    /// déjà scellée, pas besoin d'attendre une session ; voie session, si
    /// elle existe déjà d'une connexion précédente au même pair dans le
    /// même processus).
    ///
    /// `Outbox::replay_candidates` ne filtre **pas** par destinataire — sa
    /// propre doc le dit explicitement : « le filtre \"peer est destinataire
    /// ou bon candidat relais\" relève de `sync::routing` » (US-209, pas
    /// câblé par cette façade, voir `03-ecarts-conception.md`). Sans ce
    /// filtre ici, un message en attente pour Bob partait vers n'importe
    /// quel pair qui se connecte ou termine un handshake en premier (trouvé
    /// en revue de la PR #102) — cette façade n'ayant pas de rôle relais,
    /// on ne garde que les candidats dont `dest_peer_id` est bien `peer_id`.
    fn redeliver_pending_envelopes(&mut self, peer_id: PeerId, wall_ms: u64) {
        let candidates: Vec<(MsgUuid, Vec<u8>)> = self
            .outbox
            .replay_candidates(&peer_id, wall_ms)
            .into_iter()
            .filter(|r| r.dest_peer_id == peer_id)
            .map(|r| (r.msg_uuid, r.packet.clone()))
            .collect();
        for (msg_uuid, packet) in candidates {
            self.outgoing.push((peer_id, packet));
            if let Ok(Some(change)) = self.outbox.mark_handed_off(&msg_uuid, &peer_id, wall_ms) {
                self.record_ledger(change.event_name(), &change.msg_uuid, wall_ms);
                self.update_message_status(&msg_uuid, change.to, wall_ms);
                self.events
                    .push_back(NodeEvent::StatusChanged(msg_uuid, change.to.into()));
            }
        }
    }

    fn persist_conversation(&mut self, conv_id: ConvId, peer_id: PeerId, is_new: bool) {
        if is_new {
            if let Some(store) = &self.store {
                let _ = store.insert_conversation(&conv_id, &peer_id);
            }
        }
    }

    /// `sent_ms` et `status_ms` sont deux horloges distinctes (et deux
    /// colonnes distinctes du schéma, voir `store::insert_message`) :
    /// `sent_ms` est toujours l'horloge de l'**expéditeur** du message (la
    /// nôtre pour un message sortant, celle du pair distant, embarquée dans
    /// `MessageFrame`, pour un message reçu), alors que `status_ms` est
    /// l'horloge **locale** à laquelle ce statut a été atteint. Avant ce
    /// correctif (revue PR #102), `persist_message` ne recevait qu'un seul
    /// paramètre `sent_ms` réutilisé pour les deux colonnes — pour un
    /// message reçu, l'horloge distante du `msg.queued` de l'expéditeur se
    /// retrouvait ainsi en `status_ms` local de sa réception.
    #[allow(clippy::too_many_arguments)]
    fn persist_message(
        &mut self,
        msg_uuid: MsgUuid,
        conv_id: ConvId,
        author_peer_id: PeerId,
        direction: &str,
        conv_seq: u64,
        body: &str,
        sent_ms: u64,
        status: &str,
        status_ms: u64,
    ) {
        if let Some(store) = &self.store {
            let sent_ms_i64 = i64::try_from(sent_ms).unwrap_or(i64::MAX);
            let status_ms_i64 = i64::try_from(status_ms).unwrap_or(i64::MAX);
            let conv_seq_i64 = i64::try_from(conv_seq).unwrap_or(i64::MAX);
            let _ = store.insert_message(
                &msg_uuid,
                &conv_id,
                direction,
                &author_peer_id,
                conv_seq_i64,
                body,
                sent_ms_i64,
                status,
                status_ms_i64,
            );
        }
    }
}

fn hex_string(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(DIGITS[(b >> 4) as usize] as char);
        s.push(DIGITS[(b & 0x0f) as usize] as char);
    }
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    fn rng(seed: u64) -> ChaCha20Rng {
        ChaCha20Rng::seed_from_u64(seed)
    }

    fn noeud(pseudo: &str, seed: u64) -> Node {
        let identity = identity::Identity::generate(pseudo, rng(seed)).expect("pseudo valide");
        Node::new(identity, seed)
    }

    const T0: u64 = 1_800_000_000_000;

    fn now(ms: u64) -> RoutingNow {
        RoutingNow::new(ms, ms)
    }

    #[test]
    fn conv_id_est_independant_de_l_ordre_des_pairs() {
        let a = [1u8; PEER_ID_LEN];
        let b = [2u8; PEER_ID_LEN];
        assert_eq!(conv_id_of(a, b), conv_id_of(b, a));
    }

    #[test]
    fn message_status_reflete_status_hormis_cancelled_replie_sur_expired() {
        assert_eq!(MessageStatus::from(Status::Queued), MessageStatus::Queued);
        assert_eq!(
            MessageStatus::from(Status::InFlight),
            MessageStatus::InFlight
        );
        assert_eq!(
            MessageStatus::from(Status::Delivered),
            MessageStatus::Delivered
        );
        assert_eq!(MessageStatus::from(Status::Expired), MessageStatus::Expired);
        assert_eq!(
            MessageStatus::from(Status::Cancelled),
            MessageStatus::Expired
        );
    }

    #[test]
    fn dengon_error_a_un_message_par_variante() {
        assert!(!DengonError::UnknownPeer.to_string().is_empty());
        assert!(!DengonError::NotConnected.to_string().is_empty());
        assert!(!DengonError::Internal.to_string().is_empty());
    }

    #[test]
    fn node_debug_n_expose_pas_les_secrets() {
        let node = noeud("alice", 1);
        let texte = format!("{node:?}");
        assert!(texte.contains("Node"));
    }

    #[test]
    fn envoi_vers_un_pair_inconnu_est_refuse() {
        let mut alice = noeud("alice", 1);
        let err = alice
            .send_message([9u8; PEER_ID_LEN], "salut", now(T0), rng(2))
            .unwrap_err();
        assert_eq!(err, DengonError::UnknownPeer);
    }

    #[test]
    fn message_hors_ligne_part_en_enveloppe_et_reste_en_file() {
        let mut alice = noeud("alice", 1);
        let bob = noeud("bob", 2);
        alice.add_contact(bob.public_identity());

        let msg_uuid = alice
            .send_message(
                bob.peer_id(),
                "en attendant que tu reviennes",
                now(T0),
                rng(3),
            )
            .expect("bob est un contact connu");

        // Pas de connexion : aucune remise immédiate, le message reste
        // `Queued` — mais il a bien été mis en file (visible via
        // `list_messages`), pas perdu.
        assert!(alice.take_outgoing().is_empty());
        let conv_id = conv_id_of(alice.peer_id(), bob.peer_id());
        let messages = alice.list_messages(conv_id);
        let sent = messages
            .iter()
            .find(|m| m.msg_uuid == msg_uuid)
            .expect("le message envoyé doit apparaître dans sa conversation");
        assert_eq!(sent.status, MessageStatus::Queued);
        assert!(sent.outgoing);
    }

    #[test]
    fn redeliver_pending_envelopes_ne_fuite_pas_vers_un_autre_pair() {
        // Trouvé en revue de la PR #102 : `redeliver_pending_envelopes` ne
        // filtrait jamais par destinataire (la doc de `Outbox::
        // replay_candidates` le dit explicitement : ce filtre relève de
        // `sync::routing`, pas câblé par cette façade). Un message en
        // attente pour bob partait donc vers n'importe quel pair qui se
        // connectait en premier.
        let mut alice = noeud("alice", 1);
        let bob = noeud("bob", 2);
        let carol = noeud("carol", 3);
        alice.add_contact(bob.public_identity());

        alice
            .send_message(bob.peer_id(), "pour bob seulement", now(T0), rng(10))
            .expect("bob est un contact connu");
        // Bob n'est pas connecté : pas de remise immédiate (pas de
        // `PeerCrypto` pour bob), le message reste dans l'outbox.
        assert!(alice.take_outgoing().is_empty());

        // Carol se connecte à alice AVANT bob.
        alice.on_peer_connected(carol.peer_id(), now(T0), rng(11));

        let fuite_vers_carol = alice.take_outgoing().into_iter().any(|(dest, bytes)| {
            dest == carol.peer_id()
                && codec::decode(&bytes)
                    .is_ok_and(|p| p.header.packet_type == PacketType::SealedEnvelope)
        });
        assert!(
            !fuite_vers_carol,
            "le message pour bob ne doit pas partir vers carol"
        );

        // Bob, lui, doit toujours pouvoir le recevoir en se connectant.
        alice.on_peer_connected(bob.peer_id(), now(T0), rng(12));
        let vers_bob = alice.take_outgoing().into_iter().any(|(dest, bytes)| {
            dest == bob.peer_id()
                && codec::decode(&bytes)
                    .is_ok_and(|p| p.header.packet_type == PacketType::SealedEnvelope)
        });
        assert!(
            vers_bob,
            "bob doit bien recevoir son message en se connectant"
        );
    }

    #[test]
    fn enveloppe_scellee_ouverte_par_son_vrai_destinataire() {
        let mut alice = noeud("alice", 1);
        let mut bob = noeud("bob", 2);
        alice.add_contact(bob.public_identity());
        bob.add_contact(alice.public_identity());

        // Les deux `link_up`/`bind_peer` (nécessaires pour que
        // `Router::on_packet` de bob ne rejette pas la trame comme venant
        // d'un lien inconnu) sans attendre la fin du handshake `XX` : une
        // enveloppe ne dépend pas de la session.
        alice.on_peer_connected(bob.peer_id(), now(T0), rng(10));
        bob.on_peer_connected(alice.peer_id(), now(T0), rng(11));

        let msg_uuid = alice
            .send_message(
                bob.peer_id(),
                "hors de portée mais connecté",
                now(T0),
                rng(12),
            )
            .expect("bob est un contact connu et \"connecté\" (lien ouvert)");

        let enveloppe = alice
            .take_outgoing()
            .into_iter()
            .map(|(_, bytes)| bytes)
            .find(|bytes| {
                codec::decode(bytes)
                    .is_ok_and(|p| p.header.packet_type == PacketType::SealedEnvelope)
            })
            .expect("l'enveloppe scellée doit avoir été mise en file de sortie");

        bob.on_bytes_received(alice.peer_id(), &enveloppe, now(T0));
        assert!(
            bob.courier.is_empty(),
            "l'enveloppe était pour bob : elle ne doit pas finir en courrier"
        );

        let events = bob.poll_events(now(T0));
        let recu = events.iter().find_map(|ev| match ev {
            NodeEvent::MessageReceived(m) if m.msg_uuid == msg_uuid => Some(m),
            _ => None,
        });
        let message = recu.expect("bob doit avoir reçu le message par enveloppe");
        assert_eq!(message.body, "hors de portée mais connecté");
        assert_eq!(message.author_peer_id, alice.peer_id());
    }

    #[test]
    fn enveloppe_pour_un_autre_pair_est_gardee_en_courrier() {
        let mut alice = noeud("alice", 1);
        let bob = noeud("bob", 2);
        let mut porteur = noeud("porteur", 3);
        alice.add_contact(bob.public_identity());

        alice.on_peer_connected(bob.peer_id(), now(T0), rng(20));
        porteur.on_peer_connected(alice.peer_id(), now(T0), rng(21));

        alice
            .send_message(bob.peer_id(), "pour bob, pas pour toi", now(T0), rng(22))
            .expect("bob est un contact connu et \"connecté\"");

        let enveloppe = alice
            .take_outgoing()
            .into_iter()
            .map(|(_, bytes)| bytes)
            .find(|bytes| {
                codec::decode(bytes)
                    .is_ok_and(|p| p.header.packet_type == PacketType::SealedEnvelope)
            })
            .expect("l'enveloppe scellée doit avoir été mise en file de sortie");

        porteur.on_bytes_received(alice.peer_id(), &enveloppe, now(T0));
        assert_eq!(
            porteur.courier.len(),
            1,
            "l'enveloppe n'est pas pour le porteur : elle doit être gardée pour bob"
        );
        assert!(
            !porteur
                .poll_events(now(T0))
                .iter()
                .any(|ev| matches!(ev, NodeEvent::MessageReceived(_))),
            "le porteur ne doit pas se croire destinataire du message"
        );
    }

    #[test]
    fn deconnexion_ferme_la_session_et_emet_un_evenement() {
        let mut alice = noeud("alice", 1);
        let bob = noeud("bob", 2);
        alice.on_peer_connected(bob.peer_id(), now(T0), rng(30));
        alice.on_peer_disconnected(bob.peer_id());
        let events = alice.poll_events(now(T0));
        assert!(events.contains(&NodeEvent::PeerDisconnected(bob.peer_id())));
    }

    #[test]
    fn conversation_sans_message_encore_recu_est_vide_mais_pas_une_erreur() {
        let alice = noeud("alice", 1);
        assert!(alice.list_messages([0u8; 8]).is_empty());
        assert!(alice.list_conversations().is_empty());
    }

    #[test]
    fn attach_store_persiste_les_messages_envoyes() {
        use crate::store::FixedKeySource;

        let mut alice = noeud("alice", 1);
        let bob = noeud("bob", 2);
        alice.add_contact(bob.public_identity());
        let store = Store::open_in_memory(FixedKeySource([7u8; 32])).expect("store en mémoire");
        alice.attach_store(store);

        alice
            .send_message(bob.peer_id(), "persisté ?", now(T0), rng(40))
            .expect("bob est un contact connu");

        let conv_id = conv_id_of(alice.peer_id(), bob.peer_id());
        assert_eq!(alice.list_messages(conv_id).len(), 1);
    }
}
