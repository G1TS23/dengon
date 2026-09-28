//! Routage du maillage : flood contrôlé (US-209).
//!
//! Référence : `docs/synthese/05-protocole-et-trame.md` §6.1 (pipeline à la
//! réception), §6.4 (anti-abus) et §7 (comportement MVP) ; `docs/powl/03` §7.1
//! et §8. Tests attendus : `docs/synthese/10-benchmarks-mvp-tests.md` §4.2.
//!
//! # Sans I/O
//!
//! [`Router`] est une **machine à états pure** : on lui présente un en-tête
//! déjà décodé, il rend une [`Decision`] ; on lui demande l'heure venue quels
//! relais émettre ([`Router::poll_due`]), il rend des [`RelayOrder`]. Il
//! n'envoie rien lui-même.
//!
//! Ce n'est pas qu'une question de goût : `dengon-ble` (le trait `Transport`)
//! dépend de `dengon-core`, et utilise `std`. Le routeur ne peut donc pas
//! l'importer — il est **générique** sur l'identifiant de lien `L`, que
//! l'appelant instancie avec `dengon_ble::LinkId`. Même principe pour le
//! temps (`now_ms` passé en argument) et l'aléa (graine au constructeur) :
//! c'est ce qui rend le module `no_std` **et** déterministe à graine fixe.
//!
//! # Pipeline (`synthese/05` §6.1)
//!
//! ```text
//! version / cohérence flags          → Reject(BadVersion | Malformed)
//! lien connu ?                       → Reject(UnknownLink)
//! horodatage > now + 2 h             → Reject(ClockSkew)
//! horodatage < now − MSG_TTL_S       → Reject(Expired)
//! quota du lien (paquets / s)        → Reject(LinkQuota)
//! msgID déjà vu                      → Reject(Duplicate)  (+ annule le relais en
//!                                      attente au DUP_CANCEL_THRESHOLDᵉ doublon)
//! > FLOOD_MAX_PER_MIN_PEER nouveaux  → Reject(FloodLimited)
//! seen-set.insert(msgID)
//! adressé à moi                      → Deliver
//! pas RELAY_OK / ttl ≤ 1             → Store (enveloppe) | NoRelay
//! ttl' = min(ttl − 1, clamp densité, clamp broadcast)
//! jitter RELAY_JITTER_MS             → RelayScheduled
//! ```
//!
//! # Préconditions (hors de ce module)
//!
//! - **Signature** : vérifiée **en amont** par l'appelant (`crypto` n'est pas
//!   encore sur `main`). Un paquet dont la signature est fausse ne doit pas
//!   arriver ici, sinon il pollue le seen-set.
//! - **`msgID`** : calculé par l'appelant (`SHA-256(sender ‖ ts ‖ type ‖
//!   payload)`, A-9). Le routeur le prend tel quel.
//! - **Fragments** : le réassemblage L2 (US-202) a lieu avant ou après,
//!   au choix de l'appelant ; le routeur traite un `Fragment` comme un paquet.
//! - **ACK qui purge une file** : un `ACK` est chiffré dans Noise, le routeur
//!   ne voit pas quel message il acquitte. `status` / `courier` (US-211/212)
//!   appellent [`Router::cancel`] quand ils l'apprennent.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;
use core::ops::RangeInclusive;

use crate::protocol::consts::{
    DENSE_LINKS, FLOOD_MAX_PER_MIN_PEER, MSG_TTL_S, PROTO_VERSION, RELAY_JITTER_MS, SEEN_SET_CAP,
    SEEN_TTL_S, TIMESTAMP_TOLERANCE_MS, TTL_CLAMP_DENSE,
};
use crate::protocol::{Flags, Header, MsgId, PacketType, PeerId};

/// Quota par lien : paquets acceptés par seconde, **doublons compris**.
///
/// `synthese/05` §6.4 demande un « quota par `peerID` et par lien
/// (paquets/s) » sans en donner la valeur, et `protocol::consts` n'a pas de
/// constante correspondante. 50 paquets/s ≈ 22 Ko/s à 440 o par fragment,
/// au-dessus du débit utile réaliste d'un lien BLE partagé : un pair honnête
/// ne l'atteint pas. Valeur de [`RoutingConfig`], pas du contrat — écart
/// consigné dans `docs/suivi/03-ecarts-conception.md`.
pub const LINK_MAX_PKT_PER_S: u16 = 50;

/// TTL maximal relayé pour le trafic **broadcast** (`ANNOUNCE`, `LOG_ATTEST`).
///
/// `synthese/05` §6.1 : « Broadcast : idem mais TTL faible (2–3) ». Borne
/// haute retenue, appliquée au relais (en plus du clamp de densité).
pub const BROADCAST_TTL_MAX: u8 = 3;

/// Nombre de doublons entendus pendant le jitter qui annulent un relais.
///
/// `synthese/05` §6.1 dit « reçu en double pendant l'attente → abandonner »,
/// soit un seuil de **1**. Le test `plusieurs_chemins_un_seul_relais_par_noeud`
/// (`tests/routing_mock.rs`) montre que ce seuil **affame** un losange
/// A→{B,C}→D→E : D entend la copie de C pendant son jitter, s'abstient, et E
/// ne reçoit jamais rien. Seuil 2 (schéma « à compteur », Ni et al. 1999) :
/// un seul chemin redondant ne suffit plus à supprimer le relais. Écart
/// consigné dans `docs/suivi/03-ecarts-conception.md`.
pub const DUP_CANCEL_THRESHOLD: u8 = 2;

/// Fenêtre glissante de l'anti-inondation, en ms (« par minute »).
const FLOOD_WINDOW_MS: u64 = 60_000;
/// Fenêtre glissante du quota par lien, en ms (« par seconde »).
const LINK_WINDOW_MS: u64 = 1_000;

/// Paramètres du routeur. [`RoutingConfig::new`] reprend les valeurs de
/// `protocol::consts` ; les champs publics permettent aux tests (et à
/// `dengon-sim`) d'explorer d'autres réglages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingConfig {
    /// `peerID` du nœud local : un paquet adressé à lui est livré, pas relayé.
    pub local_id: PeerId,
    /// Capacité du seen-set ([`SEEN_SET_CAP`]).
    pub seen_cap: usize,
    /// Durée de rétention d'un `msgID` dans le seen-set, en ms ([`SEEN_TTL_S`]).
    pub seen_ttl_ms: u64,
    /// Durée de vie applicative d'un message, en ms ([`MSG_TTL_S`]).
    pub msg_ttl_ms: u64,
    /// Nouveaux `msgID` acceptés par voisin sur 60 s ([`FLOOD_MAX_PER_MIN_PEER`]).
    pub flood_max_per_min: u16,
    /// Paquets acceptés par lien sur 1 s ([`LINK_MAX_PKT_PER_S`]).
    pub link_max_pkt_per_s: u16,
    /// Plafond de TTL relayé pour `ANNOUNCE` / `LOG_ATTEST` ([`BROADCAST_TTL_MAX`]).
    pub broadcast_ttl_max: u8,
    /// Nombre de voisins à partir duquel le clamp s'applique ([`DENSE_LINKS`]).
    pub dense_links: u8,
    /// TTL maximal relayé en zone dense ([`TTL_CLAMP_DENSE`]).
    pub ttl_clamp_dense: u8,
    /// Fenêtre du délai aléatoire avant relais, en ms ([`RELAY_JITTER_MS`]).
    pub jitter_ms: RangeInclusive<u16>,
    /// Doublons entendus pendant le jitter qui annulent le relais
    /// ([`DUP_CANCEL_THRESHOLD`]) ; `1` = règle littérale de `synthese/05`,
    /// `0` = ne jamais annuler.
    pub dup_cancel_threshold: u8,
}

impl RoutingConfig {
    /// Configuration par défaut du protocole pour le nœud `local_id`.
    #[must_use]
    pub fn new(local_id: PeerId) -> Self {
        Self {
            local_id,
            seen_cap: SEEN_SET_CAP,
            seen_ttl_ms: u64::from(SEEN_TTL_S) * 1_000,
            msg_ttl_ms: u64::from(MSG_TTL_S) * 1_000,
            flood_max_per_min: FLOOD_MAX_PER_MIN_PEER,
            link_max_pkt_per_s: LINK_MAX_PKT_PER_S,
            broadcast_ttl_max: BROADCAST_TTL_MAX,
            dense_links: DENSE_LINKS,
            ttl_clamp_dense: TTL_CLAMP_DENSE,
            jitter_ms: RELAY_JITTER_MS,
            dup_cancel_threshold: DUP_CANCEL_THRESHOLD,
        }
    }
}

/// Pourquoi un paquet est **rejeté** : l'appelant le jette sans le traiter.
///
/// Sert tel quel au futur événement `pkt.rejected` (observabilité, US-208).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RejectReason {
    /// `version` différente de [`PROTO_VERSION`].
    BadVersion,
    /// `recipient_id` et [`Flags::ADDRESSED`] se contredisent.
    Malformed,
    /// Paquet reçu d'un lien jamais annoncé par [`Router::link_up`].
    UnknownLink,
    /// Horodatage dans le futur au-delà de [`TIMESTAMP_TOLERANCE_MS`].
    ClockSkew,
    /// Message plus vieux que [`RoutingConfig::msg_ttl_ms`].
    Expired,
    /// Le lien a dépassé son quota de paquets par seconde.
    LinkQuota,
    /// `msgID` déjà vu : doublon.
    Duplicate,
    /// Le voisin a dépassé son quota de nouveaux `msgID` par minute.
    FloodLimited,
}

/// Pourquoi un paquet **accepté** (nouveau) n'est pas relayé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NoRelayReason {
    /// TTL épuisé (`ttl ≤ 1`, ou ramené à 0 par un clamp).
    TtlExhausted,
    /// [`Flags::RELAY_OK`] absent.
    RelayNotAllowed,
}

/// Verdict du routeur sur un paquet reçu.
///
/// Toute variante autre que [`Decision::Reject`] signifie « paquet **nouveau**,
/// déjà noté dans le seen-set » : l'appelant le traite localement s'il le
/// concerne (un `ANNOUNCE` par exemple), quelle que soit la décision de relais.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// À jeter.
    Reject(RejectReason),
    /// Adressé au nœud local : à traiter ici, jamais relayé.
    Deliver,
    /// Enveloppe scellée qu'on ne relaie pas : à **déposer** (collecte
    /// ultérieure par le destinataire, `sync::courier`).
    Store,
    /// Accepté, mais pas relayé.
    NoRelay(NoRelayReason),
    /// Relais programmé : [`Router::poll_due`] le rendra à `at_ms`, avec ce
    /// `ttl`, sauf doublon reçu entre-temps.
    RelayScheduled {
        /// Échéance du relais (heure de réception + jitter), en ms.
        at_ms: u64,
        /// TTL à écrire dans le paquet relayé.
        ttl: u8,
    },
}

/// Un relais à émettre maintenant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayOrder<L> {
    /// Le paquet à relayer.
    pub msg_id: MsgId,
    /// TTL à écrire dans l'en-tête avant émission.
    pub ttl: u8,
    /// Tous les voisins **sauf** celui d'où vient le paquet, en ordre croissant.
    pub targets: Vec<L>,
}

/// Compteurs cumulés depuis la création du routeur.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RoutingStats {
    /// Paquets présentés à [`Router::on_packet`].
    pub received: u64,
    /// Paquets rejetés, toutes raisons confondues.
    pub rejected: u64,
    /// … dont doublons.
    pub duplicates: u64,
    /// … dont anti-inondation.
    pub flood_limited: u64,
    /// … dont quota de lien.
    pub link_quota: u64,
    /// Paquets livrés localement.
    pub delivered: u64,
    /// Enveloppes à déposer.
    pub stored: u64,
    /// Paquets acceptés non relayés.
    pub not_relayed: u64,
    /// Relais programmés.
    pub relays_scheduled: u64,
    /// Relais annulés (doublon pendant le jitter, ou [`Router::cancel`]).
    pub relays_cancelled: u64,
    /// Relais rendus par [`Router::poll_due`] avec au moins une cible.
    pub relays_emitted: u64,
    /// Relais arrivés à échéance sans aucun voisin à qui les envoyer.
    pub relays_without_target: u64,
}

/// Le routeur d'un nœud.
///
/// `L` est l'identifiant de lien de la plateforme (`dengon_ble::LinkId`) ;
/// `Ord` parce qu'en `no_std` on n'a que des `BTreeMap`, et parce que l'ordre
/// fixe des cibles participe au déterminisme.
#[derive(Debug)]
pub struct Router<L> {
    cfg: RoutingConfig,
    links: BTreeMap<L, LinkState>,
    seen: SeenSet,
    pending: BTreeMap<MsgId, Pending<L>>,
    rng: SplitMix64,
    stats: RoutingStats,
}

impl<L: Copy + Ord> Router<L> {
    /// Un routeur sans voisin. `seed` fixe entièrement la suite des jitters.
    #[must_use]
    pub fn new(cfg: RoutingConfig, seed: u64) -> Self {
        Self {
            seen: SeenSet::new(cfg.seen_cap, cfg.seen_ttl_ms),
            cfg,
            links: BTreeMap::new(),
            pending: BTreeMap::new(),
            rng: SplitMix64(seed),
            stats: RoutingStats::default(),
        }
    }

    /// Un voisin est connecté (`TransportEvent::PeerConnected`). Idempotent.
    pub fn link_up(&mut self, link: L) {
        self.links.entry(link).or_default();
    }

    /// Un voisin est parti (`TransportEvent::PeerDisconnected`).
    ///
    /// Les relais en attente dont il est la **source** sont conservés : le
    /// paquet reste valable pour les autres voisins.
    pub fn link_down(&mut self, link: L) {
        self.links.remove(&link);
    }

    /// Nombre de voisins connectés (la « densité » du clamp).
    #[must_use]
    pub fn neighbours(&self) -> usize {
        self.links.len()
    }

    /// La configuration en vigueur.
    #[must_use]
    pub fn config(&self) -> &RoutingConfig {
        &self.cfg
    }

    /// Compteurs cumulés.
    #[must_use]
    pub fn stats(&self) -> RoutingStats {
        self.stats
    }

    /// Nombre de `msgID` actuellement retenus dans le seen-set.
    #[must_use]
    pub fn seen_len(&self) -> usize {
        self.seen.len()
    }

    /// Nombre de relais en attente de leur jitter.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Échéance du prochain relais en attente, pour que la boucle d'événements
    /// de l'appelant sache quand rappeler [`Router::poll_due`].
    #[must_use]
    pub fn next_deadline(&self) -> Option<u64> {
        self.pending.values().map(|p| p.deadline).min()
    }

    /// Annule le relais en attente de `msg_id`, s'il y en a un (ACK vu passer,
    /// message purgé par `status` / `courier`). Rend `true` si un relais a été
    /// annulé.
    pub fn cancel(&mut self, msg_id: &MsgId) -> bool {
        let annule = self.pending.remove(msg_id).is_some();
        if annule {
            self.stats.relays_cancelled += 1;
        }
        annule
    }

    /// Passe un paquet reçu de `from` dans le pipeline (voir la doc du module).
    ///
    /// `now_ms` : horloge murale du nœud, ms UTC (même référence que
    /// `Header::timestamp_ms`).
    pub fn on_packet(&mut self, from: L, hdr: &Header, msg_id: &MsgId, now_ms: u64) -> Decision {
        self.stats.received += 1;
        let decision = self.decide(from, hdr, msg_id, now_ms);
        match decision {
            Decision::Reject(motif) => {
                self.stats.rejected += 1;
                match motif {
                    RejectReason::Duplicate => self.stats.duplicates += 1,
                    RejectReason::FloodLimited => self.stats.flood_limited += 1,
                    RejectReason::LinkQuota => self.stats.link_quota += 1,
                    _ => {}
                }
            }
            Decision::Deliver => self.stats.delivered += 1,
            Decision::Store => self.stats.stored += 1,
            Decision::NoRelay(_) => self.stats.not_relayed += 1,
            Decision::RelayScheduled { .. } => self.stats.relays_scheduled += 1,
        }
        decision
    }

    /// Rend les relais dont le jitter est écoulé à `now_ms`, par échéance
    /// croissante puis `msgID` croissant (ordre déterministe).
    ///
    /// Les cibles sont calculées **maintenant** : un voisin arrivé pendant le
    /// jitter est servi, un voisin parti ne l'est pas. Un relais sans aucune
    /// cible est abandonné (compté dans [`RoutingStats::relays_without_target`]).
    pub fn poll_due(&mut self, now_ms: u64) -> Vec<RelayOrder<L>> {
        let mut echus: Vec<(u64, MsgId)> = self
            .pending
            .iter()
            .filter(|(_, p)| p.deadline <= now_ms)
            .map(|(id, p)| (p.deadline, *id))
            .collect();
        echus.sort_unstable();

        let mut ordres = Vec::with_capacity(echus.len());
        for (_, msg_id) in echus {
            let Some(p) = self.pending.remove(&msg_id) else {
                continue;
            };
            let targets: Vec<L> = self
                .links
                .keys()
                .copied()
                .filter(|l| *l != p.source)
                .collect();
            if targets.is_empty() {
                self.stats.relays_without_target += 1;
                continue;
            }
            self.stats.relays_emitted += 1;
            ordres.push(RelayOrder {
                msg_id,
                ttl: p.ttl,
                targets,
            });
        }
        ordres
    }

    fn decide(&mut self, from: L, hdr: &Header, msg_id: &MsgId, now_ms: u64) -> Decision {
        use Decision::Reject;

        if hdr.version != PROTO_VERSION {
            return Reject(RejectReason::BadVersion);
        }
        if !hdr.flags_are_consistent() {
            return Reject(RejectReason::Malformed);
        }
        let Some(lien) = self.links.get_mut(&from) else {
            return Reject(RejectReason::UnknownLink);
        };

        // Fraîcheur. La tolérance ±2 h de `synthese/05` §6.4 ne vaut que vers
        // le futur : vers le passé, un message porté en store-and-forward a
        // légitimement jusqu'à MSG_TTL_S (24 h) — voir 03-ecarts-conception.
        if hdr.timestamp_ms > now_ms.saturating_add(TIMESTAMP_TOLERANCE_MS) {
            return Reject(RejectReason::ClockSkew);
        }
        if now_ms.saturating_sub(hdr.timestamp_ms) > self.cfg.msg_ttl_ms {
            return Reject(RejectReason::Expired);
        }

        // Quota brut du lien : compte aussi les doublons, sinon un pair
        // pourrait saturer le nœud en rejouant un seul msgID.
        if !lien
            .packets
            .try_take(now_ms, LINK_WINDOW_MS, self.cfg.link_max_pkt_per_s)
        {
            return Reject(RejectReason::LinkQuota);
        }

        self.seen.expire(now_ms);
        if self.seen.contains(msg_id) {
            // « Écouter avant de rediffuser » : assez de voisins l'ont déjà
            // rediffusé pendant notre jitter, on s'abstient.
            let seuil = self.cfg.dup_cancel_threshold;
            if let Some(p) = self.pending.get_mut(msg_id) {
                p.dups = p.dups.saturating_add(1);
                if seuil > 0 && p.dups >= seuil {
                    self.pending.remove(msg_id);
                    self.stats.relays_cancelled += 1;
                }
            }
            return Reject(RejectReason::Duplicate);
        }

        // Anti-inondation : seuls les msgID *nouveaux* comptent. Un msgID
        // refusé ici n'entre pas au seen-set — un voisin honnête pourra le
        // relivrer plus tard.
        if !lien
            .new_ids
            .try_take(now_ms, FLOOD_WINDOW_MS, self.cfg.flood_max_per_min)
        {
            return Reject(RejectReason::FloodLimited);
        }

        self.seen.insert(*msg_id, now_ms);

        if hdr.recipient_id == Some(self.cfg.local_id) {
            return Decision::Deliver;
        }

        let pas_de_relais = |motif| {
            if hdr.packet_type == PacketType::SealedEnvelope {
                Decision::Store
            } else {
                Decision::NoRelay(motif)
            }
        };
        if !hdr.flags.contains(Flags::RELAY_OK) {
            return pas_de_relais(NoRelayReason::RelayNotAllowed);
        }
        if hdr.ttl <= 1 {
            return pas_de_relais(NoRelayReason::TtlExhausted);
        }

        let mut ttl = hdr.ttl - 1;
        if self.links.len() >= usize::from(self.cfg.dense_links) {
            ttl = ttl.min(self.cfg.ttl_clamp_dense);
        }
        if matches!(
            hdr.packet_type,
            PacketType::Announce | PacketType::LogAttest
        ) {
            ttl = ttl.min(self.cfg.broadcast_ttl_max);
        }
        if ttl == 0 {
            return pas_de_relais(NoRelayReason::TtlExhausted);
        }

        let at_ms = now_ms.saturating_add(self.draw_jitter());
        self.pending.insert(
            *msg_id,
            Pending {
                deadline: at_ms,
                ttl,
                source: from,
                dups: 0,
            },
        );
        Decision::RelayScheduled { at_ms, ttl }
    }

    fn draw_jitter(&mut self) -> u64 {
        let debut = u64::from(*self.cfg.jitter_ms.start());
        let fin = u64::from(*self.cfg.jitter_ms.end());
        if fin <= debut {
            return debut;
        }
        debut + self.rng.next_u64() % (fin - debut + 1)
    }
}

/// Un relais qui attend la fin de son jitter.
#[derive(Debug, Clone, Copy)]
struct Pending<L> {
    deadline: u64,
    ttl: u8,
    source: L,
    /// Doublons entendus depuis la programmation.
    dups: u8,
}

/// État par voisin : deux fenêtres glissantes.
#[derive(Debug, Default)]
struct LinkState {
    /// Tous les paquets reçus (quota par lien, 1 s).
    packets: RateWindow,
    /// Nouveaux msgID acceptés (anti-inondation, 60 s).
    new_ids: RateWindow,
}

/// Fenêtre glissante : horodatages des événements acceptés.
///
/// Ne retient que les événements **acceptés**, donc sa taille est bornée par
/// `max` : pas de croissance mémoire sous flood.
#[derive(Debug, Default)]
struct RateWindow(VecDeque<u64>);

impl RateWindow {
    /// Accepte un événement à `now` s'il en reste moins de `max` dans la
    /// fenêtre `[now − window, now]`.
    fn try_take(&mut self, now: u64, window: u64, max: u16) -> bool {
        while self
            .0
            .front()
            .is_some_and(|&t| now.saturating_sub(t) >= window)
        {
            self.0.pop_front();
        }
        if self.0.len() >= usize::from(max) {
            return false;
        }
        self.0.push_back(now);
        true
    }
}

/// Seen-set borné : `msgID` → heure d'insertion, éviction du plus ancien.
///
/// Un `msgID` n'est inséré qu'une fois (on ne rafraîchit pas un doublon), donc
/// l'ordre d'insertion est aussi l'ordre d'âge : la file suffit, pas besoin de
/// LRU plus fin.
#[derive(Debug)]
struct SeenSet {
    cap: usize,
    ttl_ms: u64,
    by_id: BTreeMap<MsgId, u64>,
    order: VecDeque<(u64, MsgId)>,
}

impl SeenSet {
    fn new(cap: usize, ttl_ms: u64) -> Self {
        Self {
            cap,
            ttl_ms,
            by_id: BTreeMap::new(),
            order: VecDeque::new(),
        }
    }

    fn len(&self) -> usize {
        self.by_id.len()
    }

    fn contains(&self, id: &MsgId) -> bool {
        self.by_id.contains_key(id)
    }

    /// Oublie les entrées plus vieilles que `ttl_ms`.
    fn expire(&mut self, now: u64) {
        while let Some(&(t, id)) = self.order.front() {
            if now.saturating_sub(t) < self.ttl_ms {
                break;
            }
            self.order.pop_front();
            self.by_id.remove(&id);
        }
    }

    fn insert(&mut self, id: MsgId, now: u64) {
        if self.cap == 0 {
            return;
        }
        while self.order.len() >= self.cap {
            if let Some((_, ancien)) = self.order.pop_front() {
                self.by_id.remove(&ancien);
            }
        }
        self.order.push_back((now, id));
        self.by_id.insert(id, now);
    }
}

/// PRNG SplitMix64 (Steele, Lea, Flood 2014) : 64 bits d'état, `no_std`, sans
/// dépendance. Suffisant pour un jitter ; **jamais** pour de la crypto.
#[derive(Debug, Clone)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::TTL_DEFAULT;
    use proptest::prelude::*;

    const MOI: PeerId = [0xAA; 8];
    const AUTRE: PeerId = [0xBB; 8];
    const T0: u64 = 1_800_000_000_000;

    fn id(n: u32) -> MsgId {
        let mut m = [0u8; 32];
        m[..4].copy_from_slice(&n.to_be_bytes());
        m
    }

    fn entete(ttl: u8) -> Header {
        Header {
            version: PROTO_VERSION,
            packet_type: PacketType::NoiseMsg,
            ttl,
            flags: Flags::RELAY_OK,
            timestamp_ms: T0,
            sender_id: [0x11; 8],
            recipient_id: None,
            payload_len: 0,
        }
    }

    fn adresse(ttl: u8, dest: PeerId) -> Header {
        Header {
            recipient_id: Some(dest),
            flags: Flags::RELAY_OK | Flags::ADDRESSED,
            ..entete(ttl)
        }
    }

    /// Routeur à `n` voisins, liens 0..n.
    fn routeur(n: u32) -> Router<u32> {
        let mut r = Router::new(RoutingConfig::new(MOI), 42);
        for l in 0..n {
            r.link_up(l);
        }
        r
    }

    fn ttl_programme(d: Decision) -> u8 {
        match d {
            Decision::RelayScheduled { ttl, .. } => ttl,
            autre => panic!("relais attendu, obtenu {autre:?}"),
        }
    }

    // --- dédup & jitter ------------------------------------------------

    #[test]
    fn meme_msg_id_deux_fois_donne_un_seul_relais() {
        let mut r = routeur(3);
        let h = entete(TTL_DEFAULT);
        assert!(matches!(
            r.on_packet(0, &h, &id(1), T0),
            Decision::RelayScheduled { .. }
        ));
        // Le doublon arrive APRÈS l'émission : déjà vu, rien de plus.
        let ordres = r.poll_due(T0 + 1_000);
        assert_eq!(ordres.len(), 1);
        assert_eq!(
            r.on_packet(1, &h, &id(1), T0 + 1_001),
            Decision::Reject(RejectReason::Duplicate)
        );
        assert!(r.poll_due(T0 + 5_000).is_empty());
        assert_eq!(r.stats().relays_emitted, 1);
    }

    #[test]
    fn deux_doublons_pendant_le_jitter_annulent_le_relais() {
        let mut r = routeur(3);
        let h = entete(TTL_DEFAULT);
        let Decision::RelayScheduled { at_ms, .. } = r.on_packet(0, &h, &id(1), T0) else {
            panic!("relais attendu");
        };
        assert_eq!(r.next_deadline(), Some(at_ms));
        for (lien, t) in [(1, T0 + 1), (2, T0 + 2)] {
            assert_eq!(
                r.on_packet(lien, &h, &id(1), t),
                Decision::Reject(RejectReason::Duplicate)
            );
        }
        assert!(r.poll_due(at_ms).is_empty());
        assert_eq!(r.stats().relays_cancelled, 1);
        assert_eq!(r.next_deadline(), None);
    }

    #[test]
    fn un_seul_doublon_ne_suffit_pas_par_defaut() {
        let mut r = routeur(3);
        let h = entete(TTL_DEFAULT);
        r.on_packet(0, &h, &id(1), T0);
        r.on_packet(1, &h, &id(1), T0 + 1);
        assert_eq!(r.poll_due(T0 + 1_000).len(), 1);
    }

    #[test]
    fn seuil_litteral_de_la_conception_et_seuil_nul() {
        let h = entete(TTL_DEFAULT);
        for (seuil, relais_attendus) in [(1, 0), (0, 1)] {
            let mut cfg = RoutingConfig::new(MOI);
            cfg.dup_cancel_threshold = seuil;
            let mut r = Router::new(cfg, 1);
            for l in 0..3u32 {
                r.link_up(l);
            }
            r.on_packet(0, &h, &id(1), T0);
            for l in 1..3 {
                r.on_packet(l, &h, &id(1), T0 + 1);
            }
            assert_eq!(
                r.poll_due(T0 + 1_000).len(),
                relais_attendus,
                "seuil {seuil}"
            );
        }
    }

    #[test]
    fn le_relais_n_est_rendu_qu_a_echeance_et_exclut_la_source() {
        let mut r = routeur(4);
        let Decision::RelayScheduled { at_ms, ttl } =
            r.on_packet(2, &entete(TTL_DEFAULT), &id(1), T0)
        else {
            panic!("relais attendu");
        };
        assert!((T0 + 10..=T0 + 220).contains(&at_ms));
        assert!(r.poll_due(at_ms - 1).is_empty());
        let ordres = r.poll_due(at_ms);
        assert_eq!(
            ordres,
            [RelayOrder {
                msg_id: id(1),
                ttl,
                targets: vec![0, 1, 3],
            }]
        );
    }

    #[test]
    fn cancel_retire_un_relais_en_attente() {
        let mut r = routeur(2);
        r.on_packet(0, &entete(TTL_DEFAULT), &id(7), T0);
        assert!(r.cancel(&id(7)));
        assert!(!r.cancel(&id(7)));
        assert!(r.poll_due(T0 + 1_000).is_empty());
    }

    // --- TTL & clamp ------------------------------------------------------

    #[test]
    fn le_ttl_est_decremente() {
        let mut r = routeur(2);
        assert_eq!(ttl_programme(r.on_packet(0, &entete(4), &id(1), T0)), 3);
        assert_eq!(ttl_programme(r.on_packet(0, &entete(2), &id(2), T0)), 1);
    }

    #[test]
    fn ttl_un_ou_zero_n_est_pas_relaye() {
        let mut r = routeur(2);
        for (n, ttl) in [(1, 1), (2, 0)] {
            assert_eq!(
                r.on_packet(0, &entete(ttl), &id(n), T0),
                Decision::NoRelay(NoRelayReason::TtlExhausted)
            );
        }
        assert_eq!(r.pending_len(), 0);
    }

    #[test]
    fn clamp_de_densite_a_six_voisins() {
        let mut dense = routeur(6);
        assert_eq!(
            ttl_programme(dense.on_packet(0, &entete(7), &id(1), T0)),
            TTL_CLAMP_DENSE
        );
        let mut clair = routeur(5);
        assert_eq!(ttl_programme(clair.on_packet(0, &entete(7), &id(1), T0)), 6);
    }

    #[test]
    fn le_broadcast_est_relaye_a_ttl_faible() {
        let mut r = routeur(2);
        let h = Header {
            packet_type: PacketType::Announce,
            ..entete(7)
        };
        assert_eq!(
            ttl_programme(r.on_packet(0, &h, &id(1), T0)),
            BROADCAST_TTL_MAX
        );
    }

    #[test]
    fn un_clamp_a_zero_empeche_le_relais() {
        let mut cfg = RoutingConfig::new(MOI);
        cfg.broadcast_ttl_max = 0;
        let mut r = Router::new(cfg, 1);
        r.link_up(0u32);
        let h = Header {
            packet_type: PacketType::LogAttest,
            ..entete(7)
        };
        assert_eq!(
            r.on_packet(0, &h, &id(1), T0),
            Decision::NoRelay(NoRelayReason::TtlExhausted)
        );
    }

    #[test]
    fn sans_relay_ok_pas_de_relais() {
        let mut r = routeur(2);
        let h = Header {
            flags: Flags::empty(),
            ..entete(7)
        };
        assert_eq!(
            r.on_packet(0, &h, &id(1), T0),
            Decision::NoRelay(NoRelayReason::RelayNotAllowed)
        );
    }

    #[test]
    fn une_enveloppe_non_relayable_est_deposee() {
        let mut r = routeur(2);
        let mut h = Header {
            packet_type: PacketType::SealedEnvelope,
            ..entete(1)
        };
        assert_eq!(r.on_packet(0, &h, &id(1), T0), Decision::Store);
        h.flags = Flags::empty();
        h.ttl = 7;
        assert_eq!(r.on_packet(0, &h, &id(2), T0), Decision::Store);
        assert_eq!(r.stats().stored, 2);
    }

    // --- livraison locale -------------------------------------------------

    #[test]
    fn un_paquet_pour_moi_est_livre_et_pas_relaye() {
        let mut r = routeur(3);
        assert_eq!(
            r.on_packet(0, &adresse(7, MOI), &id(1), T0),
            Decision::Deliver
        );
        assert_eq!(r.pending_len(), 0);
        assert!(matches!(
            r.on_packet(0, &adresse(7, AUTRE), &id(2), T0),
            Decision::RelayScheduled { .. }
        ));
    }

    // --- rejets -------------------------------------------------------------

    #[test]
    fn version_inconnue_rejetee() {
        let mut r = routeur(1);
        let h = Header {
            version: 2,
            ..entete(7)
        };
        assert_eq!(
            r.on_packet(0, &h, &id(1), T0),
            Decision::Reject(RejectReason::BadVersion)
        );
    }

    #[test]
    fn drapeaux_incoherents_rejetes() {
        let mut r = routeur(1);
        let h = Header {
            recipient_id: Some(AUTRE),
            ..entete(7)
        };
        assert_eq!(
            r.on_packet(0, &h, &id(1), T0),
            Decision::Reject(RejectReason::Malformed)
        );
    }

    #[test]
    fn lien_inconnu_rejete() {
        let mut r = routeur(1);
        assert_eq!(
            r.on_packet(9, &entete(7), &id(1), T0),
            Decision::Reject(RejectReason::UnknownLink)
        );
    }

    #[test]
    fn horloge_trop_en_avance_rejetee() {
        let mut r = routeur(1);
        let h = Header {
            timestamp_ms: T0 + TIMESTAMP_TOLERANCE_MS + 1,
            ..entete(7)
        };
        assert_eq!(
            r.on_packet(0, &h, &id(1), T0),
            Decision::Reject(RejectReason::ClockSkew)
        );
        let h = Header {
            timestamp_ms: T0 + TIMESTAMP_TOLERANCE_MS,
            ..entete(7)
        };
        assert!(matches!(
            r.on_packet(0, &h, &id(2), T0),
            Decision::RelayScheduled { .. }
        ));
    }

    #[test]
    fn message_expire_rejete_mais_vieux_de_23_h_accepte() {
        let mut r = routeur(2);
        let jour = u64::from(MSG_TTL_S) * 1_000;
        assert_eq!(
            r.on_packet(0, &entete(7), &id(1), T0 + jour + 1),
            Decision::Reject(RejectReason::Expired)
        );
        assert!(matches!(
            r.on_packet(0, &entete(7), &id(2), T0 + 23 * 3_600_000),
            Decision::RelayScheduled { .. }
        ));
    }

    // --- anti-inondation & quotas ----------------------------------------

    #[test]
    fn anti_inondation_vingt_et_unieme_msg_id_refuse_puis_accepte_apres_60_s() {
        let mut r = routeur(3);
        let max = u32::from(FLOOD_MAX_PER_MIN_PEER);
        // Un paquet toutes les 100 ms : sous le quota de lien.
        for n in 0..max {
            let d = r.on_packet(0, &entete(7), &id(n), T0 + u64::from(n) * 100);
            assert!(matches!(d, Decision::RelayScheduled { .. }), "n={n}");
        }
        assert_eq!(
            r.on_packet(0, &entete(7), &id(max), T0 + 2_100),
            Decision::Reject(RejectReason::FloodLimited)
        );
        // Un autre voisin n'est pas pénalisé.
        assert!(matches!(
            r.on_packet(1, &entete(7), &id(max), T0 + 2_200),
            Decision::RelayScheduled { .. }
        ));
        // 60 s après le premier, une place se libère.
        assert!(matches!(
            r.on_packet(0, &entete(7), &id(max + 1), T0 + 60_000),
            Decision::RelayScheduled { .. }
        ));
    }

    #[test]
    fn un_msg_id_refuse_par_l_anti_inondation_reste_acceptable() {
        let mut cfg = RoutingConfig::new(MOI);
        cfg.flood_max_per_min = 1;
        let mut r = Router::new(cfg, 1);
        r.link_up(0u32);
        r.link_up(1u32);
        r.on_packet(0, &entete(7), &id(1), T0);
        assert_eq!(
            r.on_packet(0, &entete(7), &id(2), T0 + 10),
            Decision::Reject(RejectReason::FloodLimited)
        );
        // id(2) n'a pas pollué le seen-set : un voisin honnête le fait passer.
        assert!(matches!(
            r.on_packet(1, &entete(7), &id(2), T0 + 20),
            Decision::RelayScheduled { .. }
        ));
    }

    #[test]
    fn quota_de_lien_compte_aussi_les_doublons() {
        let mut r = routeur(2);
        let max = u32::from(LINK_MAX_PKT_PER_S);
        r.on_packet(0, &entete(7), &id(1), T0);
        for _ in 1..max {
            assert_eq!(
                r.on_packet(0, &entete(7), &id(1), T0 + 1),
                Decision::Reject(RejectReason::Duplicate)
            );
        }
        assert_eq!(
            r.on_packet(0, &entete(7), &id(2), T0 + 2),
            Decision::Reject(RejectReason::LinkQuota)
        );
        assert!(matches!(
            r.on_packet(0, &entete(7), &id(2), T0 + 1_000),
            Decision::RelayScheduled { .. }
        ));
        assert_eq!(r.stats().link_quota, 1);
    }

    // --- seen-set -------------------------------------------------------

    #[test]
    fn seen_set_borne_a_sa_capacite() {
        let mut cfg = RoutingConfig::new(MOI);
        cfg.seen_cap = 4;
        cfg.flood_max_per_min = u16::MAX;
        cfg.link_max_pkt_per_s = u16::MAX;
        let mut r = Router::new(cfg, 1);
        r.link_up(0u32);
        for n in 0..10 {
            r.on_packet(0, &entete(7), &id(n), T0);
        }
        assert_eq!(r.seen_len(), 4);
        // id(0) a été évincé : il est de nouveau accepté.
        assert!(matches!(
            r.on_packet(0, &entete(7), &id(0), T0),
            Decision::RelayScheduled { .. }
        ));
    }

    #[test]
    fn seen_set_oublie_apres_seen_ttl() {
        let mut r = routeur(2);
        r.on_packet(0, &entete(7), &id(1), T0);
        let seen_ttl = u64::from(SEEN_TTL_S) * 1_000;
        assert_eq!(
            r.on_packet(0, &entete(7), &id(1), T0 + seen_ttl - 1),
            Decision::Reject(RejectReason::Duplicate)
        );
        assert!(matches!(
            r.on_packet(0, &entete(7), &id(1), T0 + seen_ttl),
            Decision::RelayScheduled { .. }
        ));
    }

    #[test]
    fn seen_set_de_capacite_nulle_ne_retient_rien() {
        let mut cfg = RoutingConfig::new(MOI);
        cfg.seen_cap = 0;
        let mut r = Router::new(cfg, 1);
        r.link_up(0u32);
        r.on_packet(0, &entete(7), &id(1), T0);
        assert_eq!(r.seen_len(), 0);
    }

    // --- liens ----------------------------------------------------------

    #[test]
    fn cibles_calculees_a_l_echeance() {
        let mut r = routeur(3);
        let Decision::RelayScheduled { at_ms, .. } = r.on_packet(0, &entete(7), &id(1), T0) else {
            panic!("relais attendu");
        };
        r.link_down(1);
        r.link_up(5);
        // La source (0) est partie aussi : le relais reste valable pour les autres.
        r.link_down(0);
        assert_eq!(r.poll_due(at_ms)[0].targets, vec![2, 5]);
    }

    #[test]
    fn relais_sans_cible_abandonne() {
        let mut r = routeur(1);
        r.on_packet(0, &entete(7), &id(1), T0);
        assert!(r.poll_due(T0 + 1_000).is_empty());
        assert_eq!(r.stats().relays_without_target, 1);
        assert_eq!(r.pending_len(), 0);
    }

    #[test]
    fn link_up_idempotent() {
        let mut r = routeur(2);
        r.link_up(0);
        assert_eq!(r.neighbours(), 2);
        assert_eq!(r.config().local_id, MOI);
    }

    #[test]
    fn jitter_de_largeur_nulle() {
        let mut cfg = RoutingConfig::new(MOI);
        cfg.jitter_ms = 50..=50;
        let mut r = Router::new(cfg, 1);
        r.link_up(0u32);
        assert_eq!(
            r.on_packet(0, &entete(7), &id(1), T0),
            Decision::RelayScheduled {
                at_ms: T0 + 50,
                ttl: 6
            }
        );
    }

    #[test]
    fn splitmix64_vecteur_de_reference() {
        // Premières sorties pour la graine 0 (implémentation de référence de
        // Vigna, splitmix64.c).
        let mut g = SplitMix64(0);
        assert_eq!(g.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(g.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    }

    // --- property tests -----------------------------------------------------

    /// Une arrivée : (lien 0..8, msgID 0..32, ttl 0..=8, avance d'horloge ms).
    fn arrivees() -> impl Strategy<Value = Vec<(u32, u32, u8, u64)>> {
        prop::collection::vec((0u32..8, 0u32..32, 0u8..=8, 0u64..300), 0..200)
    }

    fn rejouer(seed: u64, voisins: u32, seq: &[(u32, u32, u8, u64)]) -> Vec<String> {
        let mut r = Router::new(RoutingConfig::new(MOI), seed);
        for l in 0..voisins {
            r.link_up(l);
        }
        let mut t = T0;
        let mut trace = Vec::new();
        for &(lien, n, ttl, dt) in seq {
            t += dt;
            trace.push(format!("{:?}", r.on_packet(lien, &entete(ttl), &id(n), t)));
            for o in r.poll_due(t) {
                trace.push(format!("{o:?}"));
            }
        }
        for o in r.poll_due(u64::MAX) {
            trace.push(format!("{o:?}"));
        }
        trace
    }

    proptest! {
        #[test]
        fn chaque_msg_id_est_relaye_au_plus_une_fois(voisins in 1u32..10, seq in arrivees()) {
            let mut r = Router::new(RoutingConfig::new(MOI), 7);
            for l in 0..voisins {
                r.link_up(l);
            }
            let mut t = T0;
            let mut relayes = alloc::collections::BTreeSet::new();
            let mut tout = Vec::new();
            for (lien, n, ttl, dt) in seq {
                t += dt;
                let d = r.on_packet(lien, &entete(ttl), &id(n), t);
                if let Decision::RelayScheduled { at_ms, ttl: t2 } = d {
                    prop_assert!(t2 < ttl);
                    if voisins >= u32::from(DENSE_LINKS) {
                        prop_assert!(t2 <= TTL_CLAMP_DENSE);
                    }
                    prop_assert!(RELAY_JITTER_MS.contains(&u16::try_from(at_ms - t).unwrap_or(u16::MAX)));
                }
                tout.extend(r.poll_due(t));
            }
            tout.extend(r.poll_due(u64::MAX));
            for o in &tout {
                // Fenêtre SEEN_TTL_S (300 s) > durée max de la séquence
                // (200 × 300 ms = 60 s) : aucun msgID n'est oublié ici.
                prop_assert!(relayes.insert(o.msg_id), "relayé deux fois");
                prop_assert!(!o.targets.is_empty());
            }
        }

        #[test]
        fn deterministe_a_graine_fixe(seed in any::<u64>(), voisins in 1u32..10, seq in arrivees()) {
            prop_assert_eq!(rejouer(seed, voisins, &seq), rejouer(seed, voisins, &seq));
        }
    }
}
