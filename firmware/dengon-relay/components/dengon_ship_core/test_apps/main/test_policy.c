// ---------------------------------------------------------------------------
// Tests de la politique de réessai (US-309).
// ---------------------------------------------------------------------------
#include "unity.h"

#include "dengon_ship_policy.h"
#include "tests.h"

static void
test_codes_reels_du_dashboard(void)
{
    TEST_ASSERT_EQUAL(DENGON_SHIP_COMMIT, dengon_ship_decide(202));
    TEST_ASSERT_EQUAL(DENGON_SHIP_COMMIT, dengon_ship_decide(200));
    TEST_ASSERT_EQUAL(DENGON_SHIP_DROP, dengon_ship_decide(400));
    TEST_ASSERT_EQUAL(DENGON_SHIP_DROP, dengon_ship_decide(413));
    TEST_ASSERT_EQUAL(DENGON_SHIP_DROP, dengon_ship_decide(422));
    TEST_ASSERT_EQUAL(DENGON_SHIP_AUTH, dengon_ship_decide(401));
    TEST_ASSERT_EQUAL(DENGON_SHIP_AUTH, dengon_ship_decide(403));
    TEST_ASSERT_EQUAL(DENGON_SHIP_RETRY, dengon_ship_decide(503));
    TEST_ASSERT_EQUAL(DENGON_SHIP_RETRY, dengon_ship_decide(500));
    TEST_ASSERT_EQUAL(DENGON_SHIP_RETRY, dengon_ship_decide(502));
}

static void
test_passagers_et_sans_reponse_se_reessaient(void)
{
    TEST_ASSERT_EQUAL(DENGON_SHIP_RETRY, dengon_ship_decide(0));
    TEST_ASSERT_EQUAL(DENGON_SHIP_RETRY, dengon_ship_decide(-1));
    TEST_ASSERT_EQUAL(DENGON_SHIP_RETRY, dengon_ship_decide(408));
    TEST_ASSERT_EQUAL(DENGON_SHIP_RETRY, dengon_ship_decide(429));
}

/* Revue PR #120 : un code qui peut venir d'une mauvaise configuration (proxy,
   redirection) ne doit jamais faire détruire un lot valide. */
static void
test_codes_de_configuration_gardent_le_lot(void)
{
    TEST_ASSERT_EQUAL(DENGON_SHIP_HOLD, dengon_ship_decide(301));
    TEST_ASSERT_EQUAL(DENGON_SHIP_HOLD, dengon_ship_decide(302));
    TEST_ASSERT_EQUAL(DENGON_SHIP_HOLD, dengon_ship_decide(404));
    TEST_ASSERT_EQUAL(DENGON_SHIP_HOLD, dengon_ship_decide(405));
    TEST_ASSERT_EQUAL(DENGON_SHIP_HOLD, dengon_ship_decide(421));
    TEST_ASSERT_EQUAL(DENGON_SHIP_HOLD, dengon_ship_decide(101));
}

/* --- Machine à états ---------------------------------------------------- */

#define PERIODE 1000u

static void
test_lot_accepte_puis_refuse(void)
{
    dengon_ship_state_t st;
    dengon_ship_step_t  s;

    dengon_ship_state_init(&st);
    s = dengon_ship_step(&st, 202, false, 16, PERIODE, 0);
    TEST_ASSERT_TRUE(s.remove);
    TEST_ASSERT_FALSE(s.rejected);
    TEST_ASSERT_EQUAL_UINT32(PERIODE, s.delay_ms);
    s = dengon_ship_step(&st, 400, false, 16, PERIODE, 0);
    TEST_ASSERT_TRUE(s.remove && s.rejected);
    TEST_ASSERT_EQUAL(16, dengon_ship_batch_limit(&st, 16));
}

/* Revue PR #120 point 3 : une entrée hors contrat ne fait plus jeter ses 15
   voisines — on les reprend une par une, seule la fautive est retirée. */
static void
test_entree_hors_contrat_isolee(void)
{
    dengon_ship_state_t st;
    dengon_ship_step_t  s;
    unsigned            retirees = 0, perdues = 0;

    dengon_ship_state_init(&st);
    s = dengon_ship_step(&st, 0, true, 16, PERIODE, 0);
    TEST_ASSERT_FALSE(s.remove);
    TEST_ASSERT_TRUE(s.isolate);
    TEST_ASSERT_EQUAL(1, dengon_ship_batch_limit(&st, 16));

    /* 16 lots d'une entrée : la 5e est hors contrat, les autres passent. */
    for (int i = 0; i < 16; i++) {
        TEST_ASSERT_EQUAL(1, dengon_ship_batch_limit(&st, 16));
        s = dengon_ship_step(&st, 202, i == 4, 1, PERIODE, 0);
        TEST_ASSERT_TRUE(s.remove);
        retirees++;
        perdues += s.rejected;
    }
    TEST_ASSERT_EQUAL(16, retirees);
    TEST_ASSERT_EQUAL(1, perdues);
    /* Isolement terminé : retour aux lots complets. */
    TEST_ASSERT_EQUAL(16, dengon_ship_batch_limit(&st, 16));
}

/* Revue PR #120 point 5 : un lot qui fait échouer le serveur en 5xx à chaque
   essai ne bloque plus la file indéfiniment. */
static void
test_5xx_persistant_isole_puis_retire(void)
{
    dengon_ship_state_t st;
    dengon_ship_step_t  s;

    dengon_ship_state_init(&st);
    for (unsigned i = 1; i < DENGON_SHIP_POISON_LIMIT; i++) {
        s = dengon_ship_step(&st, 500, false, 16, PERIODE, 0);
        TEST_ASSERT_FALSE(s.remove);
        TEST_ASSERT_FALSE(s.isolate);
        TEST_ASSERT_EQUAL_UINT32(dengon_ship_backoff_ms(i, 0), s.delay_ms);
    }
    s = dengon_ship_step(&st, 500, false, 16, PERIODE, 0);
    TEST_ASSERT_TRUE(s.isolate);
    TEST_ASSERT_FALSE(s.remove);
    TEST_ASSERT_EQUAL(1, dengon_ship_batch_limit(&st, 16));

    /* L'entrée fautive, seule : retirée après la même limite. */
    for (unsigned i = 1; i < DENGON_SHIP_POISON_LIMIT; i++) {
        TEST_ASSERT_FALSE(dengon_ship_step(&st, 503, false, 1, PERIODE, 0).remove);
    }
    s = dengon_ship_step(&st, 503, false, 1, PERIODE, 0);
    TEST_ASSERT_TRUE(s.remove && s.rejected);
}

/* Une panne réseau ne rend jamais un lot suspect, aussi longue soit-elle. */
static void
test_panne_reseau_ne_retire_jamais(void)
{
    dengon_ship_state_t st;

    dengon_ship_state_init(&st);
    for (int i = 0; i < 100; i++) {
        dengon_ship_step_t s = dengon_ship_step(&st, -1, false, 16, PERIODE, 0);
        TEST_ASSERT_FALSE(s.remove);
        TEST_ASSERT_FALSE(s.isolate);
    }
    TEST_ASSERT_EQUAL(16, dengon_ship_batch_limit(&st, 16));
}

/* Revue PR #120 points 6 et 7 : 404 persistant = lot gardé ; après un
   401/404 le backoff repart de zéro. */
static void
test_404_garde_et_compteurs_remis_a_zero(void)
{
    dengon_ship_state_t st;
    dengon_ship_step_t  s;

    dengon_ship_state_init(&st);
    dengon_ship_step(&st, -1, false, 16, PERIODE, 0);
    dengon_ship_step(&st, -1, false, 16, PERIODE, 0);
    TEST_ASSERT_EQUAL(2, st.failures);
    for (int i = 0; i < 50; i++) {
        s = dengon_ship_step(&st, 404, false, 16, PERIODE, 0);
        TEST_ASSERT_EQUAL(DENGON_SHIP_HOLD, s.action);
        TEST_ASSERT_FALSE(s.remove);
        TEST_ASSERT_EQUAL_UINT32(DENGON_SHIP_BACKOFF_MAX_MS, s.delay_ms);
    }
    TEST_ASSERT_EQUAL(0, st.failures);
    s = dengon_ship_step(&st, 401, false, 16, PERIODE, 0);
    TEST_ASSERT_EQUAL(DENGON_SHIP_AUTH, s.action);
    TEST_ASSERT_EQUAL(0, st.failures);
    s = dengon_ship_step(&st, -1, false, 16, PERIODE, 0);
    TEST_ASSERT_EQUAL_UINT32(DENGON_SHIP_BACKOFF_MIN_MS, s.delay_ms);
}

static void
test_backoff_double_puis_plafonne(void)
{
    TEST_ASSERT_EQUAL_UINT32(1000, dengon_ship_backoff_ms(1, 0));
    TEST_ASSERT_EQUAL_UINT32(2000, dengon_ship_backoff_ms(2, 0));
    TEST_ASSERT_EQUAL_UINT32(4000, dengon_ship_backoff_ms(3, 0));
    TEST_ASSERT_EQUAL_UINT32(32000, dengon_ship_backoff_ms(6, 0));
    TEST_ASSERT_EQUAL_UINT32(60000, dengon_ship_backoff_ms(7, 0));
    TEST_ASSERT_EQUAL_UINT32(60000, dengon_ship_backoff_ms(1000, 0));
    /* 0 traité comme un premier échec, pas comme un décalage de -1. */
    TEST_ASSERT_EQUAL_UINT32(1000, dengon_ship_backoff_ms(0, 0));
}

static void
test_gigue_bornee_a_25_pourcent(void)
{
    for (uint32_t rnd = 0; rnd < 5000; rnd += 7) {
        uint32_t d = dengon_ship_backoff_ms(1, rnd * 2654435761u);
        TEST_ASSERT_TRUE(d >= 1000 && d <= 1250);
        d = dengon_ship_backoff_ms(20, rnd * 2654435761u);
        TEST_ASSERT_TRUE(d >= 60000 && d <= 75000);
    }
}

void
run_policy(void)
{
    RUN_TEST(test_codes_reels_du_dashboard);
    RUN_TEST(test_passagers_et_sans_reponse_se_reessaient);
    RUN_TEST(test_codes_de_configuration_gardent_le_lot);
    RUN_TEST(test_lot_accepte_puis_refuse);
    RUN_TEST(test_entree_hors_contrat_isolee);
    RUN_TEST(test_5xx_persistant_isole_puis_retire);
    RUN_TEST(test_panne_reseau_ne_retire_jamais);
    RUN_TEST(test_404_garde_et_compteurs_remis_a_zero);
    RUN_TEST(test_backoff_double_puis_plafonne);
    RUN_TEST(test_gigue_bornee_a_25_pourcent);
}
