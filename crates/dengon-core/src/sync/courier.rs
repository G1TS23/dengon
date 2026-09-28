//! Courrier : dépôt / collecte d'**enveloppes scellées** pour autrui (US-212).
//!
//! Fait foi : `docs/synthese/05-protocole-et-trame.md` §4 (`SEALED_ENVELOPE`,
//! `ENVELOPE_OFFER`, `ENVELOPE_REQUEST`) et §6.3 (collecte à chaque
//! rencontre), `docs/synthese/07-cycle-de-vie-et-statuts.md` §7
//! (expiration), `docs/synthese/06-securite.md` §3 (Noise `X`,
//! `recipient_tag`).
//!
//! Un nœud — téléphone ou relais ESP32 — **transporte** des enveloppes pour un
//! destinataire hors de portée, sans pouvoir les lire : le payload d'un
//! `SEALED_ENVELOPE` est
//!
//! ```text
//! recipient_tag(16) ‖ epoch_day(2) ‖ noise_x_ciphertext
//! ```
//!
//! et le courrier n'en lit **que** les deux premiers champs, en clair par
//! construction. Il ne reçoit ni ne détient aucune clé : le chiffrement Noise
//! `X` est fait par l'expéditeur, le déchiffrement par le destinataire.
//!
//! # Cycle
//!
//! 1. [`Courier::deposit`] : un `SEALED_ENVELOPE` reçu (signature déjà vérifiée
//!    par le pipeline de réception) est stocké tel quel, octet pour octet.
//! 2. À chaque rencontre : [`Courier::offer`] donne les `recipient_tag` à
//!    annoncer (`ENVELOPE_OFFER`) ; le pair répond avec ceux qui sont les
//!    siens (`ENVELOPE_REQUEST`) ; [`Courier::matching`] rend les enveloppes
//!    correspondantes à envoyer.
//! 3. Une fois l'envoi réussi, [`Courier::confirm_handoff`] retire l'enveloppe.
//! 4. [`Courier::expire`] supprime les enveloppes périmées.
//!
//! # Bornes
//!
//! Au plus [`CourierConfig::capacity`] enveloppes (`ENVELOPE_STORE_MAX = 64`
//! par défaut, plafond ESP32-WROOM), chacune d'au plus `ENVELOPE_MAX_BYTES`
//! octets. Magasin plein : politique **explicite** [`EvictionPolicy`].

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

use crate::protocol::consts::{
    ENVELOPE_MAX_BYTES, ENVELOPE_STORE_MAX, MSG_TTL_S, RECIPIENT_TAG_LEN,
};
use crate::protocol::{codec, DecodeError, MsgId, PacketType};

/// Tag anonyme et tournant du destinataire d'une enveloppe
/// (`HMAC-SHA256(pub_static, "dengon-tag" ‖ jour)[0..16]`, `synthese/06` §3).
/// Le courrier le compare, il ne le calcule jamais.
pub type RecipientTag = [u8; RECIPIENT_TAG_LEN];

/// Longueur de la partie en clair du payload : `recipient_tag(16) ‖ epoch_day(2)`.
pub const SEALED_HEADER_LEN: usize = RECIPIENT_TAG_LEN + 2;

/// Partie en clair du payload d'un `SEALED_ENVELOPE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealedHeader {
    /// Tag du destinataire pour le jour `epoch_day`.
    pub recipient_tag: RecipientTag,
    /// Jour (depuis l'epoch Unix) pour lequel le tag a été calculé.
    pub epoch_day: u16,
}

/// Sépare le payload d'un `SEALED_ENVELOPE` en partie en clair et
/// ciphertext Noise `X` (opaque, jamais interprété ici).
///
/// # Errors
///
/// [`CourierError::MalformedEnvelope`] si le payload ne contient pas au moins
/// l'en-tête et un octet de ciphertext.
pub fn parse_sealed_payload(payload: &[u8]) -> Result<(SealedHeader, &[u8]), CourierError> {
    if payload.len() <= SEALED_HEADER_LEN {
        return Err(CourierError::MalformedEnvelope);
    }
    let (head, ciphertext) = payload.split_at(SEALED_HEADER_LEN);
    let mut recipient_tag = [0; RECIPIENT_TAG_LEN];
    recipient_tag.copy_from_slice(&head[..RECIPIENT_TAG_LEN]);
    let epoch_day = u16::from_be_bytes([head[RECIPIENT_TAG_LEN], head[RECIPIENT_TAG_LEN + 1]]);
    Ok((
        SealedHeader {
            recipient_tag,
            epoch_day,
        },
        ciphertext,
    ))
}

/// Que faire quand le magasin est plein.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionPolicy {
    /// Refuser la nouvelle enveloppe : les enveloppes déjà acceptées sont
    /// protégées (`docs/synthese/08-relais-esp32.md` §7). **Par défaut.**
    RejectNew,
    /// Évincer l'enveloppe déposée le plus tôt (`synthese/05` §6.4,
    /// « éviction LRU / plus ancien »).
    EvictOldest,
}

/// Réglages du [`Courier`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CourierConfig {
    /// Nombre maximal d'enveloppes détenues.
    pub capacity: usize,
    /// Politique quand `capacity` est atteinte.
    pub policy: EvictionPolicy,
    /// Durée de vie d'une enveloppe, en ms (`MSG_TTL_S`).
    pub ttl_ms: u64,
}

impl Default for CourierConfig {
    fn default() -> Self {
        Self {
            capacity: ENVELOPE_STORE_MAX,
            policy: EvictionPolicy::RejectNew,
            ttl_ms: u64::from(MSG_TTL_S) * 1000,
        }
    }
}

/// Une enveloppe détenue pour autrui (table `held_envelopes`, `synthese/09`,
/// sans `copy_budget` qui est v2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldEnvelope {
    /// `msgID` du paquet (clé de dédup).
    pub msg_id: MsgId,
    /// Tag du destinataire, lu en clair dans le payload.
    pub recipient_tag: RecipientTag,
    /// Jour du tag.
    pub epoch_day: u16,
    /// Paquet L3 `SEALED_ENVELOPE` complet, **tel que reçu** (signature
    /// comprise) : c'est lui qui est réémis.
    pub packet: Vec<u8>,
    /// Instant du dépôt chez ce courrier (ms).
    pub deposit_ms: u64,
    /// Échéance (ms) : au-delà, l'enveloppe est supprimée.
    pub expires_ms: u64,
}

/// Résultat d'un dépôt accepté.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepositOutcome {
    /// Enveloppe stockée ; `evicted` = celle sacrifiée par
    /// [`EvictionPolicy::EvictOldest`], le cas échéant.
    Stored {
        /// `msgID` de l'enveloppe évincée pour faire de la place.
        evicted: Option<MsgId>,
    },
    /// Même `msgID` déjà détenu : rien n'a changé.
    Duplicate,
}

/// Refus d'un dépôt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CourierError {
    /// Paquet L3 illisible.
    Decode(DecodeError),
    /// Le paquet n'est pas un `SEALED_ENVELOPE`.
    NotSealedEnvelope(PacketType),
    /// Payload trop court pour `recipient_tag ‖ epoch_day ‖ ciphertext`.
    MalformedEnvelope,
    /// Paquet de plus de `ENVELOPE_MAX_BYTES` octets.
    TooLarge,
    /// Enveloppe déjà périmée à l'arrivée.
    Expired,
    /// Magasin plein et politique [`EvictionPolicy::RejectNew`].
    Full,
}

impl fmt::Display for CourierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(e) => write!(f, "paquet illisible : {e:?}"),
            Self::NotSealedEnvelope(t) => write!(f, "pas une enveloppe scellée : {t:?}"),
            Self::MalformedEnvelope => f.write_str("payload d'enveloppe trop court"),
            Self::TooLarge => write!(f, "enveloppe de plus de {ENVELOPE_MAX_BYTES} octets"),
            Self::Expired => f.write_str("enveloppe déjà expirée"),
            Self::Full => f.write_str("magasin d'enveloppes plein"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CourierError {}

/// Magasin d'enveloppes scellées détenues pour autrui, borné.
#[derive(Debug)]
pub struct Courier {
    config: CourierConfig,
    envelopes: BTreeMap<MsgId, HeldEnvelope>,
}

impl Default for Courier {
    fn default() -> Self {
        Self::new(CourierConfig::default())
    }
}

impl Courier {
    /// Courrier vide.
    #[must_use]
    pub fn new(config: CourierConfig) -> Self {
        Self {
            config,
            envelopes: BTreeMap::new(),
        }
    }

    /// Réglages en vigueur.
    #[must_use]
    pub fn config(&self) -> CourierConfig {
        self.config
    }

    /// Nombre d'enveloppes détenues.
    #[must_use]
    pub fn len(&self) -> usize {
        self.envelopes.len()
    }

    /// `true` si aucune enveloppe n'est détenue.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.envelopes.is_empty()
    }

    /// Enveloppe détenue sous ce `msgID`.
    #[must_use]
    pub fn get(&self, msg_id: &MsgId) -> Option<&HeldEnvelope> {
        self.envelopes.get(msg_id)
    }

    /// Dépose un `SEALED_ENVELOPE` reçu (paquet L3 encodé, **signature déjà
    /// vérifiée** par le pipeline de réception, `synthese/05` §6.1).
    ///
    /// Échéance : `min(timestamp_ms + ttl, now_ms + ttl)`. La première borne
    /// empêche une enveloppe de vivre indéfiniment en changeant de courrier ;
    /// la seconde neutralise une horloge d'émetteur en avance.
    ///
    /// Commence par [`Courier::expire`].
    ///
    /// # Errors
    ///
    /// Voir [`CourierError`] ; un dépôt refusé ne modifie pas le magasin.
    pub fn deposit(
        &mut self,
        msg_id: MsgId,
        raw_packet: Vec<u8>,
        now_ms: u64,
    ) -> Result<DepositOutcome, CourierError> {
        self.expire(now_ms);
        if raw_packet.len() > ENVELOPE_MAX_BYTES {
            return Err(CourierError::TooLarge);
        }
        let packet = codec::decode(&raw_packet).map_err(CourierError::Decode)?;
        if packet.header.packet_type != PacketType::SealedEnvelope {
            return Err(CourierError::NotSealedEnvelope(packet.header.packet_type));
        }
        let (header, _ciphertext) = parse_sealed_payload(&packet.payload)?;
        let ttl = self.config.ttl_ms;
        let expires_ms = packet
            .header
            .timestamp_ms
            .saturating_add(ttl)
            .min(now_ms.saturating_add(ttl));
        if now_ms > expires_ms {
            return Err(CourierError::Expired);
        }
        if self.envelopes.contains_key(&msg_id) {
            return Ok(DepositOutcome::Duplicate);
        }

        let mut evicted = None;
        if self.envelopes.len() >= self.config.capacity {
            match self.config.policy {
                EvictionPolicy::RejectNew => return Err(CourierError::Full),
                EvictionPolicy::EvictOldest => {
                    let Some(oldest) = self
                        .envelopes
                        .values()
                        .min_by_key(|e| (e.deposit_ms, e.msg_id))
                        .map(|e| e.msg_id)
                    else {
                        // Capacité nulle : rien à évincer, rien à stocker.
                        return Err(CourierError::Full);
                    };
                    self.envelopes.remove(&oldest);
                    evicted = Some(oldest);
                }
            }
        }

        self.envelopes.insert(
            msg_id,
            HeldEnvelope {
                msg_id,
                recipient_tag: header.recipient_tag,
                epoch_day: header.epoch_day,
                packet: raw_packet,
                deposit_ms: now_ms,
                expires_ms,
            },
        );
        Ok(DepositOutcome::Stored { evicted })
    }

    /// Tags à annoncer dans un `ENVELOPE_OFFER` : tags distincts des
    /// enveloppes non expirées, triés.
    #[must_use]
    pub fn offer(&self, now_ms: u64) -> Vec<RecipientTag> {
        let mut tags: Vec<RecipientTag> = self.live(now_ms).map(|e| e.recipient_tag).collect();
        tags.sort_unstable();
        tags.dedup();
        tags
    }

    /// Enveloppes non expirées dont le tag figure dans `requested` (contenu
    /// d'un `ENVELOPE_REQUEST`). Ne retire rien : voir
    /// [`Courier::confirm_handoff`].
    #[must_use]
    pub fn matching(&self, requested: &[RecipientTag], now_ms: u64) -> Vec<&HeldEnvelope> {
        self.live(now_ms)
            .filter(|e| requested.contains(&e.recipient_tag))
            .collect()
    }

    /// Retire une enveloppe **effectivement remise** au pair qui l'a
    /// demandée (après envoi réussi par le `Transport`). `None` si elle
    /// n'était plus détenue.
    pub fn confirm_handoff(&mut self, msg_id: &MsgId) -> Option<HeldEnvelope> {
        self.envelopes.remove(msg_id)
    }

    /// Supprime les enveloppes périmées (`now_ms > expires_ms`) et renvoie
    /// leurs `msgID` (pour l'événement `envelope.expired`).
    pub fn expire(&mut self, now_ms: u64) -> Vec<MsgId> {
        let expired: Vec<MsgId> = self
            .envelopes
            .values()
            .filter(|e| now_ms > e.expires_ms)
            .map(|e| e.msg_id)
            .collect();
        for id in &expired {
            self.envelopes.remove(id);
        }
        expired
    }

    fn live(&self, now_ms: u64) -> impl Iterator<Item = &HeldEnvelope> {
        self.envelopes
            .values()
            .filter(move |e| now_ms <= e.expires_ms)
    }
}

#[cfg(test)]
mod tests;
