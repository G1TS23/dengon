// ---------------------------------------------------------------------------
// Cas propres à l'implémentation C — ceux que la suite Rust ne peut pas
// exprimer parce que MockTransport n'a ni MTU, ni conn_handle, ni file bornée.
// ---------------------------------------------------------------------------
#include <string.h>

#include "unity.h"

#include "banc.h"
#include "tests.h"

static banc_t s_banc;
static lot_t  s_lot;
static uint8_t s_gros[DENGON_TC_FRAME_MAX + 1];

/* Ouvre un lien sans l'annoncer, comme la glue à BLE_GAP_EVENT_CONNECT. */
static dengon_link_id_t
ouvrir_sans_annoncer(uint16_t conn, uint16_t mtu)
{
    dengon_link_id_t id = 0;

    TEST_ASSERT_EQUAL(DENGON_TR_OK,
                      dengon_tc_link_open(&s_banc.tc, conn, DENGON_ROLE_PERIPHERAL, &id));
    dengon_tc_link_set_mtu(&s_banc.tc, conn, mtu);
    return id;
}

/* 1 trame = 1 PDU ATT : la limite est mtu - 3, et elle est exacte. */
static void
test_trame_limitee_au_mtu_moins_3(void)
{
    dengon_link_id_t lien;
    dengon_tc_route_t route;
    size_t max = 0;

    banc_demarre(&s_banc);
    lien = banc_connecter_un_pair(&s_banc); /* MTU 517 */

    TEST_ASSERT_EQUAL(DENGON_TR_OK,
                      dengon_tc_route_for_send(&s_banc.tc, lien, 514, &route, &max));
    TEST_ASSERT_EQUAL(517, route.mtu);
    TEST_ASSERT_EQUAL(DENGON_TR_FRAME_TOO_LARGE,
                      dengon_tc_route_for_send(&s_banc.tc, lien, 515, &route, &max));
    TEST_ASSERT_EQUAL(514, max);
}

/* Tant que l'échange MTU n'a pas abouti, seul le plancher 23 vaut : 20 octets. */
static void
test_mtu_par_defaut_est_le_plancher(void)
{
    dengon_link_id_t lien;
    dengon_tc_route_t route;
    size_t max = 0;

    banc_demarre(&s_banc);
    lien = ouvrir_sans_annoncer(1, 0 /* ignoré : < 23 */);
    dengon_tc_link_announce(&s_banc.tc, 1);

    TEST_ASSERT_EQUAL(DENGON_TR_OK, dengon_tc_route_for_send(&s_banc.tc, lien, 20, &route, &max));
    TEST_ASSERT_EQUAL(DENGON_TR_FRAME_TOO_LARGE,
                      dengon_tc_route_for_send(&s_banc.tc, lien, 21, &route, &max));
    TEST_ASSERT_EQUAL(20, max);
}

/* Un lien ouvert mais pas encore annoncé n'existe pas pour le cœur. */
static void
test_lien_non_annonce_est_inconnu(void)
{
    dengon_link_id_t lien;
    dengon_tc_route_t route;

    banc_demarre(&s_banc);
    lien = ouvrir_sans_annoncer(1, 517);

    TEST_ASSERT_EQUAL(DENGON_TR_UNKNOWN_PEER,
                      dengon_tc_route_for_send(&s_banc.tc, lien, 1, &route, NULL));
    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_EQUAL(0, s_lot.n);
}

/* Un pair qui écrit avant de s'abonner : PeerConnected paresseux, AVANT la trame. */
static void
test_premiere_trame_annonce_le_lien(void)
{
    static const uint8_t b[] = { 1, 2, 3 };
    dengon_link_id_t lien;

    banc_demarre(&s_banc);
    lien = ouvrir_sans_annoncer(4, 517);
    TEST_ASSERT_EQUAL(DENGON_TR_OK, dengon_tc_on_rx(&s_banc.tc, 4, b, sizeof(b)));
    /* L'abonnement qui arrive ensuite ne ré-annonce pas. */
    TEST_ASSERT_FALSE(dengon_tc_link_announce(&s_banc.tc, 4));

    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_EQUAL(2, s_lot.n);
    TEST_ASSERT_EQUAL(DENGON_EVT_PEER_CONNECTED, s_lot.ev[0].kind);
    TEST_ASSERT_EQUAL(lien, s_lot.ev[0].link);
    TEST_ASSERT_EQUAL(DENGON_EVT_FRAME_RECEIVED, s_lot.ev[1].kind);
    TEST_ASSERT_EQUAL_UINT8_ARRAY(b, s_lot.ev[1].u.frame.bytes, sizeof(b));
}

/* Un lien jamais annoncé disparaît sans PeerDisconnected (règle n°5 : le cœur
   n'a jamais connu ce LinkId, il ne doit pas en entendre parler). */
static void
test_fermeture_d_un_lien_non_annonce_est_silencieuse(void)
{
    banc_demarre(&s_banc);
    ouvrir_sans_annoncer(1, 517);
    TEST_ASSERT_FALSE(dengon_tc_link_close(&s_banc.tc, 1, DENGON_HCI_CONN_SPVN_TMO));
    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_EQUAL(0, s_lot.n);
    TEST_ASSERT_EQUAL(0, dengon_tc_link_count(&s_banc.tc));
}

/* Défaut Rust à 8 liens, contrôleur à 3 : le quota est borné, pas refusé. */
static void
test_quota_borne_au_materiel(void)
{
    dengon_transport_config_t cfg = banc_config();
    size_t eff = 0;

    banc_nouveau(&s_banc);
    TEST_ASSERT_EQUAL(DENGON_TR_OK, dengon_tc_start(&s_banc.tc, &cfg, 3, &eff));
    TEST_ASSERT_EQUAL(3, eff);

    for (uint16_t c = 1; c <= 3; c++) {
        ouvrir_sans_annoncer(c, 517);
    }
    TEST_ASSERT_EQUAL(DENGON_TR_TOO_MANY_CONNECTIONS,
                      dengon_tc_link_open(&s_banc.tc, 4, DENGON_ROLE_PERIPHERAL, NULL));

    /* Une place libérée se reprend. */
    dengon_tc_link_close(&s_banc.tc, 2, DENGON_HCI_CONN_SPVN_TMO);
    TEST_ASSERT_EQUAL(DENGON_TR_OK,
                      dengon_tc_link_open(&s_banc.tc, 4, DENGON_ROLE_PERIPHERAL, NULL));
}

static void
test_ouvrir_avant_start_est_refuse(void)
{
    banc_nouveau(&s_banc);
    TEST_ASSERT_EQUAL(DENGON_TR_NOT_STARTED,
                      dengon_tc_link_open(&s_banc.tc, 1, DENGON_ROLE_CENTRAL, NULL));
}

/* Même conn_handle ouvert deux fois : incohérence de la pile, pas un second lien. */
static void
test_conn_handle_en_double_est_refuse(void)
{
    banc_demarre(&s_banc);
    ouvrir_sans_annoncer(1, 517);
    TEST_ASSERT_EQUAL(DENGON_TR_BACKEND,
                      dengon_tc_link_open(&s_banc.tc, 1, DENGON_ROLE_CENTRAL, NULL));
}

/* File saturée par un pair bavard : les trames en trop sont jetées et
   comptées, mais le PeerDisconnected passe TOUJOURS (réserve de la file). */
static void
test_file_saturee_garde_la_fermeture(void)
{
    static const uint8_t b[] = { 0xAB };
    const dengon_disconnect_reason_t brutale = DENGON_DISC_BRUTALE;
    dengon_link_id_t lien;
    size_t trames = 0;
    bool ferme = false;

    banc_demarre(&s_banc);
    lien = banc_connecter_un_pair(&s_banc);
    for (int i = 0; i < 2 * DENGON_TC_EVQ_CAP; i++) {
        banc_faire_recevoir(&s_banc, lien, b, sizeof(b));
    }
    TEST_ASSERT_TRUE(s_banc.tc.dropped_frames > 0);
    TEST_ASSERT_TRUE(dengon_tc_link_close(&s_banc.tc, banc_conn_de(&s_banc, lien),
                                          DENGON_HCI_CONN_SPVN_TMO));

    banc_poll(&s_banc, &s_lot);
    for (size_t i = 0; i < s_lot.n; i++) {
        trames += s_lot.ev[i].kind == DENGON_EVT_FRAME_RECEIVED;
        ferme |= s_lot.ev[i].kind == DENGON_EVT_PEER_DISCONNECTED &&
                 s_lot.ev[i].u.reason == brutale;
    }
    TEST_ASSERT_TRUE(ferme);
    TEST_ASSERT_EQUAL(DENGON_EVT_PEER_DISCONNECTED, s_lot.ev[s_lot.n - 1].kind);
    TEST_ASSERT_EQUAL(2 * DENGON_TC_EVQ_CAP - s_banc.tc.dropped_frames, trames);
    TEST_ASSERT_EQUAL(0, s_banc.tc.dropped_lifecycle);
}

/* poll respecte sa capacité et garde le reste pour le suivant. */
static void
test_poll_par_petits_lots(void)
{
    static const uint8_t b[] = { 7 };
    dengon_transport_event_t ev[2];
    dengon_link_id_t lien;

    banc_demarre(&s_banc);
    lien = banc_connecter_un_pair(&s_banc);
    banc_faire_recevoir(&s_banc, lien, b, sizeof(b));
    banc_faire_recevoir(&s_banc, lien, b, sizeof(b));

    TEST_ASSERT_EQUAL(2, dengon_tc_poll(&s_banc.tc, ev, 2));
    TEST_ASSERT_EQUAL(DENGON_EVT_PEER_CONNECTED, ev[0].kind);
    dengon_tc_event_free(&ev[1]);
    TEST_ASSERT_EQUAL(1, dengon_tc_poll(&s_banc.tc, ev, 2));
    TEST_ASSERT_EQUAL(DENGON_EVT_FRAME_RECEIVED, ev[0].kind);
    dengon_tc_event_free(&ev[0]);
}

/* Une trame vide est une trame : elle remonte, de longueur 0. */
static void
test_trame_vide_remonte(void)
{
    dengon_link_id_t lien;

    banc_demarre(&s_banc);
    lien = banc_connecter_un_pair(&s_banc);
    banc_poll(&s_banc, &s_lot);
    banc_faire_recevoir(&s_banc, lien, NULL, 0);
    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_EQUAL(1, s_lot.n);
    TEST_ASSERT_EQUAL(DENGON_EVT_FRAME_RECEIVED, s_lot.ev[0].kind);
    TEST_ASSERT_EQUAL(0, s_lot.ev[0].u.frame.len);
}

/* Broadcast : « au mieux ». Trop gros pour tout lien -> erreur ; trop gros
   pour UN lien -> ce lien est sauté, les autres servis. */
static void
test_broadcast_au_mieux(void)
{
    dengon_tc_route_t routes[DENGON_TC_MAX_LINKS];
    size_t n = 99;

    banc_demarre(&s_banc);
    banc_connecter_un_pair(&s_banc);         /* conn 1, MTU 517 */
    ouvrir_sans_annoncer(2, 23);             /* conn 2, MTU 23  */
    dengon_tc_link_announce(&s_banc.tc, 2);
    ouvrir_sans_annoncer(3, 517);            /* conn 3, jamais annoncé */

    TEST_ASSERT_EQUAL(DENGON_TR_OK,
        dengon_tc_routes_for_broadcast(&s_banc.tc, 20, routes, DENGON_TC_MAX_LINKS, &n));
    TEST_ASSERT_EQUAL(2, n);

    TEST_ASSERT_EQUAL(DENGON_TR_OK,
        dengon_tc_routes_for_broadcast(&s_banc.tc, 100, routes, DENGON_TC_MAX_LINKS, &n));
    TEST_ASSERT_EQUAL(1, n);
    TEST_ASSERT_EQUAL(1, routes[0].conn_handle);

    TEST_ASSERT_EQUAL(DENGON_TR_FRAME_TOO_LARGE,
        banc_broadcast(&s_banc, s_gros, sizeof(s_gros)));
}

/* Le rôle et le handle RX distant voyagent jusqu'à la route. */
static void
test_route_porte_role_et_handle(void)
{
    dengon_link_id_t lien = 0;
    dengon_tc_route_t route;
    dengon_link_role_t role;

    banc_demarre(&s_banc);
    TEST_ASSERT_EQUAL(DENGON_TR_OK,
                      dengon_tc_link_open(&s_banc.tc, 9, DENGON_ROLE_CENTRAL, &lien));
    dengon_tc_link_set_peer_rx(&s_banc.tc, 9, 0x2A);
    dengon_tc_link_set_rssi(&s_banc.tc, 9, -61);
    dengon_tc_link_announce(&s_banc.tc, 9);

    TEST_ASSERT_TRUE(dengon_tc_link_role(&s_banc.tc, 9, &role));
    TEST_ASSERT_EQUAL(DENGON_ROLE_CENTRAL, role);
    TEST_ASSERT_EQUAL(DENGON_TR_OK, dengon_tc_route_for_send(&s_banc.tc, lien, 1, &route, NULL));
    TEST_ASSERT_EQUAL(9, route.conn_handle);
    TEST_ASSERT_EQUAL(0x2A, route.peer_rx_handle);

    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_TRUE(s_lot.ev[0].u.connected.has_rssi);
    TEST_ASSERT_EQUAL(-61, s_lot.ev[0].u.connected.rssi);
}

/* Correspondance code HCI -> motif. */
static void
test_mapping_des_raisons_hci(void)
{
    TEST_ASSERT_EQUAL(DENGON_DISC_BRUTALE, dengon_tc_map_hci_reason(0x08));
    TEST_ASSERT_EQUAL(DENGON_DISC_PROPRE,  dengon_tc_map_hci_reason(0x13));
    TEST_ASSERT_EQUAL(DENGON_DISC_PROPRE,  dengon_tc_map_hci_reason(0x14));
    TEST_ASSERT_EQUAL(DENGON_DISC_PROPRE,  dengon_tc_map_hci_reason(0x15));
    TEST_ASSERT_EQUAL(DENGON_DISC_LOCALE,  dengon_tc_map_hci_reason(0x16));
    TEST_ASSERT_EQUAL(DENGON_DISC_BRUTALE, dengon_tc_map_hci_reason(0x22));
    TEST_ASSERT_EQUAL(DENGON_DISC_BRUTALE, dengon_tc_map_hci_reason(0x3E));
}

void
run_specifique(void)
{
    /* Sans cela, Unity attribue chaque cas au fichier de UNITY_BEGIN. */
    UnitySetTestFile(__FILE__);
    RUN_TEST(test_trame_limitee_au_mtu_moins_3);
    RUN_TEST(test_mtu_par_defaut_est_le_plancher);
    RUN_TEST(test_lien_non_annonce_est_inconnu);
    RUN_TEST(test_premiere_trame_annonce_le_lien);
    RUN_TEST(test_fermeture_d_un_lien_non_annonce_est_silencieuse);
    RUN_TEST(test_quota_borne_au_materiel);
    RUN_TEST(test_ouvrir_avant_start_est_refuse);
    RUN_TEST(test_conn_handle_en_double_est_refuse);
    RUN_TEST(test_file_saturee_garde_la_fermeture);
    RUN_TEST(test_poll_par_petits_lots);
    RUN_TEST(test_trame_vide_remonte);
    RUN_TEST(test_broadcast_au_mieux);
    RUN_TEST(test_route_porte_role_et_handle);
    RUN_TEST(test_mapping_des_raisons_hci);
    lot_liberer(&s_lot);
}
