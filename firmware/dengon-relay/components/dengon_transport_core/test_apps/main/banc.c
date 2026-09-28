// ---------------------------------------------------------------------------
// Banc d'essai — voir banc.h.
// ---------------------------------------------------------------------------
#include <string.h>

#include "unity.h"

#include "banc.h"

/* Plafond « matériel » simulé, égal à CONFIG_BT_NIMBLE_MAX_CONNECTIONS. */
#define BANC_HW_MAX_LINKS 3

dengon_transport_config_t
banc_config(void)
{
    dengon_transport_config_t cfg = {
        .local_peer_id   = { 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80 },
        .advertise       = true,
        .scan            = true,
        .max_connections = 8,
        .preferred_mtu   = 517,
    };
    return cfg;
}

void
banc_nouveau(banc_t *b)
{
    memset(b, 0, sizeof(*b));
    dengon_tc_init(&b->tc);
}

void
banc_demarre(banc_t *b)
{
    dengon_transport_config_t cfg = banc_config();

    banc_nouveau(b);
    TEST_ASSERT_EQUAL_MESSAGE(DENGON_TR_OK,
                              dengon_tc_start(&b->tc, &cfg, BANC_HW_MAX_LINKS, NULL),
                              "conformité : start() sur un transport neuf doit réussir");
}

dengon_link_id_t
banc_connecter_un_pair(banc_t *b)
{
    dengon_link_id_t id = 0;
    uint16_t conn = 1;

    /* Comme NimBLE : le plus petit conn_handle libre, donc RÉUTILISÉ après
       une fermeture. C'est ce qui rend le cas « LinkId jamais réutilisé »
       significatif. */
    while (dengon_tc_has_conn(&b->tc, conn)) {
        conn++;
    }

    TEST_ASSERT_EQUAL(DENGON_TR_OK,
                      dengon_tc_link_open(&b->tc, conn, DENGON_ROLE_CENTRAL, &id));
    dengon_tc_link_set_mtu(&b->tc, conn, 517);
    TEST_ASSERT_TRUE(dengon_tc_link_announce(&b->tc, conn));

    TEST_ASSERT_TRUE(b->n_ids < sizeof(b->ids) / sizeof(b->ids[0]));
    b->ids[b->n_ids] = id;
    b->conns[b->n_ids] = conn;
    b->n_ids++;
    return id;
}

uint16_t
banc_conn_de(const banc_t *b, dengon_link_id_t lien)
{
    for (size_t i = 0; i < b->n_ids; i++) {
        if (b->ids[i] == lien) {
            return b->conns[i];
        }
    }
    return 0xFFFF;
}

void
banc_couper(banc_t *b, dengon_link_id_t lien, dengon_disconnect_reason_t motif)
{
    uint8_t hci;

    switch (motif) {
    case DENGON_DISC_PROPRE: hci = DENGON_HCI_REM_USER_CONN_TERM; break;
    case DENGON_DISC_LOCALE: hci = DENGON_HCI_CONN_TERM_LOCAL; break;
    default:                 hci = DENGON_HCI_CONN_SPVN_TMO; break;
    }
    dengon_tc_link_close(&b->tc, banc_conn_de(b, lien), hci);
}

void
banc_faire_recevoir(banc_t *b, dengon_link_id_t lien, const uint8_t *bytes, size_t len)
{
    /* Le code de retour est ignoré : sur un lien mort, la vraie pile ne
       livrerait rien du tout, et le cœur rend UNKNOWN_PEER. */
    (void)dengon_tc_on_rx(&b->tc, banc_conn_de(b, lien), bytes, len);
}

dengon_tr_err_t
banc_send(banc_t *b, dengon_link_id_t lien, const uint8_t *bytes, size_t len)
{
    dengon_tc_route_t route;
    dengon_tr_err_t err;

    (void)bytes;
    err = dengon_tc_route_for_send(&b->tc, lien, len, &route, NULL);
    if (err == DENGON_TR_OK) {
        b->radio_sent++;
    }
    return err;
}

dengon_tr_err_t
banc_broadcast(banc_t *b, const uint8_t *bytes, size_t len)
{
    dengon_tc_route_t routes[DENGON_TC_MAX_LINKS];
    size_t n = 0;
    dengon_tr_err_t err;

    (void)bytes;
    err = dengon_tc_routes_for_broadcast(&b->tc, len, routes, DENGON_TC_MAX_LINKS, &n);
    if (err == DENGON_TR_OK) {
        b->radio_sent += n;
    }
    return err;
}

void
lot_liberer(lot_t *lot)
{
    for (size_t i = 0; i < lot->n; i++) {
        dengon_tc_event_free(&lot->ev[i]);
    }
    lot->n = 0;
}

void
banc_poll(banc_t *b, lot_t *lot)
{
    lot_liberer(lot);
    lot->n = dengon_tc_poll(&b->tc, lot->ev, DENGON_TC_EVQ_CAP);
}
