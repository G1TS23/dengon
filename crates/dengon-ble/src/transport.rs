//! Le contrat `Transport` — l'unique couture entre le cœur et la radio.
//!
//! Référence : [`docs/synthese/04-architecture.md`] §3. Ce module ne contient
//! **aucune** logique : uniquement les types que toutes les plateformes
//! partagent. Les implémentations vivent ailleurs (`btleplug` desktop,
//! `AndroidTransport.kt`, `transport_nimble.c`).
//!
//! # Ce que `Transport` ne fait pas
//!
//! - **Il ne connaît pas le protocole.** Il transporte des `Vec<u8>` opaques.
//!   La fragmentation *protocole* (paquet plus grand que le MTU) est le travail
//!   de `dengon-core::protocol` ; seule la fragmentation *BLE* est à la charge
//!   de l'implémentation.
//! - **Il ne route pas.** Le choix du pair à qui envoyer appartient à
//!   `dengon-core::sync`.
//! - **Il ne chiffre pas.** Les octets arrivent déjà scellés.
//!
//! [`docs/synthese/04-architecture.md`]: ../../../docs/synthese/04-architecture.md

use core::fmt;

/// Identifiant d'un **lien** BLE ouvert, attribué par l'implémentation.
///
/// # Ce n'est pas un `peerID`
///
/// Le `peerID` du protocole (décision A-8 : `SHA-256(pub_static)[0..8]`, 8
/// octets) identifie un **nœud**, de façon stable et pseudonyme. Un `LinkId`
/// identifie une **connexion**, il est local au processus et sans valeur
/// cryptographique. Deux connexions successives vers le même nœud portent deux
/// `LinkId` différents.
///
/// C'est volontaire : le transport ne sait pas *qui* est au bout du lien tant
/// que le handshake applicatif n'a pas eu lieu. Faire porter un `peerID` à
/// `Transport` reviendrait à lui faire faire de la crypto.
///
/// # Contrat pour les implémentations
///
/// Un `LinkId` **ne doit jamais être réutilisé** pour un autre pair au cours
/// d'une même exécution, même après déconnexion. Sinon une trame en retard sur
/// l'ancien lien serait attribuée au nouveau pair — et la déduplication en
/// amont ne rattraperait pas l'erreur, puisqu'elle raisonne sur le `msgID`, pas
/// sur l'origine. Un simple compteur monotone suffit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinkId(u64);

impl LinkId {
    /// Construit un `LinkId` à partir d'un entier brut.
    ///
    /// Réservé aux implémentations de [`Transport`], qui sont seules à savoir
    /// attribuer ces identifiants sans collision.
    #[must_use]
    pub const fn new(brut: u64) -> Self {
        Self(brut)
    }

    /// La valeur brute, pour journalisation ou passage par FFI.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for LinkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "link#{}", self.0)
    }
}

/// Réglages passés à [`Transport::start`].
///
/// Volontairement **minimal**. Les constantes de protocole (`SERVICE_UUID`,
/// `CHAR_RX_UUID`, `CHAR_TX_UUID`, tailles de fragment) ne figurent pas ici :
/// elles arrivent avec `protocol::consts` (US-108) et sont les mêmes pour
/// toutes les implémentations, donc elles n'ont rien à faire dans une
/// configuration d'exécution. Ce qui est ici, c'est ce qui **varie d'un nœud à
/// l'autre**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportConfig {
    /// `peerID` local (8 octets, décision A-8).
    ///
    /// L'implémentation en annonce les 4 premiers octets en *manufacturer
    /// data*, et s'en sert pour la **règle anti-boucle de connexion** :
    /// lorsque deux nœuds se découvrent, celui dont le `peerID` est le plus
    /// petit initie la connexion GATT
    /// (`docs/synthese/05-protocole-et-trame.md` §6). Sans cette règle, les
    /// deux se connectent simultanément et l'un des deux liens est gaspillé.
    ///
    /// Typé `[u8; 8]` et non `PeerId` : ce type nommé appartient à
    /// `protocol::types` (US-108), livrée en parallèle. Le remplacer plus tard
    /// est un changement de signature, pas de sémantique.
    pub local_peer_id: [u8; 8],

    /// Annoncer le service `dengon` (rôle périphérique).
    pub advertise: bool,

    /// Scanner les pairs (rôle central).
    ///
    /// Un nœud normal a les deux à `true` : chaque nœud est serveur **et**
    /// client GATT. Les dissocier sert aux tests et aux bancs d'essai, où l'on
    /// veut forcer un sens de connexion.
    pub scan: bool,

    /// Nombre maximal de liens simultanés.
    ///
    /// Borne l'usage mémoire, qui est la ressource critique sur ESP32-WROOM
    /// (décision A-4 : pas de PSRAM). Au-delà, l'implémentation refuse les
    /// nouvelles connexions plutôt que de saturer.
    pub max_connections: usize,

    /// MTU ATT à négocier à la connexion.
    ///
    /// `517` est la valeur visée ; en cas de refus du pair, BLE retombe sur 23
    /// (20 octets utiles). L'implémentation **n'échoue pas** pour autant :
    /// elle fragmente. La valeur réellement obtenue sur les appareils cibles
    /// est mesurée par le Spike C (US-103, sujet C-1).
    pub preferred_mtu: u16,
}

impl Default for TransportConfig {
    /// Configuration d'un nœud ordinaire : double rôle, 8 liens, MTU 517.
    ///
    /// `local_peer_id` vaut `[0; 8]`, qui n'est **pas** un `peerID` valide :
    /// c'est au nœud de le renseigner à partir de son identité.
    fn default() -> Self {
        Self {
            local_peer_id: [0; 8],
            advertise: true,
            scan: true,
            max_connections: 8,
            preferred_mtu: 517,
        }
    }
}

/// Pourquoi un lien s'est fermé.
///
/// # Pourquoi ce type existe
///
/// `04-architecture.md` §3 décrit `PeerDisconnected { peer_link_id }`, sans
/// motif. Le motif est ajouté ici parce que la couche du dessus en a besoin
/// pour décider : une **coupure brutale** justifie de retenter tout de suite
/// (le pair est peut-être encore à portée), un **départ propre** non. Sans
/// cette information, `sync` ne peut que deviner. Écart consigné dans
/// `docs/suivi/03-ecarts-conception.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    /// Le pair s'est déconnecté proprement (déconnexion GATT annoncée).
    Propre,

    /// Le lien est tombé sans prévenir : pair hors de portée, radio coupée,
    /// batterie vide, *supervision timeout* BLE.
    ///
    /// C'est le cas **normal** dans un réseau maillé mobile, pas une anomalie.
    Brutale,

    /// C'est *notre* nœud qui a fermé le lien (quota `max_connections`
    /// atteint, arrêt du transport, politique locale).
    Locale,
}

/// Ce qu'un transport remonte au cœur.
///
/// Ordre garanti **par lien** : pour un `LinkId` donné, les événements
/// arrivent dans l'ordre où ils se sont produits. Aucun ordre n'est garanti
/// *entre* liens différents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportEvent {
    /// Un lien est établi et utilisable.
    PeerConnected {
        /// Le lien qui vient de s'ouvrir.
        peer_link_id: LinkId,
        /// Puissance du signal en dBm, si la plateforme la donne.
        ///
        /// `None` est fréquent : Android ne fournit le RSSI qu'à la demande, et
        /// NimBLE pas du tout sur une connexion entrante.
        rssi: Option<i16>,
    },

    /// Un lien est fermé. Le `LinkId` est mort : l'envoyer à
    /// [`Transport::send`] rend [`TransportError::UnknownPeer`].
    PeerDisconnected {
        /// Le lien qui vient de se fermer.
        peer_link_id: LinkId,
        /// Ce qui l'a fermé.
        reason: DisconnectReason,
    },

    /// Une trame applicative complète est arrivée.
    ///
    /// « Complète » = la fragmentation **BLE** est déjà réassemblée par
    /// l'implémentation. Ce n'est pas la fragmentation protocole : un paquet
    /// `dengon` plus grand que le MTU arrive en plusieurs `FrameReceived`, et
    /// c'est `protocol` qui les recolle.
    FrameReceived {
        /// Le lien qui a livré la trame.
        peer_link_id: LinkId,
        /// Les octets, opaques pour le transport.
        bytes: Vec<u8>,
    },
}

/// Ce qui peut échouer dans un transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// Appel à `send` ou `broadcast` avant [`Transport::start`].
    NotStarted,

    /// [`Transport::start`] appelé alors que le transport tourne déjà.
    AlreadyStarted,

    /// Le `LinkId` n'existe pas, ou plus.
    ///
    /// Le cas courant est bénin : le pair s'est déconnecté entre le moment où
    /// le cœur a lu l'événement et celui où il a répondu. **À traiter comme
    /// une condition normale**, pas comme un bug.
    UnknownPeer(LinkId),

    /// La trame dépasse ce que l'implémentation accepte en une fois.
    FrameTooLarge {
        /// Taille proposée, en octets.
        size: usize,
        /// Maximum accepté, en octets.
        max: usize,
    },

    /// Le quota [`TransportConfig::max_connections`] est atteint.
    TooManyConnections {
        /// Le plafond configuré.
        max: usize,
    },

    /// Échec propre à la pile BLE sous-jacente (BlueZ, Android, NimBLE).
    ///
    /// Volontairement une `String` : chaque plateforme a ses propres codes, et
    /// les énumérer ici ferait fuiter la plateforme dans le contrat partagé.
    Backend(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotStarted => write!(f, "le transport n'est pas démarré"),
            Self::AlreadyStarted => write!(f, "le transport est déjà démarré"),
            Self::UnknownPeer(id) => write!(f, "pair inconnu ou déconnecté : {id}"),
            Self::FrameTooLarge { size, max } => {
                write!(f, "trame de {size} octets, maximum {max}")
            }
            Self::TooManyConnections { max } => {
                write!(f, "quota de connexions atteint ({max})")
            }
            Self::Backend(details) => write!(f, "erreur de la pile BLE : {details}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// Raccourci local : tout ce module renvoie la même erreur.
pub type Result<T> = core::result::Result<T, TransportError>;

/// Abstraction d'un lien BLE, implémentée nativement par plateforme.
///
/// # Modèle d'exécution
///
/// `Transport` est **non bloquant et sans `async`**. La radio vit dans son
/// propre fil (ou sa propre tâche FreeRTOS) ; elle dépose ses événements dans
/// une file que [`poll`](Transport::poll) vide. Le cœur appelle `poll` dans sa
/// boucle, à son rythme.
///
/// Ce choix est ce qui rend le contrat portable : il n'impose pas d'exécuteur
/// `async` à Android ni à l'ESP32, et il traverse le FFI sans difficulté. Le
/// prix à payer est que l'appelant doit interroger régulièrement.
///
/// `Send` est requis parce que le cœur fait souvent tourner sa boucle sur un
/// fil dédié. `Sync` ne l'est pas : toutes les méthodes prennent `&mut self`.
///
/// # Comportement en déconnexion brutale
///
/// C'est le cas le plus fréquent sur le terrain — quelqu'un s'éloigne, un
/// téléphone se verrouille. Toute implémentation **doit** :
///
/// 1. émettre exactement un
///    [`PeerDisconnected`](TransportEvent::PeerDisconnected) avec
///    [`DisconnectReason::Brutale`], au plus tard au *supervision timeout* BLE
///    (quelques secondes) ;
/// 2. **livrer d'abord les trames déjà reçues** sur ce lien. Une trame complète
///    arrivée avant la coupure est valide : la jeter ferait perdre un message
///    que le réseau a déjà transporté ;
/// 3. jeter silencieusement les trames **partielles** (fragmentation BLE
///    incomplète) — elles ne sont pas récupérables ;
/// 4. faire échouer tout [`send`](Transport::send) ultérieur sur ce `LinkId`
///    avec [`UnknownPeer`](TransportError::UnknownPeer), **sans paniquer** ;
/// 5. ne plus jamais émettre d'événement portant ce `LinkId`.
///
/// Une trame `send` acceptée juste avant la coupure peut être perdue **sans
/// erreur** : BLE accuse réception au niveau du lien, pas de l'application. Le
/// transport ne garantit donc **pas** la livraison — c'est l'accusé de
/// réception applicatif (`sync`) qui la constate.
///
/// # Ordre des appels
///
/// [`start`](Transport::start) d'abord. Avant lui, `send` et `broadcast`
/// rendent [`NotStarted`](TransportError::NotStarted) et `poll` rend un `Vec`
/// vide. Un second `start` rend
/// [`AlreadyStarted`](TransportError::AlreadyStarted) : ce n'est pas une
/// reconfiguration.
pub trait Transport: Send {
    /// Démarre l'annonce du service `dengon` et/ou le scan des pairs.
    ///
    /// # Errors
    ///
    /// [`AlreadyStarted`](TransportError::AlreadyStarted) si le transport
    /// tourne déjà ; [`Backend`](TransportError::Backend) si la pile BLE refuse
    /// de démarrer (adaptateur absent, permission manquante).
    fn start(&mut self, cfg: TransportConfig) -> Result<()>;

    /// Retire et rend les événements accumulés depuis le dernier appel.
    ///
    /// Ne bloque jamais, et **ne peut pas échouer** : un `Vec` vide veut dire
    /// « rien de neuf ». Les événements sont **consommés** — deux `poll` de
    /// suite ne rendent pas deux fois la même chose.
    ///
    /// Avant [`start`](Transport::start), rend un `Vec` vide. La signature
    /// infaillible est celle de `04-architecture.md` §3, et elle est
    /// délibérée : `poll` est appelé en boucle et traverse le FFI vers Kotlin
    /// et C, où un type résultat coûte cher pour un cas qui ne se produit qu'en
    /// cas d'erreur de programmation. C'est [`send`](Transport::send) qui
    /// signale un transport non démarré.
    fn poll(&mut self) -> Vec<TransportEvent>;

    /// Envoie une trame à un pair connecté.
    ///
    /// Le retour `Ok` signifie « remis à la pile BLE », **pas** « reçu par le
    /// pair ». Voir la section sur la déconnexion brutale.
    ///
    /// # Errors
    ///
    /// [`NotStarted`](TransportError::NotStarted),
    /// [`UnknownPeer`](TransportError::UnknownPeer) si le lien est fermé,
    /// [`FrameTooLarge`](TransportError::FrameTooLarge),
    /// [`Backend`](TransportError::Backend).
    fn send(&mut self, peer_link_id: LinkId, bytes: &[u8]) -> Result<()>;

    /// Diffuse une trame à tous les pairs connectés, au mieux.
    ///
    /// Sert à `ANNOUNCE` et au gossip. « Au mieux » est à prendre au pied de la
    /// lettre : si l'envoi échoue vers un pair, les autres sont quand même
    /// servis et l'appel rend `Ok`. Un `broadcast` sans aucun pair connecté est
    /// un succès, pas une erreur — c'est l'état normal d'un nœud isolé.
    ///
    /// # Errors
    ///
    /// [`NotStarted`](TransportError::NotStarted),
    /// [`FrameTooLarge`](TransportError::FrameTooLarge) — les deux se
    /// constatent sans toucher à la radio.
    fn broadcast(&mut self, bytes: &[u8]) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::{DisconnectReason, LinkId, TransportConfig, TransportError};

    #[test]
    fn un_link_id_s_affiche_lisiblement() {
        assert_eq!(LinkId::new(42).to_string(), "link#42");
    }

    #[test]
    fn la_valeur_brute_du_link_id_fait_l_aller_retour() {
        assert_eq!(LinkId::new(7).get(), 7);
    }

    #[test]
    fn la_config_par_defaut_decrit_un_noeud_a_double_role() {
        let cfg = TransportConfig::default();
        assert!(cfg.advertise, "un nœud doit être serveur GATT");
        assert!(cfg.scan, "un nœud doit aussi être client GATT");
        assert_eq!(cfg.preferred_mtu, 517);
        assert_eq!(
            cfg.local_peer_id, [0; 8],
            "peerID non renseigné par défaut : c'est au nœud de le faire"
        );
    }

    #[test]
    fn les_erreurs_se_lisent_en_francais() {
        let e = TransportError::FrameTooLarge {
            size: 600,
            max: 512,
        };
        assert_eq!(e.to_string(), "trame de 600 octets, maximum 512");
        assert_eq!(
            TransportError::UnknownPeer(LinkId::new(3)).to_string(),
            "pair inconnu ou déconnecté : link#3"
        );
    }

    #[test]
    fn les_motifs_de_deconnexion_sont_distincts() {
        assert_ne!(DisconnectReason::Propre, DisconnectReason::Brutale);
        assert_ne!(DisconnectReason::Brutale, DisconnectReason::Locale);
    }
}
