//! Relais dédié — la logique du nœud ESP32 (US-308), en `no_std` + `alloc`.
//!
//! Câble ensemble ce que `sync` et `ledger` livrent séparément, pour un nœud
//! qui **transporte** sans jamais lire : [`Router`] (dédup, TTL, relais
//! jitteré), [`Inventory`] (réconciliation `INVENTORY` + push du manquant),
//! [`Courier`] (dépôt d'enveloppes scellées, `ENVELOPE_OFFER` /
//! `ENVELOPE_REQUEST`) et [`Ledger`] (journal chaîné signé). C'est ce que
//! `api` (côté client, `std`) laisse explicitement au relais dédié.
//!
//! # Aucune I/O
//!
//! Comme le reste de la crate, le relais ne possède ni radio, ni horloge,
//! ni stockage : le firmware (C, tâches FreeRTOS) lui passe les trames
//! reçues ([`Relay::on_frame`]), l'heure ([`Now`]), et récupère ce qu'il faut
//! émettre ([`Relay::take_outgoing`]) et persister
//! ([`Relay::take_ledger_entries`], [`Relay::ledger_anchor`]). Le journal
//! reprend après `esp_restart()` à partir de la seule ancre
//! ([`Ledger::resume`]), sans relire le fichier.
//!
//! # Voisins : appris par `ANNOUNCE`
//!
//! La couche transport ne donne qu'un identifiant de lien, jamais le
//! `peerID` du voisin. `INVENTORY` et `ENVELOPE_OFFER` étant **adressés**, le
//! relais attend le premier `ANNOUNCE` authentique reçu sur un lien
//! ([`Announce::verify`]) pour le lier à ce pair, puis lui envoie son
//! inventaire et ses offres. Il émet lui-même un `ANNOUNCE` à chaque
//! ouverture de lien. Limite assumée : un `ANNOUNCE` *relayé* arrivé en
//! premier sur un lien neuf lierait ce lien au mauvais pair (écart consigné).
//!
//! # Horloge murale
//!
//! Le routeur rejette tout paquet daté de plus de 2 h dans le futur. Un
//! ESP32 sans Wi-Fi n'a pas l'heure : tant que l'appelant fournit une heure
//! antérieure à [`WALL_CLOCK_MIN_MS`], le relais adopte celle du plus récent
//! `ANNOUNCE` authentique reçu (décalage appliqué à l'horloge monotone). La
//! synchronisation SNTP arrive avec le Wi-Fi (US-309).

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::crypto::{SigningKey, VerifyingKey};
use crate::identity::Identity;
use crate::ledger::{Anchor, Entry, Ledger};
use crate::observability::batch::{self, BatchError};
use crate::observability::{catalog, hex, msg_log_id, NodeKind, Value};
use crate::protocol::codec::announce::{Announce, AnnounceError, CAP_RELAY};
use crate::protocol::codec::{self, received_signing_input, Packet};
use crate::protocol::consts::{
    HEADER_LEN_ADDRESSED, MSG_ID_LEN, PROTO_VERSION, RECIPIENT_TAG_LEN, SIGNATURE_LEN,
};
use crate::protocol::{Flags, Header, MsgId, PacketType, PeerId, TTL_OFFSET};
use crate::sync::courier::{
    self, Courier, CourierConfig, CourierError, DepositOutcome, RecipientTag,
};
use crate::sync::inventory::{self, Inventory, InventoryConfig};
use crate::sync::routing::{Decision, RejectReason, Router, RoutingConfig};

pub use crate::sync::routing::Now;

/// En deçà (2024-01-01T00:00:00Z), l'heure murale fournie par l'appelant est
/// considérée comme inconnue (ESP32 sans SNTP : horloge partie de 1970).
pub const WALL_CLOCK_MIN_MS: u64 = 1_704_067_200_000;

/// Longueur maximale du champ `pseudo` de `peer.announce_seen` (catalogue).
const LOG_PSEUDO_MAX: usize = 32;

/// Plus grande trame qu'un lien BLE porte en un seul PDU (`ATT_MTU 517 - 3`).
/// Le relais ne fragmente pas ce qu'il émet : ses propres paquets doivent
/// tenir dans une trame.
pub const FRAME_MAX: usize = 514;

/// Surcoût d'un paquet adressé et signé : en-tête (30) + signature (64).
const SIGNED_ADDRESSED_OVERHEAD: usize = HEADER_LEN_ADDRESSED + SIGNATURE_LEN;

/// `msgID` au plus dans un `INVENTORY` du relais, pour tenir en une trame
/// (13 : `2 + 13 × 32 + 94 = 512` octets).
pub const RELAY_INVENTORY_MAX_IDS: usize = (FRAME_MAX - SIGNED_ADDRESSED_OVERHEAD - 2) / MSG_ID_LEN;

/// `recipient_tag` au plus dans un `ENVELOPE_OFFER` du relais (26).
pub const RELAY_OFFER_MAX_TAGS: usize =
    (FRAME_MAX - SIGNED_ADDRESSED_OVERHEAD - 2) / RECIPIENT_TAG_LEN;

/// Réglages du relais.
#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// Pseudo annoncé (libre, borné à 255 octets par le format `ANNOUNCE`).
    pub pseudo: String,
    /// Graine du tirage de jitter du routeur.
    pub routing_seed: u64,
    /// Réglages de la réconciliation d'inventaire.
    pub inventory: InventoryConfig,
    /// Réglages du magasin d'enveloppes.
    pub courier: CourierConfig,
}

impl RelayConfig {
    /// Valeurs par défaut du protocole, avec ce pseudo et cette graine.
    pub fn new(pseudo: &str, routing_seed: u64) -> Self {
        Self {
            pseudo: String::from(pseudo),
            routing_seed,
            inventory: InventoryConfig {
                max_ids: RELAY_INVENTORY_MAX_IDS,
                ..InventoryConfig::new()
            },
            courier: CourierConfig::default(),
        }
    }
}

/// Secrets du relais, générés une fois puis persistés (NVS côté firmware).
#[derive(Clone)]
pub struct RelaySecrets {
    /// Secret X25519 (clé statique Noise, d'où dérive le `peerID`).
    pub dh_secret: [u8; 32],
    /// Graine Ed25519 (signature des paquets et du journal).
    pub sign_seed: [u8; 32],
}

impl fmt::Debug for RelaySecrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RelaySecrets(..)")
    }
}

/// Compteurs cumulés, pour `relay.health` et les tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RelayStats {
    /// Trames illisibles (décodage L3 refusé).
    pub malformed: u64,
    /// `ANNOUNCE` / paquets signés refusés (usurpation, signature).
    pub unauthentic: u64,
    /// Paquets effectivement relayés (une fois par paquet, pas par cible).
    pub relayed: u64,
    /// Enveloppes déposées dans le magasin.
    pub envelopes_stored: u64,
    /// Enveloppes remises sur `ENVELOPE_REQUEST`.
    pub envelopes_handed_off: u64,
    /// Paquets reçus avant que l'heure murale soit connue, donc ignorés.
    pub clock_unknown: u64,
}

#[derive(Debug, Clone, Copy)]
struct Voisin {
    peer: PeerId,
    key: VerifyingKey,
}

#[derive(Debug)]
struct EnAttente {
    bytes: Vec<u8>,
    packet_type: u8,
    ttl_in: u8,
    from_peer: PeerId,
}

/// Le relais : état de routage, caches et journal, piloté par le firmware.
///
/// `L` est l'identifiant de lien de la couche transport (`uint64_t` côté C).
pub struct Relay<L> {
    identity: Identity,
    router: Router<L>,
    inventory: Inventory<L>,
    courier: Courier,
    ledger: Ledger<SigningKey>,
    links: BTreeMap<L, Option<Voisin>>,
    a_relayer: BTreeMap<MsgId, EnAttente>,
    /// `recipient_tag` des enveloppes détenues : `Courier::expire` ne rend
    /// que les `msgID`, l'événement `envelope.expired` exige aussi le tag.
    tags: BTreeMap<MsgId, RecipientTag>,
    /// `mur - mono` appris d'un `ANNOUNCE` (voir la note de module).
    wall_offset: Option<u64>,
    outgoing: Vec<(L, Vec<u8>)>,
    stats: RelayStats,
}

impl<L> fmt::Debug for Relay<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Relay")
            .field("peer_id", &hex(&self.identity.peer_id()))
            .field("links", &self.links.len())
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl<L: Copy + Ord> Relay<L> {
    /// Relais d'identité `secrets`, dont le journal reprend à `anchor`
    /// ([`Anchor::GENESIS`] au tout premier démarrage).
    pub fn new(secrets: &RelaySecrets, anchor: Anchor, cfg: RelayConfig) -> Self {
        let identity = Identity::from_parts(&secrets.dh_secret, &secrets.sign_seed, cfg.pseudo);
        let peer_id = identity.peer_id();
        Self {
            router: Router::new(RoutingConfig::new(peer_id), cfg.routing_seed),
            inventory: Inventory::new(cfg.inventory),
            courier: Courier::new(cfg.courier),
            ledger: Ledger::resume(anchor, SigningKey::from_seed(&secrets.sign_seed)),
            identity,
            links: BTreeMap::new(),
            a_relayer: BTreeMap::new(),
            tags: BTreeMap::new(),
            wall_offset: None,
            outgoing: Vec::new(),
            stats: RelayStats::default(),
        }
    }

    /// `peerID` du relais.
    pub fn peer_id(&self) -> PeerId {
        self.identity.peer_id()
    }

    /// Clé publique Ed25519 du relais : celle à passer à
    /// `dengon-verify --pubkey` pour vérifier son journal.
    pub fn verifying_key(&self) -> [u8; 32] {
        self.identity.signing_key().verifying_key().to_bytes()
    }

    /// Compteurs cumulés.
    pub fn stats(&self) -> RelayStats {
        self.stats
    }

    /// Nombre d'enveloppes détenues.
    pub fn envelopes_held(&self) -> usize {
        self.courier.len()
    }

    /// Nombre de paquets au cache de réconciliation.
    pub fn cache_len(&self) -> usize {
        self.inventory.len()
    }

    /// `true` une fois l'heure murale connue (fournie ou apprise).
    pub fn clock_known(&self, now: Now) -> bool {
        self.clock(now).is_some()
    }

    // ----- Liens -----------------------------------------------------------

    /// Un lien vient de s'ouvrir : le relais s'y annonce.
    pub fn link_up(&mut self, link: L, now: Now) {
        self.router.link_up(link);
        self.links.insert(link, None);
        let now = self.clock_or_mono(now);
        let announce = Announce {
            peer_id: self.peer_id(),
            pub_static: self.identity.static_keypair().public(),
            pub_sign: self.verifying_key(),
            pseudo: String::from(self.identity.pseudo()),
            ledger_height: self.ledger_anchor().first_seq,
            caps: CAP_RELAY,
        };
        if let Ok(payload) = announce.encode() {
            self.emit(link, PacketType::Announce, None, payload, now.wall_ms);
        }
    }

    /// Le lien est tombé.
    pub fn link_down(&mut self, link: L) {
        self.router.link_down(link);
        self.inventory.link_down(link);
        self.links.remove(&link);
    }

    // ----- Réception -------------------------------------------------------

    /// Une trame est arrivée sur `link`.
    pub fn on_frame(&mut self, link: L, raw: &[u8], now: Now) {
        if !self.links.contains_key(&link) {
            return;
        }
        let Ok(packet) = codec::decode(raw) else {
            self.stats.malformed += 1;
            let from = self.voisin(link).map_or([0; 8], |v| v.peer);
            self.record_rejected(from, "malformed", now);
            return;
        };

        if packet.header.packet_type == PacketType::Announce {
            self.on_announce(link, &packet, raw, now);
        }

        let Some(now) = self.clock(now) else {
            self.stats.clock_unknown += 1;
            return;
        };
        let id = codec::msg_id(&packet);
        let decision = self.router.on_packet(link, &packet.header, &id, now);
        let from_peer = self.voisin(link).map_or([0; 8], |v| v.peer);

        match decision {
            Decision::Reject(RejectReason::BadVersion) => {
                self.record_rejected(from_peer, "bad_version", now);
            }
            Decision::Reject(_) => return,
            Decision::Deliver => self.on_delivered(link, &packet, raw, now),
            Decision::RelayScheduled { .. } => {
                self.a_relayer.insert(
                    id,
                    EnAttente {
                        bytes: raw.to_vec(),
                        packet_type: packet.header.packet_type.to_u8(),
                        ttl_in: packet.header.ttl,
                        from_peer,
                    },
                );
            }
            Decision::Store | Decision::NoRelay(_) => {}
        }

        if packet.header.packet_type == PacketType::SealedEnvelope
            && !matches!(decision, Decision::Deliver)
        {
            self.deposit(link, id, &packet, raw, now);
        }
        if let Some(ttl) = inventory::cacheable(&packet.header, &decision) {
            let _ = self
                .inventory
                .remember(id, packet.header.timestamp_ms, ttl, raw.to_vec(), now);
        }
    }

    fn on_announce(&mut self, link: L, packet: &Packet, raw: &[u8], now: Now) {
        let announce = match Announce::verify(packet, raw) {
            Ok(a) => a,
            Err(e) => {
                self.stats.unauthentic += 1;
                let reason = match e {
                    AnnounceError::Malformed | AnnounceError::NotAnnounce => "malformed",
                    AnnounceError::PeerIdMismatch => "peerid_mismatch",
                    AnnounceError::BadSignature => "bad_sig",
                };
                self.record_rejected(packet.header.sender_id, reason, now);
                return;
            }
        };
        if announce.peer_id == self.peer_id() {
            return; // notre propre annonce, revenue par un autre chemin
        }

        // Heure apprise : on ne fait qu'avancer, jamais reculer.
        let ts = packet.header.timestamp_ms;
        if ts >= WALL_CLOCK_MIN_MS && now.wall_ms < WALL_CLOCK_MIN_MS {
            let offset = ts.saturating_sub(now.mono_ms);
            if self.wall_offset.is_none_or(|o| offset > o) {
                self.wall_offset = Some(offset);
            }
        }

        // Premier `ANNOUNCE` authentique sur un lien encore anonyme.
        let Some(slot) = self.links.get_mut(&link) else {
            return;
        };
        if slot.is_some() {
            return;
        }
        let Ok(key) = VerifyingKey::from_bytes(&announce.pub_sign) else {
            return;
        };
        *slot = Some(Voisin {
            peer: announce.peer_id,
            key,
        });
        let now = self.clock_or_mono(now);
        self.router.bind_peer(link, announce.peer_id, now.mono_ms);

        let mut pseudo = announce.pseudo.clone();
        while pseudo.chars().count() > LOG_PSEUDO_MAX {
            pseudo.pop();
        }
        self.record(
            "peer.announce_seen",
            &obj([
                ("peer", Value::Str(hex(&announce.peer_id))),
                ("pseudo", Value::Str(pseudo)),
                (
                    "ledger_height",
                    Value::Int(i64::try_from(announce.ledger_height).unwrap_or(i64::MAX)),
                ),
                ("caps", Value::Int(i64::from(announce.caps))),
            ]),
            now,
        );

        // Réconciliation : notre inventaire, puis nos offres d'enveloppes.
        let ids = self.inventory.link_up(link, now);
        if let Ok(payload) = inventory::encode_payload(&ids) {
            self.emit(
                link,
                PacketType::Inventory,
                Some(announce.peer_id),
                payload,
                now.wall_ms,
            );
        }
        self.offer(link, announce.peer_id, now);
    }

    fn on_delivered(&mut self, link: L, packet: &Packet, raw: &[u8], now: Now) {
        let Some(voisin) = self.voisin(link) else {
            return; // adressé à nous par un lien anonyme : ignoré
        };
        let authentique = packet.header.sender_id == voisin.peer
            && packet.signature.as_ref().is_some_and(|sig| {
                received_signing_input(raw)
                    .ok()
                    .flatten()
                    .is_some_and(|input| voisin.key.verify(&input, sig).is_ok())
            });
        if !authentique {
            self.stats.unauthentic += 1;
            self.record_rejected(voisin.peer, "bad_sig", now);
            return;
        }
        match packet.header.packet_type {
            PacketType::Inventory => {
                if let Ok(ids) = inventory::decode_payload(&packet.payload) {
                    let _ = self.inventory.on_inventory(link, &ids, now);
                }
            }
            PacketType::EnvelopeRequest => {
                if let Ok(tags) = courier::decode_tag_list(&packet.payload) {
                    self.hand_off(link, voisin.peer, &tags, now);
                }
            }
            // Le relais n'est destinataire d'aucune enveloppe ni session :
            // `ENVELOPE_OFFER`, `NOISE_*`, `ACK` qui lui seraient adressés
            // sont ignorés.
            _ => {}
        }
    }

    // ----- Courrier --------------------------------------------------------

    fn deposit(&mut self, from: L, id: MsgId, packet: &Packet, raw: &[u8], now: Now) {
        let Ok((sealed, _)) = courier::parse_sealed_payload(&packet.payload) else {
            return;
        };
        match self.courier.deposit(id, raw.to_vec(), now.wall_ms) {
            Ok(DepositOutcome::Stored { evicted }) => {
                self.stats.envelopes_stored += 1;
                self.tags.insert(id, sealed.recipient_tag);
                if let Some(old) = evicted {
                    self.record_expired(old, "store_full", now);
                }
                self.record(
                    "envelope.stored",
                    &obj([
                        ("msg_log_id", Value::Str(hex(&msg_log_id(&id)))),
                        ("recipient_tag", Value::Str(hex(&sealed.recipient_tag))),
                        ("epoch_day", Value::Int(i64::from(sealed.epoch_day))),
                        ("copy_budget", Value::Int(0)),
                    ]),
                    now,
                );
                // Nouvelle enveloppe : proposée aux voisins déjà liés, sauf
                // celui qui vient de nous la donner.
                let voisins: Vec<(L, PeerId)> = self
                    .links
                    .iter()
                    .filter(|(l, _)| **l != from)
                    .filter_map(|(l, v)| v.map(|v| (*l, v.peer)))
                    .collect();
                for (link, peer) in voisins {
                    let payload = courier::encode_tag_list(&[sealed.recipient_tag]);
                    self.emit(
                        link,
                        PacketType::EnvelopeOffer,
                        Some(peer),
                        payload,
                        now.wall_ms,
                    );
                }
            }
            Ok(DepositOutcome::Duplicate) => {}
            Err(CourierError::Full) => {
                self.record(
                    "relay.overloaded",
                    &obj([
                        ("subsystem", Value::Str(String::from("courier"))),
                        ("action", Value::Str(String::from("refuse_envelope"))),
                    ]),
                    now,
                );
            }
            Err(_) => {}
        }
    }

    fn offer(&mut self, link: L, peer: PeerId, now: Now) {
        let mut tags = self.courier.offer(now.wall_ms);
        if tags.is_empty() {
            return;
        }
        // Une trame au plus ; le reste sera offert à la prochaine rencontre
        // (ou à chaque nouveau dépôt, voir `deposit`).
        tags.truncate(RELAY_OFFER_MAX_TAGS);
        let count = i64::try_from(tags.len()).unwrap_or(i64::MAX);
        self.emit(
            link,
            PacketType::EnvelopeOffer,
            Some(peer),
            courier::encode_tag_list(&tags),
            now.wall_ms,
        );
        self.record(
            "envelope.offered",
            &obj([
                ("count", Value::Int(count)),
                ("to_peer", Value::Str(hex(&peer))),
            ]),
            now,
        );
    }

    fn hand_off(&mut self, link: L, peer: PeerId, tags: &[RecipientTag], now: Now) {
        let remises: Vec<(MsgId, RecipientTag, Vec<u8>)> = self
            .courier
            .matching(tags, now.wall_ms)
            .into_iter()
            .map(|e| (e.msg_id, e.recipient_tag, e.packet.clone()))
            .collect();
        for (id, tag, packet) in remises {
            // Remise considérée faite dès la mise en file d'émission : la
            // couche transport ne confirme pas la réception (écart consigné).
            self.outgoing.push((link, packet));
            self.courier.confirm_handoff(&id);
            self.tags.remove(&id);
            self.stats.envelopes_handed_off += 1;
            self.record(
                "envelope.handoff",
                &obj([
                    ("msg_log_id", Value::Str(hex(&msg_log_id(&id)))),
                    ("recipient_tag", Value::Str(hex(&tag))),
                    ("to_peer", Value::Str(hex(&peer))),
                    ("budget_after", Value::Int(0)),
                ]),
                now,
            );
        }
    }

    // ----- Échéances -------------------------------------------------------

    /// Fait avancer toutes les échéances : [`Relay::poll_routing`],
    /// [`Relay::poll_inventory`] puis [`Relay::poll_courier`].
    pub fn poll(&mut self, now: Now) {
        self.poll_routing(now);
        self.poll_inventory(now);
        self.poll_courier(now);
    }

    /// Relais jitterés arrivés à échéance (cadence fine : 10 à 220 ms,
    /// tâche `route` du firmware).
    pub fn poll_routing(&mut self, now: Now) {
        let Some(now) = self.clock(now) else {
            return;
        };
        for ordre in self.router.poll_due(now.mono_ms) {
            let Some(mut attente) = self.a_relayer.remove(&ordre.msg_id) else {
                continue;
            };
            if let Some(ttl) = attente.bytes.get_mut(TTL_OFFSET) {
                *ttl = ordre.ttl;
            }
            let fanout = u8::try_from(ordre.targets.len()).unwrap_or(u8::MAX);
            for cible in &ordre.targets {
                self.outgoing.push((*cible, attente.bytes.clone()));
            }
            self.stats.relayed += 1;
            let payload = crate::observability::pkt_relayed(
                msg_log_id(&ordre.msg_id),
                attente.packet_type,
                attente.ttl_in,
                ordre.ttl,
                fanout,
                attente.from_peer,
            );
            self.record("pkt.relayed", &payload, now);
        }
        // Relais annulés par le routeur (doublons entendus) : leurs octets
        // ne seront jamais réclamés.
        if self.router.pending_len() == 0 {
            self.a_relayer.clear();
        }
    }

    /// Pushs d'inventaire cadencés (`PUSH_MAX_PER_MIN`, tâche `inventory`).
    pub fn poll_inventory(&mut self, now: Now) {
        let Some(now) = self.clock(now) else {
            return;
        };
        for push in self.inventory.poll_push(now) {
            self.outgoing.push((push.target, push.bytes.to_vec()));
        }
    }

    /// Expiration des enveloppes détenues (cadence lente, tâche `courier`).
    pub fn poll_courier(&mut self, now: Now) {
        let Some(now) = self.clock(now) else {
            return;
        };
        for id in self.courier.expire(now.wall_ms) {
            self.record_expired(id, "ttl", now);
        }
    }

    /// Prochaine échéance (ms d'horloge **monotone**) à laquelle appeler
    /// [`Relay::poll`], s'il y en a une.
    pub fn next_deadline(&self, now: Now) -> Option<u64> {
        let now = self.clock_or_mono(now);
        match (
            self.router.next_deadline(),
            self.inventory.next_deadline(now),
        ) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Trames à émettre, `(lien, octets)`, dans l'ordre. Vide la file.
    pub fn take_outgoing(&mut self) -> Vec<(L, Vec<u8>)> {
        core::mem::take(&mut self.outgoing)
    }

    // ----- Journal ---------------------------------------------------------

    /// Ajoute au journal un événement produit par le firmware (`relay.boot`,
    /// `peer.connected`…). Refusé (`false`) si `name` n'est pas au catalogue.
    pub fn record_event(&mut self, name: &str, payload_json: &str, now: Now) -> bool {
        if !catalog::is_known(name) {
            return false;
        }
        let now = self.clock_or_mono(now);
        self.ledger.append(name, payload_json, now.wall_ms);
        true
    }

    /// Entrées du journal pas encore persistées (vidées de la RAM).
    pub fn take_ledger_entries(&mut self) -> Vec<Entry> {
        self.ledger.take_entries()
    }

    /// Ancre de la prochaine entrée : le curseur à persister.
    pub fn ledger_anchor(&self) -> Anchor {
        self.ledger.anchor()
    }

    // ----- Export vers le dashboard (US-309) -------------------------------

    /// `node_id` du relais côté dashboard : `relay-` + `peerID[0..3]` en hex.
    pub fn node_id(&self) -> String {
        batch::relay_node_id(&self.peer_id())
    }

    /// Corps de `POST /ingest/batch` pour ces entrées du journal, signé par
    /// la clé Ed25519 du relais (qui ne sort jamais de ce handle).
    pub fn build_batch(&self, entries: &[Entry]) -> Result<Vec<u8>, BatchError> {
        batch::signed_batch_from_entries(
            entries,
            &self.node_id(),
            NodeKind::Relay,
            self.identity.signing_key(),
        )
    }

    // ----- Interne ---------------------------------------------------------

    /// Heure effective : celle de l'appelant si elle est plausible, sinon
    /// l'horloge monotone décalée de l'heure apprise, sinon inconnue.
    fn clock(&self, now: Now) -> Option<Now> {
        if now.wall_ms >= WALL_CLOCK_MIN_MS {
            return Some(now);
        }
        self.wall_offset
            .map(|o| Now::new(now.mono_ms.saturating_add(o), now.mono_ms))
    }

    /// Comme [`Relay::clock`], mais retombe sur l'horloge monotone : pour
    /// horodater le journal et nos propres paquets même sans heure.
    fn clock_or_mono(&self, now: Now) -> Now {
        self.clock(now)
            .unwrap_or_else(|| Now::new(now.mono_ms, now.mono_ms))
    }

    fn voisin(&self, link: L) -> Option<Voisin> {
        self.links.get(&link).copied().flatten()
    }

    /// Émet un paquet du relais lui-même : TTL 1 (strictement local, pas de
    /// `RELAY_OK`), toujours signé.
    fn emit(
        &mut self,
        link: L,
        packet_type: PacketType,
        recipient: Option<PeerId>,
        payload: Vec<u8>,
        wall_ms: u64,
    ) {
        let Ok(payload_len) = u16::try_from(payload.len()) else {
            return;
        };
        let mut flags = Flags::SIGNED;
        if recipient.is_some() {
            flags = flags | Flags::ADDRESSED;
        }
        let mut packet = Packet {
            header: Header {
                version: PROTO_VERSION,
                packet_type,
                ttl: 1,
                flags,
                timestamp_ms: wall_ms,
                sender_id: self.peer_id(),
                recipient_id: recipient,
                payload_len,
            },
            payload,
            signature: None,
        };
        let Ok(input) = codec::signing_input(&packet) else {
            return;
        };
        packet.signature = Some(self.identity.signing_key().sign(&input));
        if let Ok(bytes) = codec::encode(&packet) {
            self.outgoing.push((link, bytes));
        }
    }

    fn record(&mut self, name: &str, payload: &Value, now: Now) {
        let json = String::from_utf8(payload.to_canonical_bytes()).unwrap_or_default();
        self.ledger.append(name, &json, now.wall_ms);
    }

    fn record_rejected(&mut self, from_peer: PeerId, reason: &str, now: Now) {
        let now = self.clock_or_mono(now);
        self.record(
            "pkt.rejected",
            &obj([
                ("from_peer", Value::Str(hex(&from_peer))),
                ("reason", Value::Str(String::from(reason))),
            ]),
            now,
        );
    }

    fn record_expired(&mut self, id: MsgId, reason: &str, now: Now) {
        let tag = self.tags.remove(&id).unwrap_or_default();
        self.record(
            "envelope.expired",
            &obj([
                ("msg_log_id", Value::Str(hex(&msg_log_id(&id)))),
                ("recipient_tag", Value::Str(hex(&tag))),
                ("reason", Value::Str(String::from(reason))),
            ]),
            now,
        );
    }
}

fn obj<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(k, v)| (String::from(k), v))
            .collect(),
    )
}

#[cfg(test)]
mod tests;
