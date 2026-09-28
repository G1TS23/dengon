//! Statuts d'un message émis — machine à états **MVP** + outbox persistante (US-211).
//!
//! Fait foi : [`docs/synthese/07-cycle-de-vie-et-statuts.md`] §1-3 et §7
//! (décision A-10 : le statut « Lu » / `READ` est **reporté en v2**).
//!
//! ```text
//! [*]       --> QUEUED    : Outbox::enqueue (send_message)
//! QUEUED    --> IN_FLIGHT : 1er paquet remis à un pair OU enveloppe déposée
//! QUEUED    --> CANCELLED : cancel(), avant tout hand-off
//! IN_FLIGHT --> DELIVERED : Ack{status=2} signé reçu
//! IN_FLIGHT --> EXPIRED   : MSG_TTL_S écoulé sans Ack
//! ```
//!
//! Deux transitions supplémentaires, **absentes du diagramme de `synthese/07`
//! §2** mais nécessaires (consignées dans `docs/suivi/03-ecarts-conception.md`) :
//!
//! - `QUEUED --> EXPIRED` : un message jamais remis pendant 24 h expire aussi
//!   (règle d'expiration de `synthese/07` §7, qui ne distingue pas le statut) ;
//! - `QUEUED --> DELIVERED` : si l'appareil s'arrête entre l'envoi radio et
//!   l'écriture de `IN_FLIGHT`, l'Ack peut arriver sur un message resté
//!   `QUEUED`. L'ignorer perdrait une remise réelle.
//!
//! # Monotonie
//!
//! Chaque statut a un **rang** ([`Status::rank`]) : `QUEUED` (0) <
//! `IN_FLIGHT` (1) < statuts terminaux (2). [`next_status`] ne renvoie jamais
//! un statut de rang inférieur, et un statut terminal n'évolue plus. Un Ack en
//! double, un Ack `READ` (v2) ou un `cancel()` tardif sont donc **ignorés**
//! (`None`), pas des erreurs. La propriété est vérifiée par property test.
//!
//! # Outbox
//!
//! [`Outbox`] garde les messages **non confirmés** (`QUEUED` / `IN_FLIGHT`) et
//! écrit chaque mutation dans un [`OutboxStore`] **avant** de rendre la main :
//! après un redémarrage, [`Outbox::open`] relit le stockage et
//! [`Outbox::replay_candidates`] redonne les messages à renvoyer (règles de
//! rejeu de `synthese/07` §3). Un message qui atteint un statut terminal est
//! retiré de l'outbox ; la trace durable de son statut appartient à la table
//! `messages` (`store`, US-207) et au journal (`ledger`, US-206), que
//! l'appelant alimente avec les [`StatusChange`] renvoyés.

pub mod outbox;

pub use outbox::{
    DeliveryKind, MemoryStore, NewMessage, Outbox, OutboxError, OutboxRecord, OutboxStore,
    RecordError, ATTEMPT_PEERS_MAX, PACKET_MAX_BYTES,
};

use crate::protocol::AckStatus;

/// Longueur d'un `msg_uuid` : 16 octets, stable de bout en bout
/// (`AppFrame` `Text`, `docs/synthese/05` §4 ; table `messages`, `synthese/09`).
pub const MSG_UUID_LEN: usize = 16;

/// Identifiant applicatif d'un message, **stable de bout en bout**. Le suivi de
/// statut porte sur lui, pas sur le `msgID` L3 (qui change à chaque paquet).
pub type MsgUuid = [u8; MSG_UUID_LEN];

/// Nombre maximal de remises d'un même message **à un même pair**
/// (`RESEND_MAX`, `synthese/07` §3). Illimité dans le temps tant que le
/// message n'a pas expiré.
///
/// Absente de `protocol::consts` : c'est une règle locale d'outbox, pas une
/// valeur échangée sur le fil.
pub const RESEND_MAX: u8 = 8;

/// Statut d'un message émis — sous-ensemble **MVP** (sans `READ`, A-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// « En attente » : créé, chiffré, dans l'outbox, remis à personne.
    Queued,
    /// « Parti » : remis à au moins un pair ou relais. Le réseau s'en occupe.
    InFlight,
    /// « Distribué » : Ack signé de l'appareil destinataire reçu. Terminal.
    Delivered,
    /// « Échec » : `MSG_TTL_S` écoulé sans Ack. Terminal.
    Expired,
    /// « Annulé » : retiré par l'expéditeur avant tout hand-off. Terminal.
    Cancelled,
}

impl Status {
    /// Tous les statuts, dans l'ordre de leur code de persistance.
    pub const ALL: [Self; 5] = [
        Self::Queued,
        Self::InFlight,
        Self::Delivered,
        Self::Expired,
        Self::Cancelled,
    ];

    /// Rang dans l'ordre partiel des statuts ; ne décroît jamais.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Queued => 0,
            Self::InFlight => 1,
            Self::Delivered | Self::Expired | Self::Cancelled => 2,
        }
    }

    /// `true` si plus aucune transition n'est possible.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        self.rank() == 2
    }

    /// Code texte, identique à la contrainte `CHECK` de la colonne
    /// `messages.status` (`docs/synthese/09`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::InFlight => "in_flight",
            Self::Delivered => "delivered",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
        }
    }

    /// Inverse de [`Status::as_str`]. `None` pour un code inconnu — y compris
    /// `"read"`, réservé v2.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == code)
    }

    /// Événement de journal émis à l'**entrée** dans ce statut
    /// (`synthese/07` §1). `msg.cancelled` est un événement du journal local
    /// uniquement : il n'est pas dans le catalogue VPS (`contracts/events`).
    #[must_use]
    pub const fn event_name(self) -> &'static str {
        match self {
            Self::Queued => "msg.queued",
            Self::InFlight => "msg.handed_off",
            Self::Delivered => "msg.delivered",
            Self::Expired => "msg.expired",
            Self::Cancelled => "msg.cancelled",
        }
    }

    /// Code sur un octet, utilisé par l'encodage d'[`OutboxRecord`].
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Queued => 0,
            Self::InFlight => 1,
            Self::Delivered => 2,
            Self::Expired => 3,
            Self::Cancelled => 4,
        }
    }

    /// Inverse de [`Status::to_u8`]. `None` si le code est inconnu.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Queued,
            1 => Self::InFlight,
            2 => Self::Delivered,
            3 => Self::Expired,
            4 => Self::Cancelled,
            _ => return None,
        })
    }
}

/// Ce qui peut arriver à un message émis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusEvent {
    /// Un paquet du message a été remis à un pair, ou son enveloppe déposée.
    HandedOff,
    /// Un Ack signé du destinataire a été reçu et vérifié.
    AckReceived(AckStatus),
    /// `expires_ms` est dépassé.
    TtlElapsed,
    /// L'expéditeur retire le message.
    Cancel,
}

/// **La** fonction de transition : statut suivant, ou `None` si l'événement
/// ne change rien (événement tardif, en double, ou hors MVP).
///
/// Pure et sans horloge : c'est ce qui permet de la tester exhaustivement.
#[must_use]
pub const fn next_status(current: Status, event: StatusEvent) -> Option<Status> {
    use Status::{Cancelled, Delivered, Expired, InFlight, Queued};
    match (current, event) {
        (Queued, StatusEvent::HandedOff) => Some(InFlight),
        (Queued | InFlight, StatusEvent::AckReceived(AckStatus::Delivered)) => Some(Delivered),
        (Queued | InFlight, StatusEvent::TtlElapsed) => Some(Expired),
        (Queued, StatusEvent::Cancel) => Some(Cancelled),
        // IN_FLIGHT + HandedOff (re-remise), Ack READ (v2), terminal + tout,
        // cancel après hand-off : aucun changement.
        _ => None,
    }
}

/// Un changement de statut effectivement appliqué — à journaliser par
/// l'appelant (`ledger.append(change.event_name())`) et à répercuter dans la
/// table `messages`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusChange {
    /// Message concerné.
    pub msg_uuid: MsgUuid,
    /// Statut précédent ; `None` à la création (entrée en `QUEUED`).
    pub from: Option<Status>,
    /// Nouveau statut.
    pub to: Status,
    /// Horodatage fourni par l'appelant (ms Unix).
    pub at_ms: u64,
}

impl StatusChange {
    /// Événement de journal correspondant (voir [`Status::event_name`]).
    #[must_use]
    pub const fn event_name(&self) -> &'static str {
        self.to.event_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const EVENTS: [StatusEvent; 5] = [
        StatusEvent::HandedOff,
        StatusEvent::AckReceived(AckStatus::Delivered),
        StatusEvent::AckReceived(AckStatus::Read),
        StatusEvent::TtlElapsed,
        StatusEvent::Cancel,
    ];

    #[test]
    fn chemin_nominal_queued_in_flight_delivered() {
        let s = next_status(Status::Queued, StatusEvent::HandedOff);
        assert_eq!(s, Some(Status::InFlight));
        let s = next_status(
            Status::InFlight,
            StatusEvent::AckReceived(AckStatus::Delivered),
        );
        assert_eq!(s, Some(Status::Delivered));
    }

    #[test]
    fn re_remise_ne_change_pas_le_statut() {
        assert_eq!(next_status(Status::InFlight, StatusEvent::HandedOff), None);
    }

    #[test]
    fn ack_read_est_ignore_au_mvp() {
        for s in [Status::Queued, Status::InFlight] {
            assert_eq!(
                next_status(s, StatusEvent::AckReceived(AckStatus::Read)),
                None
            );
        }
    }

    #[test]
    fn annulation_seulement_avant_hand_off() {
        assert_eq!(
            next_status(Status::Queued, StatusEvent::Cancel),
            Some(Status::Cancelled)
        );
        assert_eq!(next_status(Status::InFlight, StatusEvent::Cancel), None);
    }

    #[test]
    fn expiration_depuis_queued_et_in_flight() {
        for s in [Status::Queued, Status::InFlight] {
            assert_eq!(
                next_status(s, StatusEvent::TtlElapsed),
                Some(Status::Expired)
            );
        }
    }

    #[test]
    fn ack_sur_queued_donne_delivered() {
        let ack = StatusEvent::AckReceived(AckStatus::Delivered);
        assert_eq!(next_status(Status::Queued, ack), Some(Status::Delivered));
    }

    #[test]
    fn les_statuts_terminaux_sont_absorbants() {
        for s in Status::ALL.into_iter().filter(|s| s.is_terminal()) {
            for ev in EVENTS {
                assert_eq!(next_status(s, ev), None, "{s:?} + {ev:?}");
            }
        }
    }

    #[test]
    fn codes_et_noms_d_evenements() {
        for s in Status::ALL {
            assert_eq!(Status::from_u8(s.to_u8()), Some(s));
            assert_eq!(Status::from_code(s.as_str()), Some(s));
        }
        assert_eq!(Status::from_u8(5), None);
        assert_eq!(Status::from_code("read"), None);
        assert_eq!(Status::InFlight.event_name(), "msg.handed_off");
        assert_eq!(Status::Queued.event_name(), "msg.queued");
        assert_eq!(Status::Delivered.event_name(), "msg.delivered");
        assert_eq!(Status::Expired.event_name(), "msg.expired");
        assert_eq!(Status::Cancelled.event_name(), "msg.cancelled");
        let change = StatusChange {
            msg_uuid: [0; MSG_UUID_LEN],
            from: Some(Status::InFlight),
            to: Status::Delivered,
            at_ms: 1,
        };
        assert_eq!(change.event_name(), "msg.delivered");
    }

    fn event() -> impl Strategy<Value = StatusEvent> {
        proptest::sample::select(EVENTS.to_vec())
    }

    proptest! {
        /// Monotonie : quelle que soit la suite d'événements, le rang ne
        /// décroît jamais et un statut terminal ne change plus.
        #[test]
        fn aucune_regression_d_etat(events in proptest::collection::vec(event(), 0..64)) {
            let mut current = Status::Queued;
            for ev in events {
                if let Some(next) = next_status(current, ev) {
                    prop_assert!(!current.is_terminal(), "{current:?} terminal a bougé");
                    prop_assert!(next.rank() > current.rank(), "{current:?} -> {next:?}");
                    current = next;
                }
            }
        }
    }
}
