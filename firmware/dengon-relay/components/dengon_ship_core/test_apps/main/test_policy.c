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
    TEST_ASSERT_EQUAL(DENGON_SHIP_RETRY, dengon_ship_decide(301));
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
    RUN_TEST(test_backoff_double_puis_plafonne);
    RUN_TEST(test_gigue_bornee_a_25_pourcent);
}
