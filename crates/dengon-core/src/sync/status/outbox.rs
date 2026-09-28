//! Outbox persistante : les messages émis **non confirmés** et leur rejeu.
//!
//! Colonnes reprises de la table `outbox` de `docs/synthese/09` (`msg_uuid`,
//! `dest_peer_id`, `packet`, `kind`, `attempts`, `first_sent_ms`,
//! `last_sent_ms`, `expires_ms`), plus le statut courant — nécessaire au
//! rejeu après redémarrage sans dépendre de la table `messages`.
//!
//! Le stockage est abstrait derrière [`OutboxStore`], un simple magasin
//! clé → octets : SQLite (`store`, US-207) côté Android/PC, NVS/flash côté
//! ESP32. Chaque enregistrement est sérialisé par [`OutboxRecord::encode`],
//! format binaire versionné décrit sur cette méthode.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

use super::{next_status, MsgUuid, Status, StatusChange, StatusEvent, RESEND_MAX};
use crate::protocol::consts::{HEADER_LEN_ADDRESSED, MSG_TTL_S, PEER_ID_LEN, SIGNATURE_LEN};
use crate::protocol::{AckStatus, PeerId};

/// Nombre maximal de pairs distincts dont on compte les remises pour un même
/// message. Borne la mémoire d'un enregistrement (cible ESP32) : une fois la
/// table pleine, le message n'est plus proposé qu'aux pairs déjà comptés.
pub const ATTEMPT_PEERS_MAX: usize = 32;

/// Taille maximale d'un paquet L3 en outbox : en-tête adressé + `payload_len`
/// maximal (`u16`) + signature.
pub const PACKET_MAX_BYTES: usize = HEADER_LEN_ADDRESSED + u16::MAX as usize + SIGNATURE_LEN;

/// Version du format d'encodage d'[`OutboxRecord`].
const RECORD_VERSION: u8 = 1;

/// Voie d'acheminement du paquet (colonne `kind` de `synthese/09`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeliveryKind {
    /// `NOISE_MSG` dans une session Noise établie avec le destinataire.
    Session,
    /// `SEALED_ENVELOPE` (Noise `X`), pour un destinataire hors de portée.
    Envelope,
}

impl DeliveryKind {
    const fn to_u8(self) -> u8 {
        match self {
            Self::Session => 0,
            Self::Envelope => 1,
        }
    }

    const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Session),
            1 => Some(Self::Envelope),
            _ => None,
        }
    }
}

/// Un message à placer en outbox (sortie de `send_message`, déjà chiffré).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMessage {
    /// Identifiant stable du message.
    pub msg_uuid: MsgUuid,
    /// Destinataire final.
    pub dest_peer_id: PeerId,
    /// Voie d'acheminement.
    pub kind: DeliveryKind,
    /// Paquet L3 encodé, prêt à émettre.
    pub packet: Vec<u8>,
}

/// Un message en outbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxRecord {
    /// Identifiant stable du message.
    pub msg_uuid: MsgUuid,
    /// Destinataire final.
    pub dest_peer_id: PeerId,
    /// Voie d'acheminement.
    pub kind: DeliveryKind,
    /// Paquet L3 encodé, prêt à émettre.
    pub packet: Vec<u8>,
    /// Statut courant.
    pub status: Status,
    /// Horodatage (ms) de l'entrée dans le statut courant.
    pub status_ms: u64,
    /// Première remise à un pair, s'il y en a eu une.
    pub first_sent_ms: Option<u64>,
    /// Dernière remise à un pair, s'il y en a eu une.
    pub last_sent_ms: Option<u64>,
    /// Échéance (ms) : au-delà, le message expire.
    pub expires_ms: u64,
    /// Remises par pair, au plus [`ATTEMPT_PEERS_MAX`] entrées.
    pub attempts: Vec<(PeerId, u8)>,
}

impl OutboxRecord {
    /// Nombre de remises déjà faites à `peer`.
    #[must_use]
    pub fn attempts_for(&self, peer: &PeerId) -> u8 {
        self.attempts
            .iter()
            .find(|(p, _)| p == peer)
            .map_or(0, |&(_, n)| n)
    }

    /// `true` si le message doit être proposé à `peer` à la reconnexion :
    /// non terminal, non expiré, moins de [`RESEND_MAX`] remises à ce pair,
    /// et de la place pour compter ce pair s'il est nouveau.
    #[must_use]
    pub fn is_replayable_to(&self, peer: &PeerId, now_ms: u64) -> bool {
        if self.status.is_terminal() || now_ms > self.expires_ms {
            return false;
        }
        match self.attempts.iter().find(|(p, _)| p == peer) {
            Some(&(_, n)) => n < RESEND_MAX,
            None => self.attempts.len() < ATTEMPT_PEERS_MAX,
        }
    }

    /// Compte une remise à `peer` (saturante, et ignorée si la table des
    /// pairs est pleine).
    fn count_attempt(&mut self, peer: &PeerId) {
        if let Some((_, n)) = self.attempts.iter_mut().find(|(p, _)| p == peer) {
            *n = n.saturating_add(1);
        } else if self.attempts.len() < ATTEMPT_PEERS_MAX {
            self.attempts.push((*peer, 1));
        }
    }

    /// Encode l'enregistrement pour le stockage. Format v1, entiers en
    /// big-endian :
    ///
    /// ```text
    /// version(1)=1 ‖ msg_uuid(16) ‖ dest_peer_id(8) ‖ kind(1) ‖ status(1)
    /// ‖ status_ms(8) ‖ first_sent(1+8) ‖ last_sent(1+8) ‖ expires_ms(8)
    /// ‖ n_attempts(1) ‖ n × (peer_id(8) ‖ count(1))
    /// ‖ packet_len(4) ‖ packet
    /// ```
    ///
    /// `first_sent` / `last_sent` : un octet de présence (0 ou 1) suivi de
    /// 8 octets (à zéro si absent).
    ///
    /// # Errors
    ///
    /// [`RecordError::PacketTooLarge`] au-delà de [`PACKET_MAX_BYTES`],
    /// [`RecordError::TooManyPeers`] au-delà de [`ATTEMPT_PEERS_MAX`].
    pub fn encode(&self) -> Result<Vec<u8>, RecordError> {
        if self.packet.len() > PACKET_MAX_BYTES {
            return Err(RecordError::PacketTooLarge);
        }
        let n_attempts = u8::try_from(self.attempts.len())
            .ok()
            .filter(|&n| usize::from(n) <= ATTEMPT_PEERS_MAX)
            .ok_or(RecordError::TooManyPeers)?;
        let packet_len =
            u32::try_from(self.packet.len()).map_err(|_| RecordError::PacketTooLarge)?;

        let mut out = Vec::with_capacity(
            64 + usize::from(n_attempts) * (PEER_ID_LEN + 1) + self.packet.len(),
        );
        out.push(RECORD_VERSION);
        out.extend_from_slice(&self.msg_uuid);
        out.extend_from_slice(&self.dest_peer_id);
        out.push(self.kind.to_u8());
        out.push(self.status.to_u8());
        out.extend_from_slice(&self.status_ms.to_be_bytes());
        for ts in [self.first_sent_ms, self.last_sent_ms] {
            out.push(u8::from(ts.is_some()));
            out.extend_from_slice(&ts.unwrap_or(0).to_be_bytes());
        }
        out.extend_from_slice(&self.expires_ms.to_be_bytes());
        out.push(n_attempts);
        for (peer, count) in &self.attempts {
            out.extend_from_slice(peer);
            out.push(*count);
        }
        out.extend_from_slice(&packet_len.to_be_bytes());
        out.extend_from_slice(&self.packet);
        Ok(out)
    }

    /// Décode un enregistrement produit par [`OutboxRecord::encode`].
    /// Ne panique jamais, quelle que soit l'entrée.
    ///
    /// # Errors
    ///
    /// Toute incohérence renvoie une [`RecordError`] ; aucun octet en trop
    /// n'est toléré.
    pub fn decode(bytes: &[u8]) -> Result<Self, RecordError> {
        let mut r = Reader(bytes);
        let version = r.u8()?;
        if version != RECORD_VERSION {
            return Err(RecordError::UnknownVersion(version));
        }
        let msg_uuid: MsgUuid = r.array()?;
        let dest_peer_id: PeerId = r.array()?;
        let kind_code = r.u8()?;
        let kind = DeliveryKind::from_u8(kind_code).ok_or(RecordError::UnknownKind(kind_code))?;
        let status_code = r.u8()?;
        let status = Status::from_u8(status_code).ok_or(RecordError::UnknownStatus(status_code))?;
        let status_ms = r.u64()?;
        let first_sent_ms = r.opt_u64()?;
        let last_sent_ms = r.opt_u64()?;
        let expires_ms = r.u64()?;
        let n_attempts = usize::from(r.u8()?);
        if n_attempts > ATTEMPT_PEERS_MAX {
            return Err(RecordError::TooManyPeers);
        }
        let mut attempts = Vec::with_capacity(n_attempts);
        for _ in 0..n_attempts {
            let peer: PeerId = r.array()?;
            attempts.push((peer, r.u8()?));
        }
        let packet_len =
            usize::try_from(u32::from_be_bytes(r.array()?)).map_err(|_| RecordError::Truncated)?;
        if packet_len > PACKET_MAX_BYTES {
            return Err(RecordError::PacketTooLarge);
        }
        let packet = r.take(packet_len)?.to_vec();
        if !r.0.is_empty() {
            return Err(RecordError::TrailingBytes);
        }
        Ok(Self {
            msg_uuid,
            dest_peer_id,
            kind,
            packet,
            status,
            status_ms,
            first_sent_ms,
            last_sent_ms,
            expires_ms,
            attempts,
        })
    }
}

/// Curseur de lecture minimal, sans panic.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], RecordError> {
        if self.0.len() < n {
            return Err(RecordError::Truncated);
        }
        let (head, tail) = self.0.split_at(n);
        self.0 = tail;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RecordError> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, RecordError> {
        Ok(self.array::<1>()?[0])
    }

    fn u64(&mut self) -> Result<u64, RecordError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn opt_u64(&mut self) -> Result<Option<u64>, RecordError> {
        let present = self.u8()?;
        let value = self.u64()?;
        match present {
            0 => Ok(None),
            1 => Ok(Some(value)),
            other => Err(RecordError::InvalidFlag(other)),
        }
    }
}

/// Erreur de (dé)codage d'un [`OutboxRecord`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordError {
    /// Entrée trop courte.
    Truncated,
    /// Octets restants après la fin de l'enregistrement.
    TrailingBytes,
    /// Version de format inconnue.
    UnknownVersion(u8),
    /// Code de [`DeliveryKind`] inconnu.
    UnknownKind(u8),
    /// Code de [`Status`] inconnu.
    UnknownStatus(u8),
    /// Octet de présence différent de 0 ou 1.
    InvalidFlag(u8),
    /// Plus de [`ATTEMPT_PEERS_MAX`] pairs dans la table des remises.
    TooManyPeers,
    /// Paquet plus grand que [`PACKET_MAX_BYTES`].
    PacketTooLarge,
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("enregistrement d'outbox tronqué"),
            Self::TrailingBytes => f.write_str("octets en trop après l'enregistrement"),
            Self::UnknownVersion(v) => write!(f, "version d'enregistrement inconnue : {v}"),
            Self::UnknownKind(k) => write!(f, "voie d'acheminement inconnue : {k}"),
            Self::UnknownStatus(s) => write!(f, "statut inconnu : {s}"),
            Self::InvalidFlag(b) => write!(f, "octet de présence invalide : {b}"),
            Self::TooManyPeers => write!(f, "plus de {ATTEMPT_PEERS_MAX} pairs comptés"),
            Self::PacketTooLarge => write!(f, "paquet de plus de {PACKET_MAX_BYTES} octets"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RecordError {}

/// Magasin persistant de l'outbox : clé `msg_uuid` → enregistrement encodé.
///
/// Contrat : après un `save` réussi, un `load_all` ultérieur — y compris
/// après redémarrage du processus — renvoie ces octets ; après un `remove`
/// réussi, il ne les renvoie plus.
pub trait OutboxStore {
    /// Erreur d'entrée/sortie du support.
    type Error;

    /// Écrit (ou remplace) l'enregistrement de `msg_uuid`.
    ///
    /// # Errors
    ///
    /// Échec d'écriture du support.
    fn save(&mut self, msg_uuid: &MsgUuid, record: &[u8]) -> Result<(), Self::Error>;

    /// Supprime l'enregistrement de `msg_uuid` (sans effet s'il est absent).
    ///
    /// # Errors
    ///
    /// Échec d'écriture du support.
    fn remove(&mut self, msg_uuid: &MsgUuid) -> Result<(), Self::Error>;

    /// Tous les enregistrements présents.
    ///
    /// # Errors
    ///
    /// Échec de lecture du support.
    fn load_all(&self) -> Result<Vec<Vec<u8>>, Self::Error>;
}

/// [`OutboxStore`] en mémoire : pour les tests, `dengon-sim`, et comme
/// bouchon tant que `store` (US-207) n'est pas branché. Survit à la
/// destruction de l'[`Outbox`] via [`Outbox::into_store`], ce qui simule un
/// redémarrage.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    records: BTreeMap<MsgUuid, Vec<u8>>,
}

impl MemoryStore {
    /// Magasin vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Nombre d'enregistrements stockés.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// `true` si aucun enregistrement n'est stocké.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl OutboxStore for MemoryStore {
    type Error = core::convert::Infallible;

    fn save(&mut self, msg_uuid: &MsgUuid, record: &[u8]) -> Result<(), Self::Error> {
        self.records.insert(*msg_uuid, record.to_vec());
        Ok(())
    }

    fn remove(&mut self, msg_uuid: &MsgUuid) -> Result<(), Self::Error> {
        self.records.remove(msg_uuid);
        Ok(())
    }

    fn load_all(&self) -> Result<Vec<Vec<u8>>, Self::Error> {
        Ok(self.records.values().cloned().collect())
    }
}

/// Erreur d'une opération d'[`Outbox`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboxError<E> {
    /// Le support de stockage a échoué ; l'état en mémoire n'a **pas** été
    /// modifié.
    Store(E),
    /// Enregistrement illisible dans le stockage (à l'ouverture), ou
    /// impossible à encoder (paquet trop grand à l'`enqueue`).
    Record(RecordError),
}

impl<E: fmt::Display> fmt::Display for OutboxError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(e) => write!(f, "stockage de l'outbox : {e}"),
            Self::Record(e) => write!(f, "enregistrement d'outbox : {e}"),
        }
    }
}

#[cfg(feature = "std")]
impl<E: fmt::Debug + fmt::Display> std::error::Error for OutboxError<E> {}

/// Outbox : messages émis non confirmés, persistés dans `S`.
///
/// Toute mutation suit le même ordre : calcul du nouvel enregistrement,
/// **écriture dans le stockage**, puis seulement mise à jour de la copie en
/// mémoire. Si le stockage échoue, l'outbox reste dans son état précédent.
#[derive(Debug)]
pub struct Outbox<S> {
    store: S,
    records: BTreeMap<MsgUuid, OutboxRecord>,
}

impl<S: OutboxStore> Outbox<S> {
    /// Ouvre l'outbox à partir du stockage (démarrage ou redémarrage).
    ///
    /// Les enregistrements déjà terminaux — possibles si le processus s'est
    /// arrêté entre deux écritures — sont supprimés du stockage.
    ///
    /// # Errors
    ///
    /// [`OutboxError::Store`] si le stockage échoue,
    /// [`OutboxError::Record`] si un enregistrement est illisible.
    pub fn open(mut store: S) -> Result<Self, OutboxError<S::Error>> {
        let mut records = BTreeMap::new();
        for bytes in store.load_all().map_err(OutboxError::Store)? {
            let record = OutboxRecord::decode(&bytes).map_err(OutboxError::Record)?;
            if record.status.is_terminal() {
                store.remove(&record.msg_uuid).map_err(OutboxError::Store)?;
            } else {
                records.insert(record.msg_uuid, record);
            }
        }
        Ok(Self { store, records })
    }

    /// Rend le stockage (fermeture de l'outbox).
    #[must_use]
    pub fn into_store(self) -> S {
        self.store
    }

    /// Nombre de messages non confirmés.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// `true` si aucun message n'est en attente de confirmation.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Enregistrement d'un message non confirmé.
    #[must_use]
    pub fn get(&self, msg_uuid: &MsgUuid) -> Option<&OutboxRecord> {
        self.records.get(msg_uuid)
    }

    /// Tous les messages non confirmés, par `msg_uuid` croissant.
    pub fn iter(&self) -> impl Iterator<Item = &OutboxRecord> {
        self.records.values()
    }

    /// Place un message en outbox, statut `QUEUED`, échéance
    /// `now_ms + MSG_TTL_S`. Idempotent : un `msg_uuid` déjà présent n'est
    /// pas modifié et renvoie `Ok(None)`.
    ///
    /// Précondition : `msg_uuid` n'a jamais été utilisé (tiré au hasard sur
    /// 128 bits par l'appelant). Un message **terminé** n'est plus en outbox ;
    /// le ré-insérer créerait un nouveau cycle `QUEUED`.
    ///
    /// # Errors
    ///
    /// [`OutboxError::Record`] si le paquet dépasse [`PACKET_MAX_BYTES`],
    /// [`OutboxError::Store`] si le stockage échoue.
    pub fn enqueue(
        &mut self,
        msg: NewMessage,
        now_ms: u64,
    ) -> Result<Option<StatusChange>, OutboxError<S::Error>> {
        if self.records.contains_key(&msg.msg_uuid) {
            return Ok(None);
        }
        let record = OutboxRecord {
            msg_uuid: msg.msg_uuid,
            dest_peer_id: msg.dest_peer_id,
            kind: msg.kind,
            packet: msg.packet,
            status: Status::Queued,
            status_ms: now_ms,
            first_sent_ms: None,
            last_sent_ms: None,
            expires_ms: now_ms.saturating_add(u64::from(MSG_TTL_S) * 1000),
            attempts: Vec::new(),
        };
        self.persist(record)?;
        Ok(Some(StatusChange {
            msg_uuid: msg.msg_uuid,
            from: None,
            to: Status::Queued,
            at_ms: now_ms,
        }))
    }

    /// Messages à proposer à `peer` qui vient de se connecter (rejeu,
    /// `synthese/07` §3 et §6 étape 4). Le filtre « `peer` est destinataire
    /// **ou** bon candidat relais » relève de `sync::routing` (US-209).
    #[must_use]
    pub fn replay_candidates(&self, peer: &PeerId, now_ms: u64) -> Vec<&OutboxRecord> {
        self.records
            .values()
            .filter(|r| r.is_replayable_to(peer, now_ms))
            .collect()
    }

    /// Consigne la remise du message à `peer` : compte la tentative, met à
    /// jour `first_sent_ms` / `last_sent_ms`, et passe `QUEUED → IN_FLIGHT`
    /// à la première remise. Renvoie le changement de statut s'il y en a un
    /// (`None` pour une re-remise ou un message inconnu).
    ///
    /// # Errors
    ///
    /// [`OutboxError::Store`] si le stockage échoue.
    pub fn mark_handed_off(
        &mut self,
        msg_uuid: &MsgUuid,
        peer: &PeerId,
        now_ms: u64,
    ) -> Result<Option<StatusChange>, OutboxError<S::Error>> {
        let Some(current) = self.records.get(msg_uuid) else {
            return Ok(None);
        };
        let mut record = current.clone();
        record.count_attempt(peer);
        record.first_sent_ms.get_or_insert(now_ms);
        record.last_sent_ms = Some(now_ms);
        let change = transition(&mut record, StatusEvent::HandedOff, now_ms);
        self.persist(record)?;
        Ok(change)
    }

    /// Applique un Ack **déjà vérifié** (signature du destinataire) : passe le
    /// message en `DELIVERED` et le retire de l'outbox. Un Ack en double, pour
    /// un message inconnu, ou `READ` (v2) est ignoré.
    ///
    /// # Errors
    ///
    /// [`OutboxError::Store`] si le stockage échoue.
    pub fn apply_ack(
        &mut self,
        msg_uuid: &MsgUuid,
        status: AckStatus,
        now_ms: u64,
    ) -> Result<Option<StatusChange>, OutboxError<S::Error>> {
        self.apply_event(msg_uuid, StatusEvent::AckReceived(status), now_ms)
    }

    /// Annule un message encore `QUEUED`. `Ok(None)` s'il est déjà parti ou
    /// inconnu : l'appelant l'affiche comme « trop tard pour annuler ».
    ///
    /// # Errors
    ///
    /// [`OutboxError::Store`] si le stockage échoue.
    pub fn cancel(
        &mut self,
        msg_uuid: &MsgUuid,
        now_ms: u64,
    ) -> Result<Option<StatusChange>, OutboxError<S::Error>> {
        self.apply_event(msg_uuid, StatusEvent::Cancel, now_ms)
    }

    /// Fait expirer tous les messages dont l'échéance est dépassée
    /// (`now_ms > expires_ms`) et les retire de l'outbox.
    ///
    /// # Errors
    ///
    /// [`OutboxError::Store`] si le stockage échoue ; les expirations déjà
    /// persistées avant l'échec restent appliquées.
    pub fn expire_due(&mut self, now_ms: u64) -> Result<Vec<StatusChange>, OutboxError<S::Error>> {
        let due: Vec<MsgUuid> = self
            .records
            .values()
            .filter(|r| now_ms > r.expires_ms)
            .map(|r| r.msg_uuid)
            .collect();
        let mut changes = Vec::with_capacity(due.len());
        for msg_uuid in due {
            if let Some(change) = self.apply_event(&msg_uuid, StatusEvent::TtlElapsed, now_ms)? {
                changes.push(change);
            }
        }
        Ok(changes)
    }

    /// Transition sans effet de bord autre que le statut, puis persistance.
    fn apply_event(
        &mut self,
        msg_uuid: &MsgUuid,
        event: StatusEvent,
        now_ms: u64,
    ) -> Result<Option<StatusChange>, OutboxError<S::Error>> {
        let Some(current) = self.records.get(msg_uuid) else {
            return Ok(None);
        };
        let mut record = current.clone();
        let Some(change) = transition(&mut record, event, now_ms) else {
            return Ok(None);
        };
        self.persist(record)?;
        Ok(Some(change))
    }

    /// Écrit `record` dans le stockage puis en mémoire ; un record terminal
    /// est supprimé des deux.
    fn persist(&mut self, record: OutboxRecord) -> Result<(), OutboxError<S::Error>> {
        if record.status.is_terminal() {
            self.store
                .remove(&record.msg_uuid)
                .map_err(OutboxError::Store)?;
            self.records.remove(&record.msg_uuid);
        } else {
            let bytes = record.encode().map_err(OutboxError::Record)?;
            self.store
                .save(&record.msg_uuid, &bytes)
                .map_err(OutboxError::Store)?;
            self.records.insert(record.msg_uuid, record);
        }
        Ok(())
    }
}

/// Applique [`next_status`] à `record` ; renvoie le changement s'il y en a un.
fn transition(record: &mut OutboxRecord, event: StatusEvent, now_ms: u64) -> Option<StatusChange> {
    let to = next_status(record.status, event)?;
    let change = StatusChange {
        msg_uuid: record.msg_uuid,
        from: Some(record.status),
        to,
        at_ms: now_ms,
    };
    record.status = to;
    record.status_ms = now_ms;
    Some(change)
}

#[cfg(test)]
mod tests;
