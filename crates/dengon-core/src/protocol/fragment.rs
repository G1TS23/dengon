//! Fragmentation / réassemblage **L2** (US-202).
//!
//! Fait foi : [`docs/synthese/05-protocole-et-trame.md`] §5 (= `docs/powl/03`
//! §5). Un paquet L3 plus grand que ce que porte une écriture BLE est découpé
//! en fragments ; chacun voyage dans un paquet L3 de type `0x09`
//! ([`PacketType::Fragment`](super::PacketType::Fragment), drapeau
//! `FRAGMENT`) dont le payload est :
//!
//! ```text
//! frag_id(8) = SHA-256(paquet_complet)[0..8] ‖ index(2) ‖ total(2) ‖ chunk(<= FRAG_SIZE)
//! ```
//!
//! Entiers en big-endian, `index` de `0` à `total - 1`.
//!
//! Ce module produit et consomme ce **payload** ([`Fragment`]) ; l'habillage
//! dans l'en-tête L3 `0x09` relève du codec (`protocol::codec`, US-201).
//!
//! # Robustesse
//!
//! [`Reassembler`] accepte les fragments dans n'importe quel ordre, ignore les
//! doublons, abandonne un réassemblage inactif depuis `FRAG_TIMEOUT_S`, et
//! **borne sa mémoire** trois fois : nombre de réassemblages simultanés
//! (`FRAG_MAX_CONCURRENT`, éviction du plus ancien), taille d'un paquet
//! ([`PACKET_MAX_LEN`]) et total d'octets en attente
//! ([`ReassemblerConfig::max_bytes`]). Un fragment n'a pas de somme de
//! contrôle propre : le paquet reconstruit est vérifié contre son `frag_id`
//! (qui **est** un condensat SHA-256), ce qui détecte un mélange de fragments
//! ou un fragment altéré.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

use sha2::{Digest, Sha256};

use super::consts::{
    FRAG_MAX_CONCURRENT, FRAG_SIZE, FRAG_TIMEOUT_S, HEADER_LEN_ADDRESSED, SIGNATURE_LEN,
};

/// Longueur d'un `frag_id`.
pub const FRAG_ID_LEN: usize = 8;

/// Longueur de l'en-tête d'un fragment : `frag_id(8) ‖ index(2) ‖ total(2)`.
pub const FRAG_HEADER_LEN: usize = FRAG_ID_LEN + 2 + 2;

/// Octets d'une écriture ATT pris par le protocole ATT lui-même
/// (opcode 1 + handle 2) : la charge utile vaut `ATT_MTU - 3`.
pub const ATT_OVERHEAD: usize = 3;

/// Taille maximale d'un paquet L3 : en-tête adressé + `payload_len` maximal
/// (`u16`) + signature.
pub const PACKET_MAX_LEN: usize = HEADER_LEN_ADDRESSED + u16::MAX as usize + SIGNATURE_LEN;

/// Plus petit `ATT_MTU` qui laisse au moins 1 octet de chunk :
/// `3 (ATT) + 30 (en-tête L3 adressé) + 12 (en-tête de fragment) + 1`.
/// En dessous — en particulier au minimum BLE `ATT_MTU_MIN = 23` — un
/// fragment dengon ne tient pas dans une écriture.
pub const MIN_USABLE_ATT_MTU: usize = ATT_OVERHEAD + HEADER_LEN_ADDRESSED + FRAG_HEADER_LEN + 1;

/// Coût mémoire forfaitaire compté par chunk en attente, en plus de ses
/// octets (entrée de `BTreeMap` + en-tête de `Vec`). Empêche qu'un flot de
/// chunks d'un octet contourne [`ReassemblerConfig::max_bytes`].
pub const CHUNK_OVERHEAD: usize = 32;

/// Budget mémoire par défaut du réassembleur : 128 Kio, de quoi tenir un
/// paquet de taille maximale ([`PACKET_MAX_LEN`]) découpé en chunks de
/// `FRAG_SIZE`, et assez peu pour l'ESP32-WROOM.
pub const REASSEMBLY_MAX_BYTES: usize = 128 * 1024;

/// Nombre de `frag_id` récemment réassemblés dont on se souvient, pour
/// ignorer leurs fragments en retard ou dupliqués (au plus `timeout_ms`).
pub const COMPLETED_MEMORY: usize = FRAG_MAX_CONCURRENT;

/// Identifiant d'un paquet fragmenté : `SHA-256(paquet_complet)[0..8]`.
pub type FragId = [u8; FRAG_ID_LEN];

/// Calcule le `frag_id` d'un paquet L3 complet.
#[must_use]
pub fn frag_id(packet: &[u8]) -> FragId {
    let digest = Sha256::digest(packet);
    let mut id = [0; FRAG_ID_LEN];
    id.copy_from_slice(&digest[..FRAG_ID_LEN]);
    id
}

/// Taille de chunk utilisable pour un `ATT_MTU` négocié :
/// `min(FRAG_SIZE, ATT_MTU - 3 - 30 - 12)`. L'en-tête L3 est compté dans sa
/// forme **adressée** (30 o), la plus longue : un fragment hérite de
/// l'adressage du paquet qu'il transporte. Pas de signature : un fragment
/// n'est pas signé (`synthese/05` §5).
///
/// `None` si le MTU est inférieur à [`MIN_USABLE_ATT_MTU`].
#[must_use]
pub fn chunk_capacity(att_mtu: u16) -> Option<usize> {
    usize::from(att_mtu)
        .checked_sub(ATT_OVERHEAD + HEADER_LEN_ADDRESSED + FRAG_HEADER_LEN)
        .filter(|&n| n > 0)
        .map(|n| n.min(FRAG_SIZE))
}

/// Payload d'un paquet `FRAGMENT` décodé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    /// Identifiant du paquet complet.
    pub frag_id: FragId,
    /// Rang du fragment, de `0` à `total - 1`.
    pub index: u16,
    /// Nombre total de fragments du paquet (≥ 1).
    pub total: u16,
    /// Morceau du paquet, `1..=FRAG_SIZE` octets.
    pub chunk: Vec<u8>,
}

impl Fragment {
    /// Encode le payload (`frag_id ‖ index ‖ total ‖ chunk`).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(FRAG_HEADER_LEN + self.chunk.len());
        out.extend_from_slice(&self.frag_id);
        out.extend_from_slice(&self.index.to_be_bytes());
        out.extend_from_slice(&self.total.to_be_bytes());
        out.extend_from_slice(&self.chunk);
        out
    }

    /// Décode un payload de fragment. Ne panique jamais.
    ///
    /// # Errors
    ///
    /// [`FragmentError::Truncated`] sous [`FRAG_HEADER_LEN`] octets, puis les
    /// erreurs de [`Fragment::validate`] (un en-tête seul donne
    /// [`FragmentError::EmptyChunk`]).
    pub fn decode(bytes: &[u8]) -> Result<Self, FragmentError> {
        if bytes.len() < FRAG_HEADER_LEN {
            return Err(FragmentError::Truncated);
        }
        let (header, chunk) = bytes.split_at(FRAG_HEADER_LEN);
        let mut frag_id = [0; FRAG_ID_LEN];
        frag_id.copy_from_slice(&header[..FRAG_ID_LEN]);
        let fragment = Self {
            frag_id,
            index: u16::from_be_bytes([header[8], header[9]]),
            total: u16::from_be_bytes([header[10], header[11]]),
            chunk: chunk.to_vec(),
        };
        fragment.validate()?;
        Ok(fragment)
    }

    /// Vérifie les invariants d'un fragment isolé.
    ///
    /// # Errors
    ///
    /// [`FragmentError::InvalidIndex`] si `total == 0` ou `index >= total` ;
    /// [`FragmentError::EmptyChunk`] ; [`FragmentError::ChunkTooLarge`]
    /// au-delà de `FRAG_SIZE`.
    pub fn validate(&self) -> Result<(), FragmentError> {
        if self.total == 0 || self.index >= self.total {
            return Err(FragmentError::InvalidIndex);
        }
        if self.chunk.is_empty() {
            return Err(FragmentError::EmptyChunk);
        }
        if self.chunk.len() > FRAG_SIZE {
            return Err(FragmentError::ChunkTooLarge);
        }
        Ok(())
    }
}

/// Découpe `packet` en fragments d'au plus `chunk_len` octets.
///
/// # Errors
///
/// [`FragmentError::EmptyPacket`], [`FragmentError::PacketTooLarge`]
/// (> [`PACKET_MAX_LEN`]), [`FragmentError::InvalidChunkLen`] (`0` ou
/// > `FRAG_SIZE`), [`FragmentError::TooManyFragments`] (> `u16::MAX`).
pub fn split(packet: &[u8], chunk_len: usize) -> Result<Vec<Fragment>, FragmentError> {
    if packet.is_empty() {
        return Err(FragmentError::EmptyPacket);
    }
    if packet.len() > PACKET_MAX_LEN {
        return Err(FragmentError::PacketTooLarge);
    }
    if chunk_len == 0 || chunk_len > FRAG_SIZE {
        return Err(FragmentError::InvalidChunkLen);
    }
    let total = u16::try_from(packet.len().div_ceil(chunk_len))
        .map_err(|_| FragmentError::TooManyFragments)?;
    let id = frag_id(packet);
    Ok(packet
        .chunks(chunk_len)
        .zip(0u16..)
        .map(|(chunk, index)| Fragment {
            frag_id: id,
            index,
            total,
            chunk: chunk.to_vec(),
        })
        .collect())
}

/// [`split`] avec la taille de chunk tirée d'un `ATT_MTU` négocié
/// ([`chunk_capacity`]).
///
/// # Errors
///
/// [`FragmentError::MtuTooSmall`] sous [`MIN_USABLE_ATT_MTU`], puis les
/// erreurs de [`split`].
pub fn split_for_mtu(packet: &[u8], att_mtu: u16) -> Result<Vec<Fragment>, FragmentError> {
    let chunk_len = chunk_capacity(att_mtu).ok_or(FragmentError::MtuTooSmall)?;
    split(packet, chunk_len)
}

/// `true` si `packet_len` octets ne tiennent pas dans une seule écriture ATT
/// au MTU donné.
#[must_use]
pub fn needs_fragmentation(packet_len: usize, att_mtu: u16) -> bool {
    packet_len > usize::from(att_mtu).saturating_sub(ATT_OVERHEAD)
}

/// Erreurs de fragmentation et de réassemblage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentError {
    /// Payload plus court que l'en-tête de fragment.
    Truncated,
    /// Chunk vide.
    EmptyChunk,
    /// Chunk de plus de `FRAG_SIZE` octets.
    ChunkTooLarge,
    /// `total == 0` ou `index >= total`.
    InvalidIndex,
    /// Paquet vide à découper.
    EmptyPacket,
    /// Paquet (ou réassemblage en cours) au-delà de [`PACKET_MAX_LEN`].
    PacketTooLarge,
    /// Taille de chunk demandée nulle ou supérieure à `FRAG_SIZE`.
    InvalidChunkLen,
    /// Plus de `u16::MAX` fragments.
    TooManyFragments,
    /// `ATT_MTU` inférieur à [`MIN_USABLE_ATT_MTU`].
    MtuTooSmall,
    /// `total` différent de celui des fragments déjà reçus pour ce `frag_id`.
    InconsistentTotal,
    /// Le fragment ferait dépasser le budget mémoire du réassembleur.
    OverBudget,
    /// Le paquet reconstruit ne correspond pas à son `frag_id`.
    IntegrityMismatch,
}

impl fmt::Display for FragmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::Truncated => "fragment tronqué",
            Self::EmptyChunk => "fragment vide",
            Self::ChunkTooLarge => "fragment plus grand que FRAG_SIZE",
            Self::InvalidIndex => "index ou total de fragment invalide",
            Self::EmptyPacket => "paquet vide",
            Self::PacketTooLarge => "paquet plus grand que PACKET_MAX_LEN",
            Self::InvalidChunkLen => "taille de chunk invalide",
            Self::TooManyFragments => "plus de 65535 fragments",
            Self::MtuTooSmall => "MTU ATT trop petit pour un fragment",
            Self::InconsistentTotal => "total incohérent pour ce frag_id",
            Self::OverBudget => "budget mémoire du réassembleur dépassé",
            Self::IntegrityMismatch => "paquet réassemblé différent de son frag_id",
        };
        f.write_str(msg)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FragmentError {}

/// Réglages du [`Reassembler`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReassemblerConfig {
    /// Réassemblages simultanés au plus (`FRAG_MAX_CONCURRENT`).
    pub max_concurrent: usize,
    /// Inactivité (ms) au-delà de laquelle un réassemblage est abandonné.
    pub timeout_ms: u64,
    /// Octets en attente au plus, tous réassemblages confondus (chunks +
    /// [`CHUNK_OVERHEAD`] par chunk).
    pub max_bytes: usize,
}

impl Default for ReassemblerConfig {
    fn default() -> Self {
        Self {
            max_concurrent: FRAG_MAX_CONCURRENT,
            timeout_ms: u64::from(FRAG_TIMEOUT_S) * 1000,
            max_bytes: REASSEMBLY_MAX_BYTES,
        }
    }
}

/// Un réassemblage en cours.
#[derive(Debug)]
struct Partial {
    total: u16,
    chunks: BTreeMap<u16, Vec<u8>>,
    /// Somme des longueurs de chunks (borne [`PACKET_MAX_LEN`]).
    packet_len: usize,
    /// Coût compté dans le budget global.
    cost: usize,
    /// Ordre de création, pour évincer le plus ancien.
    created: u64,
    last_ms: u64,
}

/// Réassembleur de fragments, à mémoire bornée. Un par nœud suffit : les
/// réassemblages sont indexés par `frag_id`.
#[derive(Debug)]
pub struct Reassembler {
    config: ReassemblerConfig,
    partials: BTreeMap<FragId, Partial>,
    /// `frag_id` réassemblés récemment → instant du réassemblage.
    completed: BTreeMap<FragId, u64>,
    buffered: usize,
    next_created: u64,
}

impl Default for Reassembler {
    fn default() -> Self {
        Self::new(ReassemblerConfig::default())
    }
}

impl Reassembler {
    /// Réassembleur vide.
    #[must_use]
    pub fn new(config: ReassemblerConfig) -> Self {
        Self {
            config,
            partials: BTreeMap::new(),
            completed: BTreeMap::new(),
            buffered: 0,
            next_created: 0,
        }
    }

    /// Réglages en vigueur.
    #[must_use]
    pub fn config(&self) -> ReassemblerConfig {
        self.config
    }

    /// Nombre de réassemblages en cours.
    #[must_use]
    pub fn len(&self) -> usize {
        self.partials.len()
    }

    /// `true` si aucun réassemblage n'est en cours.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.partials.is_empty()
    }

    /// Octets comptés dans le budget ([`ReassemblerConfig::max_bytes`]).
    #[must_use]
    pub fn buffered_bytes(&self) -> usize {
        self.buffered
    }

    /// Abandonne les réassemblages inactifs depuis plus de `timeout_ms`, et
    /// oublie les `frag_id` terminés depuis plus longtemps. Renvoie le nombre
    /// de réassemblages abandonnés.
    pub fn expire(&mut self, now_ms: u64) -> usize {
        let timeout = self.config.timeout_ms;
        self.completed
            .retain(|_, done_ms| now_ms.saturating_sub(*done_ms) <= timeout);
        let stale: Vec<FragId> = self
            .partials
            .iter()
            .filter(|(_, p)| now_ms.saturating_sub(p.last_ms) > timeout)
            .map(|(id, _)| *id)
            .collect();
        for id in &stale {
            self.drop_partial(id);
        }
        stale.len()
    }

    /// Ajoute un fragment reçu. Renvoie le paquet L3 complet quand le dernier
    /// fragment manquant arrive, `None` sinon — y compris pour un doublon, ou
    /// un fragment d'un paquet déjà réassemblé il y a moins de `timeout_ms`
    /// (le paquet ne sort qu'**une** fois).
    ///
    /// # Errors
    ///
    /// Le fragment est rejeté — sans corrompre les autres réassemblages —
    /// s'il est invalide ([`Fragment::validate`]), si son `total` contredit
    /// les fragments déjà reçus ([`FragmentError::InconsistentTotal`]), si
    /// le paquet dépasserait [`PACKET_MAX_LEN`] ou le budget mémoire
    /// ([`FragmentError::PacketTooLarge`], [`FragmentError::OverBudget`] :
    /// le réassemblage concerné est alors abandonné), ou si le paquet
    /// reconstruit ne correspond pas à son `frag_id`
    /// ([`FragmentError::IntegrityMismatch`]).
    pub fn push(
        &mut self,
        fragment: Fragment,
        now_ms: u64,
    ) -> Result<Option<Vec<u8>>, FragmentError> {
        fragment.validate()?;
        self.expire(now_ms);

        let id = fragment.frag_id;
        if self.completed.contains_key(&id) {
            return Ok(None);
        }

        // Paquet en un seul fragment : rien à mettre en attente.
        if fragment.total == 1 {
            let packet = check_integrity(&id, fragment.chunk)?;
            self.remember_completed(id, now_ms);
            return Ok(Some(packet));
        }

        let cost = fragment.chunk.len() + CHUNK_OVERHEAD;
        if cost > self.config.max_bytes {
            self.drop_partial(&id);
            return Err(FragmentError::OverBudget);
        }

        if let Some(partial) = self.partials.get(&id) {
            if partial.total != fragment.total {
                return Err(FragmentError::InconsistentTotal);
            }
            if partial.chunks.contains_key(&fragment.index) {
                return Ok(None);
            }
            if partial.packet_len + fragment.chunk.len() > PACKET_MAX_LEN {
                self.drop_partial(&id);
                return Err(FragmentError::PacketTooLarge);
            }
        } else {
            while self.partials.len() >= self.config.max_concurrent.max(1) {
                self.evict_oldest(None);
            }
            self.partials.insert(
                id,
                Partial {
                    total: fragment.total,
                    chunks: BTreeMap::new(),
                    packet_len: 0,
                    cost: 0,
                    created: self.next_created,
                    last_ms: now_ms,
                },
            );
            self.next_created += 1;
        }

        // Budget global : on évince les autres réassemblages, du plus ancien
        // au plus récent, avant de sacrifier celui-ci.
        while self.buffered + cost > self.config.max_bytes {
            if !self.evict_oldest(Some(&id)) {
                self.drop_partial(&id);
                return Err(FragmentError::OverBudget);
            }
        }

        let Some(partial) = self.partials.get_mut(&id) else {
            return Ok(None);
        };
        partial.packet_len += fragment.chunk.len();
        partial.cost += cost;
        partial.last_ms = now_ms;
        partial.chunks.insert(fragment.index, fragment.chunk);
        self.buffered += cost;

        if partial.chunks.len() < usize::from(partial.total) {
            return Ok(None);
        }
        let Some(done) = self.partials.remove(&id) else {
            return Ok(None);
        };
        self.buffered -= done.cost;
        let mut packet = Vec::with_capacity(done.packet_len);
        for chunk in done.chunks.into_values() {
            packet.extend_from_slice(&chunk);
        }
        let packet = check_integrity(&id, packet)?;
        self.remember_completed(id, now_ms);
        Ok(Some(packet))
    }

    /// Mémorise un `frag_id` réassemblé (au plus [`COMPLETED_MEMORY`], le
    /// plus ancien est oublié).
    fn remember_completed(&mut self, id: FragId, now_ms: u64) {
        while self.completed.len() >= COMPLETED_MEMORY {
            let oldest = self
                .completed
                .iter()
                .min_by_key(|(_, t)| **t)
                .map(|(id, _)| *id);
            match oldest {
                Some(old) => self.completed.remove(&old),
                None => break,
            };
        }
        self.completed.insert(id, now_ms);
    }

    /// Évince le réassemblage le plus ancien, sauf `keep`. `false` s'il n'y
    /// avait rien à évincer.
    fn evict_oldest(&mut self, keep: Option<&FragId>) -> bool {
        let oldest = self
            .partials
            .iter()
            .filter(|(id, _)| Some(*id) != keep)
            .min_by_key(|(_, p)| p.created)
            .map(|(id, _)| *id);
        match oldest {
            Some(id) => {
                self.drop_partial(&id);
                true
            }
            None => false,
        }
    }

    fn drop_partial(&mut self, id: &FragId) {
        if let Some(p) = self.partials.remove(id) {
            self.buffered -= p.cost;
        }
    }
}

fn check_integrity(id: &FragId, packet: Vec<u8>) -> Result<Vec<u8>, FragmentError> {
    if frag_id(&packet) == *id {
        Ok(packet)
    } else {
        Err(FragmentError::IntegrityMismatch)
    }
}

#[cfg(test)]
mod tests;
