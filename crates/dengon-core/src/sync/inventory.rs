//! Réconciliation par échange d'inventaire (US-210).
//!
//! Référence : `docs/synthese/05-protocole-et-trame.md` §4 (paquet
//! `INVENTORY`), §6.2 (réconciliation à chaque nouveau pair) et §7
//! (store-and-forward) ; `docs/synthese/08-relais-esp32.md` §1 et §5 (cache de
//! réconciliation). Tests attendus : `docs/synthese/10-benchmarks-mvp-tests.md`
//! §4.2 (« réconciliation A/B convergente ; push repasse par le pipeline »).
//!
//! # Le principe
//!
//! Chaque nœud garde un **cache de réconciliation** : les octets bruts des
//! paquets récents qu'il porte pour d'autres. Quand un voisin arrive (après
//! `ANNOUNCE` + handshake), chacun lui envoie la liste de ses `msgID`
//! (paquet `INVENTORY`, [`Inventory::link_up`]) ; à la réception de la liste
//! de l'autre ([`Inventory::on_inventory`]), il **pousse ce qui lui manque**
//! ([`Inventory::poll_push`]). Les paquets poussés sont des paquets ordinaires
//! chez le receveur : ils repassent par [`super::routing::Router::on_packet`]
//! (dédup, fraîcheur, anti-inondation, éventuel re-relais).
//!
//! # Sans I/O
//!
//! Même contrat que [`super::routing`] : machine à états pure, générique sur
//! l'identifiant de lien `L`, heure fournie par l'appelant ([`Now`]). Le
//! module n'encode que le **payload** `INVENTORY` ([`encode_payload`] /
//! [`decode_payload`]) ; l'en-tête L3 et la signature restent à
//! `protocol::codec` et `crypto`.
//!
//! # Push cadencé
//!
//! Le routeur du voisin refuse plus de [`FLOOD_MAX_PER_MIN_PEER`] (20)
//! nouveaux `msgID` par minute venant de nous. Pousser d'un bloc un cache de
//! 120 paquets ferait rejeter 100 paquets en `FloodLimited` — de la bande BLE
//! gaspillée. Le push est donc une **file par lien**, vidée à au plus
//! [`PUSH_MAX_PER_MIN`] paquets par minute ; la marge laisse passer
//! l'`INVENTORY` lui-même et le trafic en direct. Écart consigné dans
//! `docs/suivi/03-ecarts-conception.md`.
//!
//! # Préconditions (hors de ce module)
//!
//! - **Quoi mettre en cache** : l'appelant passe à [`Inventory::remember`]
//!   les paquets pour lesquels [`cacheable`] rend un TTL — la décision du
//!   routeur fait foi.
//! - **Signature** de l'`INVENTORY` reçu : vérifiée en amont, comme pour
//!   tout paquet (`crypto`).
//! - **ACK** : `status` / `courier` (US-211/212) appellent
//!   [`Inventory::forget`] quand un message est acquitté, en plus de
//!   [`super::routing::Router::cancel`].

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::vec::Vec;
use core::fmt;

use super::routing::{Decision, Now, RateWindow};
use crate::protocol::consts::{FLOOD_MAX_PER_MIN_PEER, MSG_ID_LEN, MSG_TTL_S};
use crate::protocol::{Flags, Header, MsgId, PacketType};

/// Capacité du cache de réconciliation, en paquets.
///
/// `synthese/08` §5 : « ~120 paquets » sur ESP32-WROOM (~64 Ko de SRAM).
/// Valeur par défaut de [`InventoryConfig`] ; un téléphone peut monter plus
/// haut.
pub const INVENTORY_CACHE_CAP: usize = 120;

/// Durée de rétention d'un paquet dans le cache, en ms (6 h, `synthese/08`
/// §1 et §5).
pub const INVENTORY_WINDOW_MS: u64 = 6 * 60 * 60 * 1_000;

/// Paquets poussés par minute et par lien. Strictement sous
/// [`FLOOD_MAX_PER_MIN_PEER`] (voir « Push cadencé » dans la doc du module).
pub const PUSH_MAX_PER_MIN: u16 = 15;

/// Nombre maximal de `msgID` dans un payload `INVENTORY` : `count(2) ‖
/// msgID[count]` doit tenir dans `payload_len`, un `u16`.
pub const INVENTORY_MAX_IDS: usize = (u16::MAX as usize - 2) / MSG_ID_LEN;

/// Fenêtre de la cadence de push, en ms (« par minute »).
const PUSH_WINDOW_MS: u64 = 60_000;

// Le cadencement n'a de sens que sous le quota d'anti-inondation du voisin.
const _: () = assert!(PUSH_MAX_PER_MIN < FLOOD_MAX_PER_MIN_PEER);

// --- Payload INVENTORY -----------------------------------------------------

/// Payload `INVENTORY` mal formé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadError {
    /// Moins de 2 octets : pas de `count`.
    Truncated,
    /// Plus de [`INVENTORY_MAX_IDS`] `msgID` (annoncés ou à encoder).
    TooMany(usize),
    /// La longueur ne correspond pas à `2 + count × 32`.
    LengthMismatch {
        /// Longueur attendue d'après `count`.
        expected: usize,
        /// Longueur reçue.
        got: usize,
    },
}

impl fmt::Display for PayloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "payload INVENTORY tronqué (count absent)"),
            Self::TooMany(n) => write!(
                f,
                "payload INVENTORY : {n} msgID, au plus {INVENTORY_MAX_IDS}"
            ),
            Self::LengthMismatch { expected, got } => write!(
                f,
                "payload INVENTORY : {got} octets, {expected} attendus d'après count"
            ),
        }
    }
}

/// Encode `count(2) ‖ msgID[count]` (`synthese/05` §4), `count` big-endian.
///
/// # Errors
///
/// [`PayloadError::TooMany`] au-delà de [`INVENTORY_MAX_IDS`] identifiants.
pub fn encode_payload(ids: &[MsgId]) -> Result<Vec<u8>, PayloadError> {
    let count = u16::try_from(ids.len())
        .ok()
        .filter(|_| ids.len() <= INVENTORY_MAX_IDS)
        .ok_or(PayloadError::TooMany(ids.len()))?;
    let mut out = Vec::with_capacity(2 + ids.len() * MSG_ID_LEN);
    out.extend_from_slice(&count.to_be_bytes());
    for id in ids {
        out.extend_from_slice(id);
    }
    Ok(out)
}

/// Décode un payload `INVENTORY`. Les doublons éventuels sont conservés
/// (sans effet sur [`Inventory::on_inventory`]).
///
/// # Errors
///
/// [`PayloadError`] si le payload n'est pas exactement `2 + count × 32`
/// octets, ou si `count` dépasse [`INVENTORY_MAX_IDS`].
pub fn decode_payload(payload: &[u8]) -> Result<Vec<MsgId>, PayloadError> {
    let (tete, reste) = payload
        .split_first_chunk::<2>()
        .ok_or(PayloadError::Truncated)?;
    let count = usize::from(u16::from_be_bytes(*tete));
    if count > INVENTORY_MAX_IDS {
        return Err(PayloadError::TooMany(count));
    }
    let expected = 2 + count * MSG_ID_LEN;
    if payload.len() != expected {
        return Err(PayloadError::LengthMismatch {
            expected,
            got: payload.len(),
        });
    }
    let mut ids = Vec::with_capacity(count);
    for bloc in reste.chunks_exact(MSG_ID_LEN) {
        let mut id = [0u8; MSG_ID_LEN];
        id.copy_from_slice(bloc);
        ids.push(id);
    }
    Ok(ids)
}

/// Le paquet reçu doit-il entrer au cache de réconciliation ? Rend le TTL
/// à écrire dans l'en-tête quand on le poussera, ou `None`.
///
/// Oui si, à la fois :
/// - le routeur l'a **accepté sans le livrer** ici (pas `Reject`, pas
///   `Deliver` : ce qui nous est adressé n'est pas à porter pour d'autres) ;
/// - son type se porte en store-and-forward : `SEALED_ENVELOPE`, `NOISE_MSG`,
///   `ACK`. Les autres sont périodiques (`ANNOUNCE`, `LOG_ATTEST`), propres à
///   une session (`NOISE_HS`), à un autre mécanisme (`ENVELOPE_*`,
///   `INVENTORY`) ou réassemblés avant (`FRAGMENT`) ;
/// - l'émetteur autorise encore un saut : [`Flags::RELAY_OK`] et `ttl > 1`.
///   Pousser un paquet à TTL épuisé contournerait la portée voulue par
///   l'émetteur.
///
/// Le TTL rendu est celui du relais programmé, sinon `ttl − 1` : le push
/// est un saut comme un autre, donc la réconciliation de proche en proche
/// reste bornée par le TTL d'origine.
#[must_use]
pub fn cacheable(hdr: &Header, decision: &Decision) -> Option<u8> {
    if !matches!(
        hdr.packet_type,
        PacketType::SealedEnvelope | PacketType::NoiseMsg | PacketType::Ack
    ) {
        return None;
    }
    if !hdr.flags.contains(Flags::RELAY_OK) || hdr.ttl <= 1 {
        return None;
    }
    match *decision {
        Decision::Reject(_) | Decision::Deliver => None,
        Decision::RelayScheduled { ttl, .. } => Some(ttl),
        Decision::Store | Decision::NoRelay(_) => Some(hdr.ttl - 1),
    }
}

// --- Configuration, ordres, compteurs --------------------------------------

/// Paramètres de la réconciliation. [`InventoryConfig::new`] reprend les
/// valeurs par défaut ; les champs publics servent aux tests et à
/// `dengon-sim`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryConfig {
    /// Paquets retenus au plus ([`INVENTORY_CACHE_CAP`]) ; au-delà, le plus
    /// ancien est évincé.
    pub cap: usize,
    /// Rétention d'un paquet depuis sa réception, en ms d'horloge monotone
    /// ([`INVENTORY_WINDOW_MS`]).
    pub window_ms: u64,
    /// Âge maximal d'un paquet d'après son horodatage, en ms d'horloge
    /// murale ([`MSG_TTL_S`]).
    pub msg_ttl_ms: u64,
    /// Paquets poussés par minute et par lien ([`PUSH_MAX_PER_MIN`]).
    pub push_max_per_min: u16,
    /// `msgID` annoncés au plus dans notre `INVENTORY`, les plus récents
    /// d'abord (borné par [`INVENTORY_MAX_IDS`]).
    pub max_ids: usize,
}

impl InventoryConfig {
    /// Configuration par défaut.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cap: INVENTORY_CACHE_CAP,
            window_ms: INVENTORY_WINDOW_MS,
            msg_ttl_ms: u64::from(MSG_TTL_S) * 1_000,
            push_max_per_min: PUSH_MAX_PER_MIN,
            max_ids: INVENTORY_CACHE_CAP,
        }
    }
}

impl Default for InventoryConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Un paquet à pousser maintenant vers un voisin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushOrder<L> {
    /// Le voisin qui ne l'a pas annoncé.
    pub target: L,
    /// Identifiant du paquet.
    pub msg_id: MsgId,
    /// TTL à écrire dans l'en-tête avant émission (octet 2).
    pub ttl: u8,
    /// Octets bruts tels que reçus (signature comprise).
    pub bytes: Vec<u8>,
}

/// Compteurs cumulés depuis la création.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InventoryStats {
    /// Paquets entrés au cache.
    pub remembered: u64,
    /// … évincés parce que le cache était plein.
    pub evicted: u64,
    /// … sortis par expiration (fenêtre ou `MSG_TTL_S`).
    pub expired: u64,
    /// … retirés par [`Inventory::forget`] (ACK).
    pub forgotten: u64,
    /// Inventaires préparés pour un voisin ([`Inventory::link_up`]).
    pub inventories_sent: u64,
    /// Inventaires reçus d'un voisin connu.
    pub inventories_received: u64,
    /// Paquets mis en file de push.
    pub pushes_queued: u64,
    /// Paquets rendus par [`Inventory::poll_push`].
    pub pushes_emitted: u64,
}

// --- La réconciliation -----------------------------------------------------

/// Cache de réconciliation et files de push d'un nœud.
///
/// `L` : identifiant de lien (`dengon_ble::LinkId`), comme pour
/// [`super::routing::Router`].
#[derive(Debug)]
pub struct Inventory<L> {
    cfg: InventoryConfig,
    cache: BTreeMap<MsgId, Entry>,
    /// Ordre d'arrivée : `(réception monotone, n° d'arrivée)` → `msgID`.
    order: BTreeMap<(u64, u64), MsgId>,
    /// N° d'arrivée suivant (départage deux réceptions à la même ms).
    next_seq: u64,
    links: BTreeMap<L, PeerSync>,
    stats: InventoryStats,
}

impl<L: Copy + Ord> Inventory<L> {
    /// Un cache vide, sans voisin.
    #[must_use]
    pub fn new(cfg: InventoryConfig) -> Self {
        Self {
            cfg,
            cache: BTreeMap::new(),
            order: BTreeMap::new(),
            next_seq: 0,
            links: BTreeMap::new(),
            stats: InventoryStats::default(),
        }
    }

    /// La configuration en vigueur.
    #[must_use]
    pub fn config(&self) -> &InventoryConfig {
        &self.cfg
    }

    /// Compteurs cumulés.
    #[must_use]
    pub fn stats(&self) -> InventoryStats {
        self.stats
    }

    /// Nombre de paquets en cache.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// `true` si le cache est vide.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// `true` si `id` est en cache.
    #[must_use]
    pub fn contains(&self, id: &MsgId) -> bool {
        self.cache.contains_key(id)
    }

    /// Les `msgID` en cache, du plus ancien au plus récent.
    #[must_use]
    pub fn ids(&self) -> Vec<MsgId> {
        self.order.values().copied().collect()
    }

    /// Paquets en attente de push vers `link` (0 si lien inconnu).
    #[must_use]
    pub fn queued(&self, link: L) -> usize {
        self.links.get(&link).map_or(0, |p| p.queue.len())
    }

    /// Met en cache un paquet reçu (ou émis localement) pour le pousser aux
    /// voisins qui ne l'ont pas. `ttl` : celui que rend [`cacheable`].
    ///
    /// Rend `false` si le paquet est déjà en cache, déjà trop vieux, ou si
    /// la capacité est nulle. Plein, le cache évince le plus ancien.
    pub fn remember(
        &mut self,
        id: MsgId,
        timestamp_ms: u64,
        ttl: u8,
        bytes: Vec<u8>,
        now: Now,
    ) -> bool {
        self.purge(now);
        if self.cfg.cap == 0 || self.cache.contains_key(&id) || self.too_old(timestamp_ms, now) {
            return false;
        }
        while self.cache.len() >= self.cfg.cap {
            let Some((_, plus_ancien)) = self.order.pop_first() else {
                break;
            };
            self.cache.remove(&plus_ancien);
            self.stats.evicted += 1;
        }
        let cle = (now.mono_ms, self.next_seq);
        self.next_seq += 1;
        self.order.insert(cle, id);
        self.cache.insert(
            id,
            Entry {
                cle,
                timestamp_ms,
                ttl,
                bytes,
            },
        );
        self.stats.remembered += 1;
        true
    }

    /// Retire `id` du cache et de toutes les files de push (ACK vu passer,
    /// message purgé). Rend `true` s'il était en cache.
    pub fn forget(&mut self, id: &MsgId) -> bool {
        for p in self.links.values_mut() {
            if p.queued.remove(id) {
                p.queue.retain(|q| q != id);
            }
        }
        let Some(e) = self.cache.remove(id) else {
            return false;
        };
        self.order.remove(&e.cle);
        self.stats.forgotten += 1;
        true
    }

    /// Un voisin est prêt (après `ANNOUNCE` + handshake) : rend notre
    /// inventaire à lui envoyer — les [`InventoryConfig::max_ids`] `msgID`
    /// les plus **récents**, du plus récent au plus ancien.
    ///
    /// Idempotent sur l'état du lien (sa file de push est conservée).
    pub fn link_up(&mut self, link: L, now: Now) -> Vec<MsgId> {
        self.purge(now);
        self.links.entry(link).or_default();
        self.stats.inventories_sent += 1;
        let max = self.cfg.max_ids.min(INVENTORY_MAX_IDS);
        self.order.values().rev().take(max).copied().collect()
    }

    /// Le voisin est parti : sa file de push est abandonnée.
    pub fn link_down(&mut self, link: L) {
        self.links.remove(&link);
    }

    /// Inventaire reçu de `from` : met en file tout ce qu'on a et qu'il n'a
    /// pas annoncé, du plus ancien au plus récent. Rend le nombre de paquets
    /// ajoutés à la file (0 si le lien est inconnu).
    ///
    /// Repli « file trop grande » (`synthese/05` §6.2) : un inventaire
    /// tronqué par l'émetteur laisse croire qu'il lui manque ses paquets les
    /// plus anciens ; on les pousse quand même, son routeur les rejettera en
    /// `Duplicate` s'il les a encore au seen-set.
    pub fn on_inventory(&mut self, from: L, theirs: &[MsgId], now: Now) -> usize {
        self.purge(now);
        let Some(p) = self.links.get_mut(&from) else {
            return 0;
        };
        self.stats.inventories_received += 1;
        let chez_lui: BTreeSet<&MsgId> = theirs.iter().collect();
        let mut ajoutes = 0;
        for id in self.order.values() {
            if chez_lui.contains(id) || !p.queued.insert(*id) {
                continue;
            }
            p.queue.push_back(*id);
            ajoutes += 1;
        }
        self.stats.pushes_queued += u64::try_from(ajoutes).unwrap_or(u64::MAX);
        ajoutes
    }

    /// Rend les paquets à pousser maintenant, dans la limite de
    /// [`InventoryConfig::push_max_per_min`] par lien sur 60 s (horloge
    /// monotone). Ordre déterministe : liens croissants, puis ordre de file.
    /// Un paquet sorti du cache entre-temps est sauté.
    pub fn poll_push(&mut self, now: Now) -> Vec<PushOrder<L>> {
        self.purge(now);
        let max = self.cfg.push_max_per_min;
        let mut ordres = Vec::new();
        for (&target, p) in &mut self.links {
            while let Some(id) = p.queue.front().copied() {
                let Some(e) = self.cache.get(&id) else {
                    p.queue.pop_front();
                    p.queued.remove(&id);
                    continue;
                };
                if !p.window.try_take(now.mono_ms, PUSH_WINDOW_MS, max) {
                    break;
                }
                p.queue.pop_front();
                p.queued.remove(&id);
                ordres.push(PushOrder {
                    target,
                    msg_id: id,
                    ttl: e.ttl,
                    bytes: e.bytes.clone(),
                });
            }
        }
        self.stats.pushes_emitted += u64::try_from(ordres.len()).unwrap_or(u64::MAX);
        ordres
    }

    /// Prochain instant (horloge monotone) où [`Inventory::poll_push`] aura
    /// quelque chose à rendre ; `None` si toutes les files sont vides. Une
    /// valeur passée veut dire « tout de suite ».
    #[must_use]
    pub fn next_deadline(&self) -> Option<u64> {
        let max = self.cfg.push_max_per_min;
        self.links
            .values()
            .filter(|p| !p.queue.is_empty())
            .map(|p| p.window.next_free(PUSH_WINDOW_MS, max))
            .min()
    }

    fn too_old(&self, timestamp_ms: u64, now: Now) -> bool {
        now.wall_ms.saturating_sub(timestamp_ms) > self.cfg.msg_ttl_ms
    }

    /// Sort du cache ce qui a dépassé la fenêtre de rétention ou
    /// `MSG_TTL_S`. Les files de push s'en aperçoivent paresseusement.
    fn purge(&mut self, now: Now) {
        let fenetre = self.cfg.window_ms;
        let perimes: Vec<MsgId> = self
            .cache
            .iter()
            .filter(|(_, e)| {
                now.mono_ms.saturating_sub(e.cle.0) >= fenetre || self.too_old(e.timestamp_ms, now)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in perimes {
            if let Some(e) = self.cache.remove(&id) {
                self.order.remove(&e.cle);
                self.stats.expired += 1;
            }
        }
    }
}

/// Un paquet en cache.
#[derive(Debug, Clone)]
struct Entry {
    /// Clé dans `Inventory::order` (réception monotone, n° d'arrivée).
    cle: (u64, u64),
    /// Horodatage de l'en-tête (horloge murale de l'émetteur).
    timestamp_ms: u64,
    /// TTL à écrire au push.
    ttl: u8,
    bytes: Vec<u8>,
}

/// État de réconciliation avec un voisin.
#[derive(Debug, Default)]
struct PeerSync {
    /// `msgID` à lui pousser, dans l'ordre.
    queue: VecDeque<MsgId>,
    /// Même contenu que `queue`, pour tester l'appartenance.
    queued: BTreeSet<MsgId>,
    /// Pushs émis sur les 60 dernières secondes.
    window: RateWindow,
}

#[cfg(test)]
mod tests;
