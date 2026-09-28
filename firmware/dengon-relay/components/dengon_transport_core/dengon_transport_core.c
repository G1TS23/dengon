// ---------------------------------------------------------------------------
// dengon_transport_core — sémantique du contrat `Transport` en C pur (US-220).
// Voir l'en-tête pour le rôle de ce composant et ses limites.
// ---------------------------------------------------------------------------
#include <stdlib.h>
#include <string.h>

#include "dengon_transport_core.h"

/* --- File d'événements ------------------------------------------------------
 *
 * FIFO circulaire UNIQUE pour tous les liens : c'est elle qui garantit l'ordre
 * par lien exigé par le contrat, et en particulier la règle n°2 de la
 * déconnexion brutale — une trame reçue avant la coupure est déjà dans la file
 * quand PeerDisconnected y entre, donc elle sort avant lui.
 *
 * Contre-pression (docs/synthese/08-relais-esp32.md §4) : quand la file se
 * remplit, on jette des TRAMES entrantes, jamais un événement de cycle de vie.
 * Les trames n'ont droit qu'à (capacité - réserve) cases ; la réserve
 * (2 événements par lien possible) reste aux PeerConnected / PeerDisconnected.
 * Sans elle, un pair bavard remplirait la file et le PeerDisconnected du
 * contrat — « exactement un » — pourrait se perdre.
 */

static size_t
lifecycle_reserve(const dengon_tc_t *tc)
{
    return 2 * tc->max_links;
}

static bool
q_push(dengon_tc_t *tc, const dengon_transport_event_t *ev)
{
    size_t limit = DENGON_TC_EVQ_CAP;

    if (ev->kind == DENGON_EVT_FRAME_RECEIVED) {
        size_t reserve = lifecycle_reserve(tc);
        limit = reserve < DENGON_TC_EVQ_CAP ? DENGON_TC_EVQ_CAP - reserve : 0;
    }
    if (tc->q_len >= limit) {
        return false;
    }

    tc->q[(tc->q_head + tc->q_len) % DENGON_TC_EVQ_CAP] = *ev;
    tc->q_len++;
    return true;
}

/* --- Table de liens ---------------------------------------------------------
 *
 * Indexée par conn_handle côté radio, par LinkId côté cœur. Les deux ne se
 * confondent jamais : NimBLE RÉUTILISE un conn_handle dès qu'il est libéré,
 * alors qu'un LinkId ne doit jamais l'être (rustdoc de `LinkId`). La
 * correspondance est donc rompue à la fermeture, et une nouvelle connexion sur
 * le même conn_handle reçoit un LinkId neuf tiré du compteur monotone.
 */

static dengon_tc_link_t *
link_by_conn(dengon_tc_t *tc, uint16_t conn_handle)
{
    for (size_t i = 0; i < DENGON_TC_MAX_LINKS; i++) {
        if (tc->links[i].used && tc->links[i].conn_handle == conn_handle) {
            return &tc->links[i];
        }
    }
    return NULL;
}

static const dengon_tc_link_t *
link_by_conn_const(const dengon_tc_t *tc, uint16_t conn_handle)
{
    return link_by_conn((dengon_tc_t *)tc, conn_handle);
}

static const dengon_tc_link_t *
link_by_id(const dengon_tc_t *tc, dengon_link_id_t id)
{
    for (size_t i = 0; i < DENGON_TC_MAX_LINKS; i++) {
        if (tc->links[i].used && tc->links[i].id == id) {
            return &tc->links[i];
        }
    }
    return NULL;
}

static size_t
frame_max(const dengon_tc_link_t *l)
{
    return (size_t)l->mtu - DENGON_TC_ATT_OVERHEAD;
}

static void
route_of(const dengon_tc_link_t *l, dengon_tc_route_t *r)
{
    r->link = l->id;
    r->conn_handle = l->conn_handle;
    r->role = l->role;
    r->peer_rx_handle = l->peer_rx_handle;
    r->mtu = l->mtu;
}

/* --- Cycle de vie ---------------------------------------------------------- */

void
dengon_tc_init(dengon_tc_t *tc)
{
    memset(tc, 0, sizeof(*tc));
    /* 0 est réservé : un LinkId nul trahit une structure non initialisée. */
    tc->next_id = 1;
}

dengon_tr_err_t
dengon_tc_start(dengon_tc_t *tc, const dengon_transport_config_t *cfg,
                size_t hw_max_links, size_t *effective_max)
{
    size_t max;

    if (tc->started) {
        return DENGON_TR_ALREADY_STARTED;
    }

    /* Le défaut Rust vaut 8 liens ; le contrôleur de l'ESP32 n'en tient que
       CONFIG_BT_NIMBLE_MAX_CONNECTIONS. On borne plutôt que d'échouer : un
       nœud qui démarre avec moins de liens vaut mieux qu'un nœud muet. */
    max = cfg->max_connections;
    if (max > hw_max_links) {
        max = hw_max_links;
    }
    if (max > DENGON_TC_MAX_LINKS) {
        max = DENGON_TC_MAX_LINKS;
    }

    tc->cfg = *cfg;
    tc->max_links = max;
    tc->started = true;

    if (effective_max != NULL) {
        *effective_max = max;
    }
    return DENGON_TR_OK;
}

/* --- Côté radio ------------------------------------------------------------ */

dengon_tr_err_t
dengon_tc_link_open(dengon_tc_t *tc, uint16_t conn_handle, dengon_link_role_t role,
                    dengon_link_id_t *out_id)
{
    dengon_tc_link_t *slot = NULL;

    if (!tc->started) {
        return DENGON_TR_NOT_STARTED;
    }
    if (link_by_conn(tc, conn_handle) != NULL) {
        return DENGON_TR_BACKEND;
    }
    if (dengon_tc_link_count(tc) >= tc->max_links) {
        return DENGON_TR_TOO_MANY_CONNECTIONS;
    }

    for (size_t i = 0; i < DENGON_TC_MAX_LINKS; i++) {
        if (!tc->links[i].used) {
            slot = &tc->links[i];
            break;
        }
    }
    if (slot == NULL) {
        return DENGON_TR_TOO_MANY_CONNECTIONS;
    }

    memset(slot, 0, sizeof(*slot));
    slot->used = true;
    slot->id = tc->next_id++;
    slot->conn_handle = conn_handle;
    slot->role = role;
    /* Tant que l'échange MTU n'a pas abouti, seul le plancher est garanti. */
    slot->mtu = DENGON_TC_ATT_MTU_MIN;

    if (out_id != NULL) {
        *out_id = slot->id;
    }
    return DENGON_TR_OK;
}

void
dengon_tc_link_set_mtu(dengon_tc_t *tc, uint16_t conn_handle, uint16_t mtu)
{
    dengon_tc_link_t *l = link_by_conn(tc, conn_handle);

    if (l != NULL && mtu >= DENGON_TC_ATT_MTU_MIN) {
        l->mtu = mtu > DENGON_TC_ATT_MTU_MAX ? DENGON_TC_ATT_MTU_MAX : mtu;
    }
}

void
dengon_tc_link_set_peer_rx(dengon_tc_t *tc, uint16_t conn_handle, uint16_t handle)
{
    dengon_tc_link_t *l = link_by_conn(tc, conn_handle);

    if (l != NULL) {
        l->peer_rx_handle = handle;
    }
}

void
dengon_tc_link_set_rssi(dengon_tc_t *tc, uint16_t conn_handle, int16_t rssi)
{
    dengon_tc_link_t *l = link_by_conn(tc, conn_handle);

    if (l != NULL) {
        l->has_rssi = true;
        l->rssi = rssi;
    }
}

static bool
announce(dengon_tc_t *tc, dengon_tc_link_t *l)
{
    dengon_transport_event_t ev = { 0 };

    if (l->announced) {
        return false;
    }

    ev.kind = DENGON_EVT_PEER_CONNECTED;
    ev.link = l->id;
    ev.u.connected.has_rssi = l->has_rssi;
    ev.u.connected.rssi = l->rssi;

    if (!q_push(tc, &ev)) {
        /* Ne peut arriver que si le cœur ne poll plus du tout pendant de
           nombreuses connexions. Le lien reste non annoncé : aucun événement
           ne portera son LinkId, la règle n°5 tient toujours. */
        tc->dropped_lifecycle++;
        return false;
    }
    l->announced = true;
    return true;
}

bool
dengon_tc_link_announce(dengon_tc_t *tc, uint16_t conn_handle)
{
    dengon_tc_link_t *l = link_by_conn(tc, conn_handle);

    return l != NULL && announce(tc, l);
}

dengon_tr_err_t
dengon_tc_on_rx(dengon_tc_t *tc, uint16_t conn_handle, const uint8_t *bytes, size_t len)
{
    dengon_tc_link_t *l = link_by_conn(tc, conn_handle);
    dengon_transport_event_t ev = { 0 };
    uint8_t *copy;

    if (l == NULL) {
        return DENGON_TR_UNKNOWN_PEER;
    }

    /* PeerConnected paresseux : un pair qui écrit sans s'être abonné (nRF
       Connect, par exemple) doit quand même voir son lien annoncé AVANT sa
       première trame — sinon le cœur recevrait une trame d'un LinkId inconnu. */
    announce(tc, l);
    if (!l->announced) {
        tc->dropped_frames++;
        return DENGON_TR_BACKEND;
    }

    /* malloc(0) peut rendre NULL : une trame vide reçoit un octet de garde. */
    copy = malloc(len > 0 ? len : 1);
    if (copy == NULL) {
        tc->dropped_frames++;
        return DENGON_TR_BACKEND;
    }
    if (len > 0) {
        memcpy(copy, bytes, len);
    }

    ev.kind = DENGON_EVT_FRAME_RECEIVED;
    ev.link = l->id;
    ev.u.frame.bytes = copy;
    ev.u.frame.len = len;

    if (!q_push(tc, &ev)) {
        free(copy);
        tc->dropped_frames++;
        return DENGON_TR_BACKEND;
    }
    return DENGON_TR_OK;
}

bool
dengon_tc_link_close(dengon_tc_t *tc, uint16_t conn_handle, uint8_t hci_reason)
{
    dengon_tc_link_t *l = link_by_conn(tc, conn_handle);
    dengon_transport_event_t ev = { 0 };
    bool emitted = false;

    if (l == NULL) {
        return false;
    }

    if (l->announced) {
        ev.kind = DENGON_EVT_PEER_DISCONNECTED;
        ev.link = l->id;
        ev.u.reason = dengon_tc_map_hci_reason(hci_reason);
        emitted = q_push(tc, &ev);
        if (!emitted) {
            tc->dropped_lifecycle++;
        }
    }

    /* Le conn_handle est libéré : NimBLE peut le redonner à la prochaine
       connexion, qui obtiendra un LinkId neuf. */
    memset(l, 0, sizeof(*l));
    return emitted;
}

size_t
dengon_tc_link_count(const dengon_tc_t *tc)
{
    size_t n = 0;

    for (size_t i = 0; i < DENGON_TC_MAX_LINKS; i++) {
        if (tc->links[i].used) {
            n++;
        }
    }
    return n;
}

bool
dengon_tc_has_conn(const dengon_tc_t *tc, uint16_t conn_handle)
{
    return link_by_conn_const(tc, conn_handle) != NULL;
}

bool
dengon_tc_link_role(const dengon_tc_t *tc, uint16_t conn_handle, dengon_link_role_t *out_role)
{
    const dengon_tc_link_t *l = link_by_conn_const(tc, conn_handle);

    if (l == NULL) {
        return false;
    }
    *out_role = l->role;
    return true;
}

/* --- Côté cœur ------------------------------------------------------------- */

size_t
dengon_tc_poll(dengon_tc_t *tc, dengon_transport_event_t *out, size_t cap)
{
    size_t n = 0;

    if (!tc->started) {
        return 0;
    }
    while (n < cap && tc->q_len > 0) {
        out[n++] = tc->q[tc->q_head];
        tc->q_head = (tc->q_head + 1) % DENGON_TC_EVQ_CAP;
        tc->q_len--;
    }
    return n;
}

void
dengon_tc_event_free(dengon_transport_event_t *ev)
{
    if (ev->kind == DENGON_EVT_FRAME_RECEIVED) {
        free(ev->u.frame.bytes);
        ev->u.frame.bytes = NULL;
        ev->u.frame.len = 0;
    }
}

dengon_tr_err_t
dengon_tc_route_for_send(const dengon_tc_t *tc, dengon_link_id_t link, size_t len,
                         dengon_tc_route_t *route, size_t *max_out)
{
    const dengon_tc_link_t *l;

    if (!tc->started) {
        return DENGON_TR_NOT_STARTED;
    }

    /* Un lien pas encore annoncé n'existe pas pour le cœur : il ne peut pas
       en connaître le LinkId, et le lui accepter briserait l'ordre par lien. */
    l = link_by_id(tc, link);
    if (l == NULL || !l->announced) {
        return DENGON_TR_UNKNOWN_PEER;
    }

    /* 1 trame = 1 PDU ATT. La fragmentation protocole (protocol::fragment,
       US-202) découpe déjà à ATT_MTU - 3 : le transport n'a pas de
       fragmentation BLE à lui, donc aucune trame partielle à jeter (règle
       n°3 du contrat, tenue par construction). */
    if (len > frame_max(l)) {
        if (max_out != NULL) {
            *max_out = frame_max(l);
        }
        return DENGON_TR_FRAME_TOO_LARGE;
    }

    route_of(l, route);
    return DENGON_TR_OK;
}

dengon_tr_err_t
dengon_tc_routes_for_broadcast(const dengon_tc_t *tc, size_t len, dengon_tc_route_t *routes,
                               size_t cap, size_t *n_out)
{
    size_t n = 0;

    *n_out = 0;
    if (!tc->started) {
        return DENGON_TR_NOT_STARTED;
    }
    if (len > DENGON_TC_FRAME_MAX) {
        return DENGON_TR_FRAME_TOO_LARGE;
    }

    for (size_t i = 0; i < DENGON_TC_MAX_LINKS && n < cap; i++) {
        const dengon_tc_link_t *l = &tc->links[i];

        /* « Au mieux » : un lien au MTU trop petit est sauté, pas une erreur. */
        if (l->used && l->announced && len <= frame_max(l)) {
            route_of(l, &routes[n++]);
        }
    }
    *n_out = n;
    return DENGON_TR_OK;
}

/* --- Utilitaires ------------------------------------------------------------ */

dengon_disconnect_reason_t
dengon_tc_map_hci_reason(uint8_t hci_reason)
{
    switch (hci_reason) {
    case DENGON_HCI_REM_USER_CONN_TERM:
    case DENGON_HCI_RD_CONN_TERM_RESRC:
    case DENGON_HCI_RD_CONN_TERM_PWROFF:
        /* Le pair a ANNONCÉ la fermeture (LL_TERMINATE_IND). */
        return DENGON_DISC_PROPRE;

    case DENGON_HCI_CONN_TERM_LOCAL:
        return DENGON_DISC_LOCALE;

    default:
        /* Supervision timeout (0x08) en tête, mais aussi LMP/LL response
           timeout (0x22), échec d'établissement (0x3E), MIC failure (0x3D)… :
           tout ce qui n'a pas été annoncé est une coupure. C'est le cas
           NORMAL d'un maillage mobile, donc le défaut. */
        return DENGON_DISC_BRUTALE;
    }
}

const char *
dengon_tr_err_str(dengon_tr_err_t err)
{
    switch (err) {
    case DENGON_TR_OK:                   return "ok";
    case DENGON_TR_NOT_STARTED:          return "le transport n'est pas démarré";
    case DENGON_TR_ALREADY_STARTED:      return "le transport est déjà démarré";
    case DENGON_TR_UNKNOWN_PEER:         return "pair inconnu ou déconnecté";
    case DENGON_TR_FRAME_TOO_LARGE:      return "trame trop grande";
    case DENGON_TR_TOO_MANY_CONNECTIONS: return "quota de connexions atteint";
    case DENGON_TR_BACKEND:              return "erreur de la pile BLE";
    default:                             return "erreur inconnue";
    }
}

const char *
dengon_disc_reason_str(dengon_disconnect_reason_t reason)
{
    switch (reason) {
    case DENGON_DISC_PROPRE:  return "Propre";
    case DENGON_DISC_BRUTALE: return "Brutale";
    case DENGON_DISC_LOCALE:  return "Locale";
    default:                  return "?";
    }
}
