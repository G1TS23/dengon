// ---------------------------------------------------------------------------
// Tests du buffer ring (US-309) : mémoire bornée, FIFO, écrasement compté,
// et le scénario de l'US — hors ligne, accumulation, retour du réseau.
// ---------------------------------------------------------------------------
#include <string.h>

#include "unity.h"

#include "dengon_ring.h"
#include "tests.h"

#define CAP 256

/* Tampon encadré de sentinelles : le ring ne doit JAMAIS écrire hors de
   ses `CAP` octets. */
static uint8_t zone[16 + CAP + 16];
static uint8_t *const buf = zone + 16;

static void
preparer(dengon_ring_t *r)
{
    memset(zone, 0xA5, sizeof(zone));
    dengon_ring_init(r, buf, CAP);
}

static void
verifier_sentinelles(void)
{
    for (size_t i = 0; i < 16; i++) {
        TEST_ASSERT_EQUAL_HEX8(0xA5, zone[i]);
        TEST_ASSERT_EQUAL_HEX8(0xA5, zone[16 + CAP + i]);
    }
}

/* Enregistrement n°`id` de `len` octets : contenu déductible de `id`. */
static void
fabriquer(uint32_t id, uint8_t *rec, size_t len)
{
    for (size_t i = 0; i < len; i++) {
        rec[i] = (uint8_t)(id * 31u + i);
    }
}

static bool
pousser(dengon_ring_t *r, uint32_t id, size_t len)
{
    uint8_t rec[CAP];

    fabriquer(id, rec, len);
    return dengon_ring_push(r, rec, len);
}

static void
test_fifo_simple(void)
{
    dengon_ring_t r;
    uint8_t       out[CAP];
    uint8_t       attendu[40];
    size_t        len = 0;
    uint64_t      first = 99;

    preparer(&r);
    TEST_ASSERT_EQUAL(0, dengon_ring_peek(&r, 10, out, sizeof(out), &len, &first));
    TEST_ASSERT_TRUE(pousser(&r, 0, 10));
    TEST_ASSERT_TRUE(pousser(&r, 1, 30));
    TEST_ASSERT_EQUAL(2, dengon_ring_count(&r));

    TEST_ASSERT_EQUAL(2, dengon_ring_peek(&r, 10, out, sizeof(out), &len, &first));
    TEST_ASSERT_EQUAL(40, len);
    TEST_ASSERT_EQUAL_UINT64(0, first);
    fabriquer(0, attendu, 10);
    fabriquer(1, attendu + 10, 30);
    TEST_ASSERT_EQUAL_MEMORY(attendu, out, 40);

    /* peek ne retire rien. */
    TEST_ASSERT_EQUAL(2, dengon_ring_count(&r));
    TEST_ASSERT_EQUAL(2, dengon_ring_commit(&r, first, 2));
    TEST_ASSERT_EQUAL(0, dengon_ring_count(&r));
    TEST_ASSERT_EQUAL_UINT64(0, r.dropped);
    verifier_sentinelles();
}

static void
test_peek_respecte_le_nombre_et_la_place(void)
{
    dengon_ring_t r;
    uint8_t       out[CAP];
    size_t        len;
    uint64_t      first;

    preparer(&r);
    for (uint32_t i = 0; i < 5; i++) {
        pousser(&r, i, 20);
    }
    TEST_ASSERT_EQUAL(3, dengon_ring_peek(&r, 3, out, sizeof(out), &len, &first));
    TEST_ASSERT_EQUAL(60, len);
    /* 50 octets de place : seulement 2 enregistrements entiers. */
    TEST_ASSERT_EQUAL(2, dengon_ring_peek(&r, 10, out, 50, &len, &first));
    TEST_ASSERT_EQUAL(40, len);
    /* Le premier ne tient pas : 0, et sa taille pour que l'appelant sache. */
    TEST_ASSERT_EQUAL(0, dengon_ring_peek(&r, 10, out, 10, &len, &first));
    TEST_ASSERT_EQUAL(20, len);
}

static void
test_plein_ecrase_les_plus_anciens_et_compte(void)
{
    dengon_ring_t r;
    uint8_t       out[CAP];
    uint8_t       attendu[46];
    size_t        len;
    uint64_t      first;

    preparer(&r);
    /* 50 octets par enregistrement (46 + 4 de préfixe) : 5 tiennent dans 256. */
    for (uint32_t i = 0; i < 12; i++) {
        TEST_ASSERT_TRUE(pousser(&r, i, 46));
        TEST_ASSERT_TRUE(r.used <= CAP);
        verifier_sentinelles();
    }
    TEST_ASSERT_EQUAL(5, dengon_ring_count(&r));
    TEST_ASSERT_EQUAL_UINT64(7, r.dropped);

    /* Il reste les 5 plus récents, dans l'ordre, intacts malgré le repli. */
    TEST_ASSERT_EQUAL(5, dengon_ring_peek(&r, 10, out, sizeof(out), &len, &first));
    TEST_ASSERT_EQUAL_UINT64(7, first);
    for (uint32_t i = 0; i < 5; i++) {
        fabriquer(7 + i, attendu, 46);
        TEST_ASSERT_EQUAL_MEMORY(attendu, out + i * 46, 46);
    }
}

static void
test_trop_gros_ou_vide_refuse_sans_rien_casser(void)
{
    dengon_ring_t r;

    preparer(&r);
    pousser(&r, 0, 10);
    TEST_ASSERT_FALSE(pousser(&r, 1, CAP - DENGON_RING_PREFIX + 1));
    TEST_ASSERT_FALSE(dengon_ring_push(&r, (const uint8_t *)"", 0));
    TEST_ASSERT_EQUAL_UINT64(2, r.dropped);
    TEST_ASSERT_EQUAL(1, dengon_ring_count(&r));
    /* Exactement la capacité : accepté, vide le reste. */
    TEST_ASSERT_TRUE(pousser(&r, 2, CAP - DENGON_RING_PREFIX));
    TEST_ASSERT_EQUAL(1, dengon_ring_count(&r));
    TEST_ASSERT_EQUAL(100, dengon_ring_fill_pct(&r));
    verifier_sentinelles();
}

static void
test_commit_apres_ecrasement_ne_retire_que_les_restants(void)
{
    dengon_ring_t r;
    uint8_t       out[CAP];
    size_t        len;
    uint64_t      first;

    preparer(&r);
    for (uint32_t i = 0; i < 4; i++) {
        pousser(&r, i, 46);
    }
    /* Batch en vol : les enregistrements 0..3. */
    TEST_ASSERT_EQUAL(4, dengon_ring_peek(&r, 4, out, sizeof(out), &len, &first));
    /* Pendant l'envoi, 3 nouveaux arrivent et écrasent 0 et 1. */
    for (uint32_t i = 4; i < 7; i++) {
        pousser(&r, i, 46);
    }
    TEST_ASSERT_EQUAL_UINT64(2, r.head_id);
    /* 202 : on retire 0..3 ; seuls 2 et 3 sont encore là. */
    TEST_ASSERT_EQUAL(2, dengon_ring_commit(&r, first, 4));
    TEST_ASSERT_EQUAL(3, dengon_ring_count(&r));
    TEST_ASSERT_EQUAL(3, dengon_ring_peek(&r, 10, out, sizeof(out), &len, &first));
    TEST_ASSERT_EQUAL_UINT64(4, first);
    /* Un commit rejoué (même lot) ne retire plus rien. */
    TEST_ASSERT_EQUAL(0, dengon_ring_commit(&r, 0, 4));
    TEST_ASSERT_EQUAL(3, dengon_ring_count(&r));
}

/* Le scénario de l'US-309 : le réseau tombe, 1000 événements arrivent,
   la mémoire ne dépasse jamais le tampon, puis au retour du réseau tout ce
   qui reste repart DANS L'ORDRE, sans trou autre que les écrasés comptés. */
static void
test_hors_ligne_puis_retour_du_reseau(void)
{
    dengon_ring_t r;
    uint8_t       out[CAP];
    uint8_t       attendu[CAP];
    size_t        len;
    uint64_t      first;
    uint64_t      prochain;
    uint32_t      envoyes = 0;

    preparer(&r);
    for (uint32_t i = 0; i < 1000; i++) {
        TEST_ASSERT_TRUE(pousser(&r, i, 10 + i % 37));
        TEST_ASSERT_TRUE(r.used <= CAP);
    }
    verifier_sentinelles();
    TEST_ASSERT_EQUAL_UINT64(1000, r.dropped + dengon_ring_count(&r));

    /* Retour du réseau : lots de 3, chacun accepté. */
    prochain = r.head_id;
    while (dengon_ring_count(&r) > 0) {
        size_t n   = dengon_ring_peek(&r, 3, out, sizeof(out), &len, &first);
        size_t off = 0;

        TEST_ASSERT_TRUE(n > 0);
        TEST_ASSERT_EQUAL_UINT64(prochain, first);
        for (size_t k = 0; k < n; k++) {
            uint32_t id = (uint32_t)(first + k);
            size_t   l  = 10 + id % 37;

            fabriquer(id, attendu, l);
            TEST_ASSERT_EQUAL_MEMORY(attendu, out + off, l);
            off += l;
        }
        TEST_ASSERT_EQUAL(len, off);
        TEST_ASSERT_EQUAL(n, dengon_ring_commit(&r, first, n));
        prochain += n;
        envoyes += (uint32_t)n;
    }
    TEST_ASSERT_EQUAL_UINT64(1000, prochain);
    TEST_ASSERT_EQUAL_UINT64(1000, r.dropped + envoyes);
    TEST_ASSERT_EQUAL(0, dengon_ring_fill_pct(&r));
    verifier_sentinelles();
}

void
run_ring(void)
{
    RUN_TEST(test_fifo_simple);
    RUN_TEST(test_peek_respecte_le_nombre_et_la_place);
    RUN_TEST(test_plein_ecrase_les_plus_anciens_et_compte);
    RUN_TEST(test_trop_gros_ou_vide_refuse_sans_rien_casser);
    RUN_TEST(test_commit_apres_ecrasement_ne_retire_que_les_restants);
    RUN_TEST(test_hors_ligne_puis_retour_du_reseau);
}
