// ---------------------------------------------------------------------------
// Fragmentation BLE (L1) du relais, au format de l'app Android (US-312).
//
// Les vecteurs reprennent ceux de FragmentationBleTest.kt (app Android) : les
// deux implémentations doivent découper et recoller les mêmes octets, sans
// quoi un téléphone et le relais ne se comprennent pas (trou trouvé en
// préparant l'US-312 : le relais lisait l'en-tête 0x00 comme début de paquet).
// ---------------------------------------------------------------------------
#include <string.h>

#include "unity.h"

#include "banc.h"
#include "tests.h"

static banc_t s_banc;
static lot_t  s_lot;
static uint8_t s_trame[DENGON_TC_FRAME_MAX + 1];
static uint8_t s_morceau[DENGON_TC_ATT_MTU_MAX];

/* Remplit s_trame d'un motif déterministe. */
static void
motif(size_t len)
{
    for (size_t i = 0; i < len; i++) {
        s_trame[i] = (uint8_t)(i * 7 + 3);
    }
}

/* Lien annoncé sur conn 1 au MTU donné ; le PeerConnected est vidé. */
static uint16_t
lien_au_mtu(uint16_t mtu)
{
    banc_demarre(&s_banc);
    TEST_ASSERT_EQUAL(DENGON_TR_OK,
                      dengon_tc_link_open(&s_banc.tc, 1, DENGON_ROLE_PERIPHERAL, NULL));
    dengon_tc_link_set_mtu(&s_banc.tc, 1, mtu);
    dengon_tc_link_announce(&s_banc.tc, 1);
    banc_poll(&s_banc, &s_lot);
    return 1;
}

/* Découpe s_trame[0..len) comme la glue, et fait recevoir chaque morceau. */
static dengon_tr_err_t
envoyer_en_morceaux(uint16_t conn, size_t len, uint16_t mtu)
{
    dengon_tr_err_t err = DENGON_TR_OK;
    size_t total = dengon_tc_chunk_count(len, mtu);

    for (size_t i = 0; i < total && err == DENGON_TR_OK; i++) {
        size_t off = 0;
        size_t n = 0;

        s_morceau[0] = dengon_tc_chunk_at(len, mtu, i, &off, &n);
        TEST_ASSERT_TRUE(DENGON_TC_CHUNK_HDR + n <= (size_t)mtu - DENGON_TC_ATT_OVERHEAD);
        memcpy(s_morceau + DENGON_TC_CHUNK_HDR, s_trame + off, n);
        err = dengon_tc_on_chunk(&s_banc.tc, conn, s_morceau, DENGON_TC_CHUNK_HDR + n);
    }
    return err;
}

/* Vecteur Android « SUITE sur tous les morceaux sauf le dernier » :
   decouper(ByteArray(50), 20) -> 3 morceaux, en-têtes 0x80 0x80 0x00. */
static void
test_decoupage_comme_android(void)
{
    size_t off = 0;
    size_t n = 0;

    /* chargeUtile(23) = 20 octets de PDU, dont 1 d'en-tête. */
    TEST_ASSERT_EQUAL(19, dengon_tc_chunk_payload(23));
    TEST_ASSERT_EQUAL(513, dengon_tc_chunk_payload(517));
    TEST_ASSERT_EQUAL(3, dengon_tc_chunk_count(50, 23));

    TEST_ASSERT_EQUAL_HEX8(0x80, dengon_tc_chunk_at(50, 23, 0, &off, &n));
    TEST_ASSERT_EQUAL(0, off);
    TEST_ASSERT_EQUAL(19, n);
    TEST_ASSERT_EQUAL_HEX8(0x80, dengon_tc_chunk_at(50, 23, 1, &off, &n));
    TEST_ASSERT_EQUAL(19, off);
    TEST_ASSERT_EQUAL_HEX8(0x00, dengon_tc_chunk_at(50, 23, 2, &off, &n));
    TEST_ASSERT_EQUAL(38, off);
    TEST_ASSERT_EQUAL(12, n);

    /* Trame vide : un seul morceau, sans données (vecteur Android). */
    TEST_ASSERT_EQUAL(1, dengon_tc_chunk_count(0, 23));
    TEST_ASSERT_EQUAL_HEX8(0x00, dengon_tc_chunk_at(0, 23, 0, &off, &n));
    TEST_ASSERT_EQUAL(0, n);

    /* Au MTU 517, la plus grande trame (514) tient en 2 morceaux. */
    TEST_ASSERT_EQUAL(1, dengon_tc_chunk_count(513, 517));
    TEST_ASSERT_EQUAL(2, dengon_tc_chunk_count(514, 517));
}

/* Cas courant : une trame d'un seul morceau remonte sans l'en-tête. */
static void
test_un_morceau_une_trame(void)
{
    static const uint8_t m[] = { 0x00, 0x01, 0x0D, 0x42 };
    uint16_t conn = lien_au_mtu(517);

    TEST_ASSERT_EQUAL(DENGON_TR_OK, dengon_tc_on_chunk(&s_banc.tc, conn, m, sizeof(m)));
    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_EQUAL(1, s_lot.n);
    TEST_ASSERT_EQUAL(DENGON_EVT_FRAME_RECEIVED, s_lot.ev[0].kind);
    TEST_ASSERT_EQUAL(3, s_lot.ev[0].u.frame.len);
    TEST_ASSERT_EQUAL_UINT8_ARRAY(m + 1, s_lot.ev[0].u.frame.bytes, 3);
}

/* Aller-retour à plusieurs MTU : une seule trame, octets intacts. */
static void
test_aller_retour_plusieurs_morceaux(void)
{
    static const uint16_t mtus[] = { 23, 24, 100, 185, 517 };
    static const size_t   lens[] = { 1, 19, 20, 50, 300, DENGON_TC_FRAME_MAX };

    for (size_t a = 0; a < sizeof(mtus) / sizeof(mtus[0]); a++) {
        for (size_t b = 0; b < sizeof(lens) / sizeof(lens[0]); b++) {
            uint16_t conn = lien_au_mtu(mtus[a]);

            motif(lens[b]);
            TEST_ASSERT_EQUAL(DENGON_TR_OK, envoyer_en_morceaux(conn, lens[b], mtus[a]));
            banc_poll(&s_banc, &s_lot);
            TEST_ASSERT_EQUAL(1, s_lot.n);
            TEST_ASSERT_EQUAL(lens[b], s_lot.ev[0].u.frame.len);
            TEST_ASSERT_EQUAL_UINT8_ARRAY(s_trame, s_lot.ev[0].u.frame.bytes, lens[b]);
            dengon_tc_link_close(&s_banc.tc, conn, DENGON_HCI_REM_USER_CONN_TERM);
        }
    }
}

/* Trame vide : le morceau [0x00] remonte une trame de longueur 0. */
static void
test_trame_vide(void)
{
    static const uint8_t m[] = { 0x00 };
    uint16_t conn = lien_au_mtu(517);

    TEST_ASSERT_EQUAL(DENGON_TR_OK, dengon_tc_on_chunk(&s_banc.tc, conn, m, sizeof(m)));
    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_EQUAL(1, s_lot.n);
    TEST_ASSERT_EQUAL(0, s_lot.ev[0].u.frame.len);
}

/* Morceau vide ou bits réservés : réassemblage abandonné, compté, et le
   lien repart proprement sur la trame suivante. */
static void
test_morceau_invalide_abandonne(void)
{
    static const uint8_t debut[] = { 0x80, 1, 2 };
    static const uint8_t reserve[] = { 0x40, 9 };
    static const uint8_t fin[] = { 0x00, 7 };
    uint16_t conn = lien_au_mtu(517);

    TEST_ASSERT_EQUAL(DENGON_TR_OK, dengon_tc_on_chunk(&s_banc.tc, conn, debut, sizeof(debut)));
    TEST_ASSERT_EQUAL(DENGON_TR_BACKEND, dengon_tc_on_chunk(&s_banc.tc, conn, debut, 0));
    TEST_ASSERT_EQUAL(1, s_banc.tc.bad_chunks);

    TEST_ASSERT_EQUAL(DENGON_TR_OK, dengon_tc_on_chunk(&s_banc.tc, conn, debut, sizeof(debut)));
    TEST_ASSERT_EQUAL(DENGON_TR_BACKEND,
                      dengon_tc_on_chunk(&s_banc.tc, conn, reserve, sizeof(reserve)));
    TEST_ASSERT_EQUAL(2, s_banc.tc.bad_chunks);

    /* Rien n'est remonté ; la trame suivante n'hérite pas des restes. */
    TEST_ASSERT_EQUAL(DENGON_TR_OK, dengon_tc_on_chunk(&s_banc.tc, conn, fin, sizeof(fin)));
    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_EQUAL(1, s_lot.n);
    TEST_ASSERT_EQUAL(1, s_lot.ev[0].u.frame.len);
    TEST_ASSERT_EQUAL_HEX8(7, s_lot.ev[0].u.frame.bytes[0]);
}

/* Au-delà de DENGON_TC_FRAME_MAX : FRAME_TOO_LARGE, puis le lien est
   réutilisable (même règle que Reassembleur.ajouter côté Android). */
static void
test_trame_trop_longue_puis_reutilisable(void)
{
    static const uint8_t fin[] = { 0x00, 9 };
    uint16_t conn = lien_au_mtu(517);

    motif(DENGON_TC_FRAME_MAX + 1);
    TEST_ASSERT_EQUAL(DENGON_TR_FRAME_TOO_LARGE,
                      envoyer_en_morceaux(conn, DENGON_TC_FRAME_MAX + 1, 517));
    TEST_ASSERT_EQUAL(1, s_banc.tc.bad_chunks);
    TEST_ASSERT_EQUAL(DENGON_TR_OK, dengon_tc_on_chunk(&s_banc.tc, conn, fin, sizeof(fin)));
    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_EQUAL(1, s_lot.n);
    TEST_ASSERT_EQUAL(1, s_lot.ev[0].u.frame.len);
}

/* Coupure au milieu d'une trame : le partiel est jeté (règle n°3), seul le
   PeerDisconnected remonte. */
static void
test_coupure_pendant_le_reassemblage(void)
{
    static const uint8_t debut[] = { 0x80, 1, 2, 3 };
    uint16_t conn = lien_au_mtu(517);

    TEST_ASSERT_EQUAL(DENGON_TR_OK, dengon_tc_on_chunk(&s_banc.tc, conn, debut, sizeof(debut)));
    TEST_ASSERT_TRUE(dengon_tc_link_close(&s_banc.tc, conn, DENGON_HCI_CONN_SPVN_TMO));
    banc_poll(&s_banc, &s_lot);
    TEST_ASSERT_EQUAL(1, s_lot.n);
    TEST_ASSERT_EQUAL(DENGON_EVT_PEER_DISCONNECTED, s_lot.ev[0].kind);
}

/* Un morceau d'un conn_handle inconnu est refusé. */
static void
test_morceau_sans_lien(void)
{
    static const uint8_t m[] = { 0x00, 1 };

    banc_demarre(&s_banc);
    TEST_ASSERT_EQUAL(DENGON_TR_UNKNOWN_PEER, dengon_tc_on_chunk(&s_banc.tc, 42, m, sizeof(m)));
}

void
run_morceaux(void)
{
    UnitySetTestFile(__FILE__);
    RUN_TEST(test_decoupage_comme_android);
    RUN_TEST(test_un_morceau_une_trame);
    RUN_TEST(test_aller_retour_plusieurs_morceaux);
    RUN_TEST(test_trame_vide);
    RUN_TEST(test_morceau_invalide_abandonne);
    RUN_TEST(test_trame_trop_longue_puis_reutilisable);
    RUN_TEST(test_coupure_pendant_le_reassemblage);
    RUN_TEST(test_morceau_sans_lien);
    lot_liberer(&s_lot);
}
