// ---------------------------------------------------------------------------
// Manufacturer data et règle anti-boucle (dengon_adv).
// ---------------------------------------------------------------------------
#include <string.h>

#include "unity.h"

#include "dengon_adv.h"
#include "tests.h"

static const uint8_t PEER_A[8] = { 0x11, 0x22, 0x33, 0x44, 0xAA, 0xBB, 0xCC, 0xDD };
static const uint8_t ADDR_1[6] = { 1, 1, 1, 1, 1, 1 };
static const uint8_t ADDR_2[6] = { 2, 2, 2, 2, 2, 2 };

/* Le format de US-114 est conservé octet pour octet : Company ID LE d'abord. */
static void
test_mfg_aller_retour(void)
{
    uint8_t mfg[DENGON_ADV_MFG_LEN];
    dengon_adv_info_t info;
    static const uint8_t attendu[] = { 0xFF, 0xFF, 0x11, 0x22, 0x33, 0x44, 0x05 };

    dengon_adv_build_mfg(PEER_A, DENGON_ADV_F_RELAY | DENGON_ADV_F_ACCEPTS_CONN, mfg);
    TEST_ASSERT_EQUAL_UINT8_ARRAY(attendu, mfg, sizeof(attendu));

    TEST_ASSERT_TRUE(dengon_adv_parse_mfg(mfg, sizeof(mfg), &info));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(PEER_A, info.peer_prefix, DENGON_ADV_PEER_PREFIX_LEN);
    TEST_ASSERT_EQUAL_HEX8(0x05, info.flags);
}

static void
test_mfg_trop_court_ou_etranger_est_rejete(void)
{
    static const uint8_t court[] = { 0xFF, 0xFF, 1, 2, 3 };
    static const uint8_t apple[] = { 0x4C, 0x00, 1, 2, 3, 4, 5 };
    dengon_adv_info_t info;

    TEST_ASSERT_FALSE(dengon_adv_parse_mfg(court, sizeof(court), &info));
    TEST_ASSERT_FALSE(dengon_adv_parse_mfg(apple, sizeof(apple), &info));
    TEST_ASSERT_FALSE(dengon_adv_parse_mfg(NULL, 7, &info));
}

/* Annonce d'un téléphone Android : Company ID + préfixe, sans flags (US-312). */
static void
test_mfg_du_telephone_sans_flags_est_accepte(void)
{
    static const uint8_t tel[] = { 0xFF, 0xFF, 0xF1, 0xD6, 0xA8, 0x60 };
    static const uint8_t prefixe[] = { 0xF1, 0xD6, 0xA8, 0x60 };
    dengon_adv_info_t info;

    TEST_ASSERT_TRUE(dengon_adv_parse_mfg(tel, sizeof(tel), &info));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(prefixe, info.peer_prefix, 4);
    TEST_ASSERT_EQUAL_HEX8(0, info.flags);
}

/* Le relais n'initie jamais vers un téléphone (US-312), seulement vers un
   relais au peerID plus grand. */
static void
test_relais_n_initie_que_vers_un_relais(void)
{
    static const uint8_t moi[8] = { 0x10, 0, 0, 0, 0, 0, 0, 0 };
    static const uint8_t adr_a[6] = { 1, 1, 1, 1, 1, 1 };
    static const uint8_t adr_b[6] = { 2, 2, 2, 2, 2, 2 };
    dengon_adv_info_t telephone = { .peer_prefix = { 0xF0, 0, 0, 0 }, .flags = 0 };
    dengon_adv_info_t grand = { .peer_prefix = { 0xF0, 0, 0, 0 }, .flags = DENGON_ADV_F_RELAY };
    dengon_adv_info_t petit = { .peer_prefix = { 0x01, 0, 0, 0 }, .flags = DENGON_ADV_F_RELAY };

    TEST_ASSERT_FALSE(dengon_adv_relay_should_connect(moi, &telephone, adr_a, adr_b));
    TEST_ASSERT_TRUE(dengon_adv_relay_should_connect(moi, &grand, adr_a, adr_b));
    TEST_ASSERT_FALSE(dengon_adv_relay_should_connect(moi, &petit, adr_a, adr_b));
}

/* Des champs ajoutés en queue par une version future ne rendent pas aveugle. */
static void
test_mfg_plus_long_est_accepte(void)
{
    static const uint8_t long_[] = { 0xFF, 0xFF, 9, 8, 7, 6, 0x01, 0xEE, 0xEE };
    dengon_adv_info_t info;

    TEST_ASSERT_TRUE(dengon_adv_parse_mfg(long_, sizeof(long_), &info));
    TEST_ASSERT_EQUAL_HEX8(0x01, info.flags);
}

/* Le plus petit peerID initie ; exactement un des deux côtés le fait. */
static void
test_anti_boucle_le_plus_petit_initie(void)
{
    static const uint8_t PEER_B[8] = { 0x11, 0x22, 0x33, 0x45, 0, 0, 0, 0 };

    TEST_ASSERT_TRUE(dengon_adv_should_initiate(PEER_A, PEER_B, ADDR_1, ADDR_2));
    TEST_ASSERT_FALSE(dengon_adv_should_initiate(PEER_B, PEER_A, ADDR_2, ADDR_1));
}

/* Préfixes égaux (4 octets seulement sont annoncés) : on départage à l'adresse. */
static void
test_anti_boucle_egalite_departagee_par_adresse(void)
{
    static const uint8_t PEER_A_BIS[8] = { 0x11, 0x22, 0x33, 0x44, 0, 0, 0, 1 };

    TEST_ASSERT_TRUE(dengon_adv_should_initiate(PEER_A, PEER_A_BIS, ADDR_1, ADDR_2));
    TEST_ASSERT_FALSE(dengon_adv_should_initiate(PEER_A_BIS, PEER_A, ADDR_2, ADDR_1));
}

/* Sa propre annonce (même préfixe, même adresse) : jamais d'auto-connexion. */
static void
test_anti_boucle_pas_de_connexion_a_soi(void)
{
    TEST_ASSERT_FALSE(dengon_adv_should_initiate(PEER_A, PEER_A, ADDR_1, ADDR_1));
}

void
run_adv(void)
{
    /* Sans cela, Unity attribue chaque cas au fichier de UNITY_BEGIN. */
    UnitySetTestFile(__FILE__);
    RUN_TEST(test_mfg_aller_retour);
    RUN_TEST(test_mfg_trop_court_ou_etranger_est_rejete);
    RUN_TEST(test_mfg_du_telephone_sans_flags_est_accepte);
    RUN_TEST(test_mfg_plus_long_est_accepte);
    RUN_TEST(test_relais_n_initie_que_vers_un_relais);
    RUN_TEST(test_anti_boucle_le_plus_petit_initie);
    RUN_TEST(test_anti_boucle_egalite_departagee_par_adresse);
    RUN_TEST(test_anti_boucle_pas_de_connexion_a_soi);
}
