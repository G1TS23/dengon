// ---------------------------------------------------------------------------
// Tests du Store sur carte (NVS + littlefs réels, US-308).
//
// ⚠ Ces tests écrivent dans l'espace NVS `dengon` et dans /lfs : sur une carte
// de relais déjà provisionnée, ils EFFACENT son identité et son journal.
// À lancer sur une carte de test, ou reflasher le relais ensuite avec
// `idf.py erase-flash flash`.
// ---------------------------------------------------------------------------
#include <string.h>
#include <unistd.h>

#include "esp_random.h"
#include "nvs.h"
#include "nvs_flash.h"
#include "unity.h"

#include "dengon_store.h"
#include "tests.h"

static void
effacer_tout(void)
{
    nvs_handle_t h;

    if (nvs_open("dengon", NVS_READWRITE, &h) == ESP_OK) {
        nvs_erase_all(h);
        nvs_commit(h);
        nvs_close(h);
    }
    unlink(DENGON_LEDGER_PATH);
    unlink(DENGON_LEDGER_OLD_PATH);
}

static void
test_secrets_generes_une_seule_fois(void)
{
    uint8_t dh1[32], sg1[32], dh2[32], sg2[32];
    uint8_t zero[32] = { 0 };
    bool    cree;

    effacer_tout();
    TEST_ASSERT_EQUAL(ESP_OK, dengon_store_load_secrets(dh1, sg1, &cree));
    TEST_ASSERT_TRUE(cree);
    TEST_ASSERT_FALSE(memcmp(dh1, zero, 32) == 0);
    TEST_ASSERT_FALSE(memcmp(dh1, sg1, 32) == 0);

    TEST_ASSERT_EQUAL(ESP_OK, dengon_store_load_secrets(dh2, sg2, &cree));
    TEST_ASSERT_FALSE(cree);
    TEST_ASSERT_EQUAL_MEMORY(dh1, dh2, 32);
    TEST_ASSERT_EQUAL_MEMORY(sg1, sg2, 32);
}

static void
test_alea_materiel_non_constant(void)
{
    uint8_t a[32], b[32];

    esp_fill_random(a, sizeof(a));
    esp_fill_random(b, sizeof(b));
    TEST_ASSERT_FALSE(memcmp(a, b, sizeof(a)) == 0);
}

/* Construit une entrée factice `seq` (hash rempli de `seq & 0xff`). */
static size_t
entree(uint8_t *out, uint64_t seq)
{
    size_t pos = 0;

    memset(out, 0, 24);
    for (int i = 0; i < 8; i++) {
        out[i] = (uint8_t)(seq >> (56 - 8 * i));
    }
    pos = 16;
    out[pos + 3] = 1; /* name_len = 1 */
    out[pos + 4] = 'x';
    pos += 5;
    memset(out + pos, 0, 4); /* payload_len = 0 */
    pos += 4;
    memset(out + pos, 0xAA, 32);
    pos += 32;
    memset(out + pos, (int)(seq & 0xff), 32);
    pos += 32;
    memset(out + pos, 0x55, 64);
    pos += 64;
    return pos;
}

static void
test_ancre_de_boot(void)
{
    uint8_t              buf[256];
    uint8_t              h[32];
    size_t               n;
    size_t               tronque;
    dengon_ledger_tail_t ancre;

    effacer_tout();
    /* Premier boot : genèse. */
    TEST_ASSERT_EQUAL(ESP_OK, dengon_store_boot_anchor(&ancre, &tronque));
    TEST_ASSERT_FALSE(ancre.has_entries);

    /* Deux entrées écrites, curseur committé. */
    n = entree(buf, 0);
    n += entree(buf + n, 1);
    memset(h, 1, sizeof(h));
    TEST_ASSERT_EQUAL(ESP_OK, dengon_store_append_ledger(buf, n, 2, h));

    /* Coupure en pleine écriture de la troisième. */
    n = entree(buf, 2);
    TEST_ASSERT_EQUAL(0, dengon_ledger_file_append(DENGON_LEDGER_PATH, buf, n - 10));

    TEST_ASSERT_EQUAL(ESP_OK, dengon_store_boot_anchor(&ancre, &tronque));
    TEST_ASSERT_TRUE(ancre.has_entries);
    TEST_ASSERT_EQUAL(2, ancre.next_seq);
    TEST_ASSERT_EQUAL_MEMORY(h, ancre.last_hash, 32);
    TEST_ASSERT_EQUAL(n - 10, tronque);
}

static void
test_curseur_porte_l_ancre_apres_rotation(void)
{
    uint8_t              h[32];
    size_t               tronque;
    dengon_ledger_tail_t ancre;

    effacer_tout();
    memset(h, 7, sizeof(h));
    /* Fichier vide (juste après une rotation), curseur à 42. */
    TEST_ASSERT_EQUAL(ESP_OK, dengon_store_append_ledger(NULL, 0, 42, h));
    TEST_ASSERT_EQUAL(ESP_OK, dengon_store_boot_anchor(&ancre, &tronque));
    TEST_ASSERT_TRUE(ancre.has_entries);
    TEST_ASSERT_EQUAL(42, ancre.next_seq);
    TEST_ASSERT_EQUAL_MEMORY(h, ancre.last_hash, 32);
    effacer_tout();
}

void
run_store_cible(void)
{
    RUN_TEST(test_secrets_generes_une_seule_fois);
    RUN_TEST(test_alea_materiel_non_constant);
    RUN_TEST(test_ancre_de_boot);
    RUN_TEST(test_curseur_porte_l_ancre_apres_rotation);
}
