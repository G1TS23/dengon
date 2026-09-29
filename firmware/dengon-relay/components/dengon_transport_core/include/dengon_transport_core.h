// ---------------------------------------------------------------------------
// dengon_transport_core — la moitié PURE du transport NimBLE (US-220).
//
// Le contrat est celui d'US-105 : crates/dengon-ble/src/transport.rs. Ce
// fichier en est le miroir C, et ce composant en porte TOUTE la sémantique :
// état démarré / non démarré, attribution des LinkId, file d'événements
// ordonnée, validation de send / broadcast, motif de déconnexion.
//
// Il ne connaît PAS NimBLE : il raisonne sur des `conn_handle` (uint16_t) et
// des codes HCI bruts (uint8_t). C'est ce qui permet de le compiler pour la
// cible `linux` d'ESP-IDF et de lui faire passer, sur l'hôte et en CI, le
// portage C des 12 cas de la suite de conformité Rust
// (crates/dengon-ble/src/conformance.rs). La glue radio vit dans
// main/transport_nimble.c et ne fait que traduire les événements GAP en
// appels à ce composant.
//
// ⚠ PAS THREAD-SAFE. La glue l'enveloppe d'un mutex : les callbacks NimBLE
//   tournent dans la tâche host, poll/send dans la tâche applicative.
// ---------------------------------------------------------------------------
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Taille de la table de liens. Majorant de CONFIG_BT_NIMBLE_MAX_CONNECTIONS. */
#define DENGON_TC_MAX_LINKS 8

/** Profondeur de la file d'événements (cycle de vie + trames). */
#define DENGON_TC_EVQ_CAP 32

/** En-tête d'une PDU ATT write/notify : opcode (1) + handle (2). */
#define DENGON_TC_ATT_OVERHEAD 3

/** MTU ATT minimal garanti par le Core Bluetooth : 20 octets utiles. */
#define DENGON_TC_ATT_MTU_MIN 23

/** MTU ATT maximal visé (docs/powl/03 §6.3). */
#define DENGON_TC_ATT_MTU_MAX 517

/**
 * Plus grande trame acceptée, en émission comme au réassemblage (US-312).
 * Depuis l'US-312 une trame peut couvrir plusieurs morceaux ; la borne reste
 * celle d'une trame du relais (`relay::FRAME_MAX`, 514) pour garder le
 * tampon de réassemblage petit (tas serré, docs/suivi/modules/firmware-relay.md).
 */
#define DENGON_TC_FRAME_MAX (DENGON_TC_ATT_MTU_MAX - DENGON_TC_ATT_OVERHEAD)

/**
 * Fragmentation BLE (L1), format de l'app Android (FragmentationBle.kt) :
 *
 *   morceau = en-tête:u8 ‖ données, 1 morceau = 1 PDU ATT (≤ MTU - 3)
 *   en-tête : bit 7 = SUITE (d'autres morceaux suivent), bits 0-6 = 0
 *
 * Une trame vide est un morceau sans données. GATT garantit l'ordre sur une
 * connexion : ni numéro ni longueur totale.
 */
#define DENGON_TC_CHUNK_SUITE 0x80
#define DENGON_TC_CHUNK_HDR   1
/**
 * Morceau d'abandon : l'émetteur n'a pas pu finir la trame en cours (pool de
 * mbufs vide…). Le récepteur jette son partiel sans compter d'erreur. Sans
 * lui, le morceau suivant (début valide de la trame d'après) serait collé au
 * partiel : deux trames perdues sans détection (revue PR #129 point 2). Les
 * bits réservés le rendent invalide pour un réassembleur plus ancien, qui
 * abandonne aussi.
 */
#define DENGON_TC_CHUNK_ABORT 0x40

/** Codes HCI de déconnexion utiles (Core Spec Vol 1, Part F). */
#define DENGON_HCI_CONN_SPVN_TMO      0x08 /* supervision timeout           */
#define DENGON_HCI_REM_USER_CONN_TERM 0x13 /* le pair a fermé               */
#define DENGON_HCI_RD_CONN_TERM_RESRC 0x14 /* le pair ferme, faute de place */
#define DENGON_HCI_RD_CONN_TERM_PWROFF 0x15 /* le pair s'éteint proprement  */
#define DENGON_HCI_CONN_TERM_LOCAL    0x16 /* notre host a fermé            */

/** Miroir de `LinkId` : compteur monotone, jamais réattribué, jamais 0. */
typedef uint64_t dengon_link_id_t;

/** Miroir de `TransportError`. `OK` n'existe pas en Rust (c'est `Ok(())`). */
typedef enum {
    DENGON_TR_OK = 0,
    DENGON_TR_NOT_STARTED,
    DENGON_TR_ALREADY_STARTED,
    DENGON_TR_UNKNOWN_PEER,
    DENGON_TR_FRAME_TOO_LARGE,
    DENGON_TR_TOO_MANY_CONNECTIONS,
    DENGON_TR_BACKEND,
} dengon_tr_err_t;

/** Miroir de `DisconnectReason`. */
typedef enum {
    DENGON_DISC_PROPRE = 0,
    DENGON_DISC_BRUTALE,
    DENGON_DISC_LOCALE,
} dengon_disconnect_reason_t;

/** Miroir des variantes de `TransportEvent`. */
typedef enum {
    DENGON_EVT_PEER_CONNECTED = 0,
    DENGON_EVT_PEER_DISCONNECTED,
    DENGON_EVT_FRAME_RECEIVED,
} dengon_event_kind_t;

/**
 * Miroir de `TransportEvent`.
 *
 * Pour FRAME_RECEIVED, `u.frame.bytes` est un tampon alloué (malloc) que
 * l'événement POSSÈDE : le libérer par dengon_tc_event_free() une fois lu.
 */
typedef struct {
    dengon_event_kind_t kind;
    dengon_link_id_t    link;
    union {
        struct {
            bool    has_rssi;
            int16_t rssi;
        } connected;
        dengon_disconnect_reason_t reason;
        struct {
            uint8_t *bytes;
            size_t   len;
        } frame;
    } u;
} dengon_transport_event_t;

/** Miroir de `TransportConfig`. */
typedef struct {
    uint8_t  local_peer_id[8];
    bool     advertise;
    bool     scan;
    size_t   max_connections;
    uint16_t preferred_mtu;
} dengon_transport_config_t;

/** Rôle GATT tenu par NOTRE nœud sur un lien donné. */
typedef enum {
    DENGON_ROLE_CENTRAL = 0, /* on a initié : on écrit sur le CHAR_RX du pair */
    DENGON_ROLE_PERIPHERAL,  /* le pair a initié : on notifie sur notre CHAR_TX */
} dengon_link_role_t;

/** Ce dont la glue a besoin pour émettre sur un lien, hors du mutex. */
typedef struct {
    dengon_link_id_t   link;
    uint16_t           conn_handle;
    dengon_link_role_t role;
    uint16_t           peer_rx_handle; /* significatif en rôle CENTRAL seulement */
    uint16_t           mtu;
} dengon_tc_route_t;

/** Un lien de la table. Interne, exposé pour l'allocation statique. */
typedef struct {
    bool               used;
    bool               announced; /* PeerConnected déjà mis en file */
    dengon_link_id_t   id;
    uint16_t           conn_handle;
    dengon_link_role_t role;
    uint16_t           mtu;
    uint16_t           peer_rx_handle;
    bool               has_rssi;
    int16_t            rssi;
    /* Réassemblage L1 en cours : alloué au premier morceau SUITE, libéré à
       la fin de la trame ou à la fermeture. NULL hors réassemblage. */
    uint8_t           *rx_part;
    size_t             rx_len;
    /* Trame trop longue abandonnée : ses morceaux restants sont ignorés
       jusqu'au dernier (sans SUITE), au lieu de passer pour une trame neuve
       (revue PR #129 point 3). */
    bool               rx_skip;
} dengon_tc_link_t;

/** L'état complet du transport. Allocation statique, aucun malloc hors trames. */
typedef struct {
    bool                      started;
    dengon_transport_config_t cfg;
    size_t                    max_links; /* quota effectif, après bornage */
    dengon_link_id_t          next_id;
    dengon_tc_link_t          links[DENGON_TC_MAX_LINKS];
    dengon_transport_event_t  q[DENGON_TC_EVQ_CAP];
    size_t                    q_head;
    size_t                    q_len;
    uint32_t                  dropped_frames;
    uint32_t                  dropped_lifecycle;
    uint32_t                  bad_chunks; /* morceaux L1 invalides ou trame trop longue */
} dengon_tc_t;

/* --- Cycle de vie ---------------------------------------------------------- */

/**
 * Remet l'état à neuf : non démarré, aucun lien, file vide. Libère les
 * réassemblages en cours : `tc` doit donc être déjà initialisé ou mis à zéro
 * (cas d'une variable statique).
 */
void dengon_tc_init(dengon_tc_t *tc);

/**
 * Démarre. Borne cfg->max_connections à min(hw_max_links, DENGON_TC_MAX_LINKS) ;
 * le quota effectif est écrit dans *effective_max (peut être NULL).
 *
 * @return OK, ou ALREADY_STARTED — ce n'est pas une reconfiguration.
 */
dengon_tr_err_t dengon_tc_start(dengon_tc_t *tc, const dengon_transport_config_t *cfg,
                                size_t hw_max_links, size_t *effective_max);

/* --- Côté radio : appelé par la glue NimBLE --------------------------------- */

/**
 * Ouvre un lien pour une connexion GAP qui vient de s'établir. Le lien n'est
 * PAS encore annoncé : PeerConnected n'est émis que par dengon_tc_link_announce()
 * (ou paresseusement à la première trame reçue).
 *
 * @return OK ; NOT_STARTED ; TOO_MANY_CONNECTIONS si le quota est atteint ;
 *         BACKEND si ce conn_handle est déjà ouvert (incohérence de la pile).
 */
dengon_tr_err_t dengon_tc_link_open(dengon_tc_t *tc, uint16_t conn_handle,
                                    dengon_link_role_t role, dengon_link_id_t *out_id);

/** MTU ATT négocié. Ignoré si le conn_handle est inconnu ou si mtu < 23. */
void dengon_tc_link_set_mtu(dengon_tc_t *tc, uint16_t conn_handle, uint16_t mtu);

/** Handle de valeur du CHAR_RX du pair (rôle central). */
void dengon_tc_link_set_peer_rx(dengon_tc_t *tc, uint16_t conn_handle, uint16_t handle);

/** RSSI mesuré au scan, reporté dans PeerConnected. */
void dengon_tc_link_set_rssi(dengon_tc_t *tc, uint16_t conn_handle, int16_t rssi);

/**
 * Met PeerConnected en file si ce n'est déjà fait.
 * @return true si l'événement vient d'être émis.
 */
bool dengon_tc_link_announce(dengon_tc_t *tc, uint16_t conn_handle);

/**
 * Une trame complète est arrivée : copie des octets et FrameReceived en file.
 * Un lien pas encore annoncé l'est d'abord (ordre par lien préservé).
 *
 * @return OK ; UNKNOWN_PEER si le conn_handle n'a pas de lien (trame jetée) ;
 *         BACKEND si la file est pleine ou l'allocation échoue (trame jetée,
 *         comptée dans dropped_frames).
 */
dengon_tr_err_t dengon_tc_on_rx(dengon_tc_t *tc, uint16_t conn_handle,
                                const uint8_t *bytes, size_t len);

/**
 * Un morceau L1 (une PDU ATT) est arrivé : réassemble, puis passe la trame
 * complète à dengon_tc_on_rx(). C'est ce qu'appelle la glue radio.
 *
 * Un morceau vide, un en-tête aux bits réservés non nuls ou une trame qui
 * dépasserait DENGON_TC_FRAME_MAX abandonnent le réassemblage en cours
 * (compté dans bad_chunks) ; le morceau suivant repart d'une trame neuve.
 *
 * @return OK (trame complète remise ou morceau mis de côté) ; UNKNOWN_PEER ;
 *         BACKEND (morceau invalide, allocation, file pleine) ;
 *         FRAME_TOO_LARGE.
 */
dengon_tr_err_t dengon_tc_on_chunk(dengon_tc_t *tc, uint16_t conn_handle,
                                   const uint8_t *chunk, size_t len);

/**
 * Ferme le lien. Émet PeerDisconnected (motif tiré du code HCI) SEULEMENT si
 * le lien avait été annoncé : un lien jamais vu du cœur disparaît en silence.
 *
 * @return true si un PeerDisconnected a été mis en file.
 */
bool dengon_tc_link_close(dengon_tc_t *tc, uint16_t conn_handle, uint8_t hci_reason);

/** Nombre de liens ouverts (annoncés ou non) — sert au quota d'annonce. */
size_t dengon_tc_link_count(const dengon_tc_t *tc);

/** Ce conn_handle a-t-il un lien ouvert ? */
bool dengon_tc_has_conn(const dengon_tc_t *tc, uint16_t conn_handle);

/** Rôle du lien de ce conn_handle. @return false si inconnu. */
bool dengon_tc_link_role(const dengon_tc_t *tc, uint16_t conn_handle,
                         dengon_link_role_t *out_role);

/* --- Côté cœur : miroir de poll / send / broadcast ------------------------- */

/**
 * Retire jusqu'à `cap` événements, dans l'ordre. Rend 0 avant start.
 * Les événements sont CONSOMMÉS.
 */
size_t dengon_tc_poll(dengon_tc_t *tc, dengon_transport_event_t *out, size_t cap);

/** Libère le tampon d'un FRAME_RECEIVED (sans effet sur les autres). */
void dengon_tc_event_free(dengon_transport_event_t *ev);

/**
 * Valide un send et rend la route à utiliser. N'émet rien.
 *
 * La trame est ensuite émise en dengon_tc_chunk_count() morceaux L1 : le MTU
 * du lien ne borne plus la trame (US-312).
 *
 * @return OK ; NOT_STARTED ; UNKNOWN_PEER si le lien est fermé ou pas encore
 *         annoncé ; FRAME_TOO_LARGE si len > DENGON_TC_FRAME_MAX
 *         (*max_out renseigné).
 */
dengon_tr_err_t dengon_tc_route_for_send(const dengon_tc_t *tc, dengon_link_id_t link,
                                         size_t len, dengon_tc_route_t *route,
                                         size_t *max_out);

/**
 * Valide un broadcast et rend les routes de tous les liens annoncés (la
 * trame y part en morceaux L1, quel que soit leur MTU).
 *
 * @return OK (même avec *n_out == 0) ; NOT_STARTED ; FRAME_TOO_LARGE si
 *         len > DENGON_TC_FRAME_MAX.
 */
dengon_tr_err_t dengon_tc_routes_for_broadcast(const dengon_tc_t *tc, size_t len,
                                               dengon_tc_route_t *routes, size_t cap,
                                               size_t *n_out);

/* --- Découpage L1 à l'émission ---------------------------------------------- */

/** Octets de trame portés par un morceau sur un lien de ce MTU (≥ 1). */
size_t dengon_tc_chunk_payload(uint16_t mtu);

/** Nombre de morceaux d'une trame de `len` octets (une trame vide : 1). */
size_t dengon_tc_chunk_count(size_t len, uint16_t mtu);

/**
 * En-tête du morceau `index` (0-based) et position de ses données dans la
 * trame : la glue émet `header ‖ frame[*off .. *off + *n]`, sans copie.
 */
uint8_t dengon_tc_chunk_at(size_t len, uint16_t mtu, size_t index, size_t *off, size_t *n);

/* --- Utilitaires ------------------------------------------------------------ */

/** Motif de déconnexion correspondant à un code HCI brut. */
dengon_disconnect_reason_t dengon_tc_map_hci_reason(uint8_t hci_reason);

/** Libellé français d'une erreur, pour les journaux. */
const char *dengon_tr_err_str(dengon_tr_err_t err);

/** Libellé d'un motif de déconnexion (« Propre », « Brutale », « Locale »). */
const char *dengon_disc_reason_str(dengon_disconnect_reason_t reason);

#ifdef __cplusplus
}
#endif
