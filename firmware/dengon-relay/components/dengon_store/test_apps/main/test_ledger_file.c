// ---------------------------------------------------------------------------
// Tests du journal sur fichier (dengon_ledger_file, US-308) — hôte et carte.
//
// Les entrées sont fabriquées au format `Entry::to_bytes` avec des hashs
// factices : ce module ne vérifie pas le chaînage (dengon-verify le fait),
// il doit seulement retrouver la dernière entrée complète.
// ---------------------------------------------------------------------------
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#include "sdkconfig.h"
#include "unity.h"

#include "dengon_ledger_file.h"
#include "tests.h"

#define CHEMIN      TEST_DIR "/t_ledger.bin"
#define CHEMIN_OLD  TEST_DIR "/t_ledger.old"

static void
put_be(uint8_t *out, uint64_t v, int n)
{
    for (int i = 0; i < n; i++) {
        out[i] = (uint8_t)(v >> (8 * (n - 1 - i)));
    }
}

/* Entrée `seq` dont le hash est rempli de `seq & 0xff`. */
static size_t
entree(uint8_t *out, uint64_t seq, const char *nom, const char *payload)
{
    size_t n = strlen(nom);
    size_t p = strlen(payload);
    size_t pos = 0;

    put_be(out + pos, seq, 8);
    pos += 8;
    put_be(out + pos, 1756684800000ULL + seq, 8);
    pos += 8;
    put_be(out + pos, n, 4);
    pos += 4;
    memcpy(out + pos, nom, n);
    pos += n;
    put_be(out + pos, p, 4);
    pos += 4;
    memcpy(out + pos, payload, p);
    pos += p;
    memset(out + pos, 0xAA, 32); /* prev_hash */
    pos += 32;
    memset(out + pos, (int)(seq & 0xff), 32); /* entry_hash */
    pos += 32;
    memset(out + pos, 0x55, 64); /* sig */
    pos += 64;
    return pos;
}

static long
taille(const char *chemin)
{
    struct stat st;
    return stat(chemin, &st) == 0 ? (long)st.st_size : -1;
}

static void
nettoyer(void)
{
    unlink(CHEMIN);
    unlink(CHEMIN_OLD);
}

static void
test_longueur_d_entree(void)
{
    uint8_t buf[512];
    size_t  n = entree(buf, 3, "relay.boot", "{}");

    TEST_ASSERT_EQUAL(DENGON_LEDGER_FIXED_LEN + 10 + 2, n);
    TEST_ASSERT_EQUAL(n, dengon_ledger_entry_len(buf, n));
    TEST_ASSERT_EQUAL(n, dengon_ledger_entry_len(buf, n + 7));
    for (size_t k = 0; k < n; k++) {
        TEST_ASSERT_EQUAL(0, dengon_ledger_entry_len(buf, k));
    }
    /* Longueur de nom absurde : corruption, pas une allocation de 4 Go. */
    put_be(buf + 16, 0xFFFFFFFFu, 4);
    TEST_ASSERT_EQUAL(0, dengon_ledger_entry_len(buf, n));
}

static void
test_fichier_absent_vaut_journal_vide(void)
{
    dengon_ledger_tail_t tail;
    size_t               tronque = 99;

    nettoyer();
    TEST_ASSERT_EQUAL(0, dengon_ledger_file_recover(CHEMIN, &tail, &tronque));
    TEST_ASSERT_FALSE(tail.has_entries);
    TEST_ASSERT_EQUAL(0, tail.next_seq);
    TEST_ASSERT_EQUAL(0, tronque);
}

static void
test_ancre_apres_la_derniere_entree(void)
{
    uint8_t              buf[1024];
    size_t               n = 0;
    dengon_ledger_tail_t tail;
    uint8_t              attendu[32];

    nettoyer();
    n += entree(buf + n, 0, "relay.boot", "{}");
    n += entree(buf + n, 1, "peer.announce_seen", "{\"peer\":\"0011223344556677\"}");
    TEST_ASSERT_EQUAL(0, dengon_ledger_file_append(CHEMIN, buf, n));
    n = entree(buf, 2, "pkt.relayed", "{}");
    TEST_ASSERT_EQUAL(0, dengon_ledger_file_append(CHEMIN, buf, n));

    TEST_ASSERT_EQUAL(0, dengon_ledger_file_recover(CHEMIN, &tail, NULL));
    TEST_ASSERT_TRUE(tail.has_entries);
    TEST_ASSERT_EQUAL(3, tail.next_seq);
    memset(attendu, 2, sizeof(attendu));
    TEST_ASSERT_EQUAL_MEMORY(attendu, tail.last_hash, 32);
}

/* Le cœur du critère « survit à esp_restart() » côté stockage : une
   écriture coupée au milieu d'une entrée est retirée, l'ancre reste sur la
   dernière entrée complète, et l'entrée suivante recollera à la chaîne. */
static void
test_fin_tronquee_reparee(void)
{
    uint8_t              buf[1024];
    size_t               n;
    size_t               tronque = 0;
    long                 propre;
    dengon_ledger_tail_t tail;

    nettoyer();
    n = entree(buf, 0, "relay.boot", "{}");
    n += entree(buf + n, 1, "pkt.relayed", "{}");
    TEST_ASSERT_EQUAL(0, dengon_ledger_file_append(CHEMIN, buf, n));
    propre = taille(CHEMIN);

    /* Moitié d'une troisième entrée. */
    n = entree(buf, 2, "envelope.stored", "{}");
    TEST_ASSERT_EQUAL(0, dengon_ledger_file_append(CHEMIN, buf, n / 2));

    TEST_ASSERT_EQUAL(0, dengon_ledger_file_recover(CHEMIN, &tail, &tronque));
    TEST_ASSERT_EQUAL(n / 2, tronque);
    TEST_ASSERT_EQUAL(propre, taille(CHEMIN));
    TEST_ASSERT_EQUAL(2, tail.next_seq);

    /* Idempotent : une seconde relecture ne retire plus rien. */
    TEST_ASSERT_EQUAL(0, dengon_ledger_file_recover(CHEMIN, &tail, &tronque));
    TEST_ASSERT_EQUAL(0, tronque);
}

static void
test_rotation_de_l_anneau(void)
{
    uint8_t buf[512];
    size_t  n = entree(buf, 0, "relay.boot", "{}");

    nettoyer();
    TEST_ASSERT_EQUAL(0, dengon_ledger_file_rotate(CHEMIN, CHEMIN_OLD, 10));
    TEST_ASSERT_EQUAL(0, dengon_ledger_file_append(CHEMIN, buf, n));
    TEST_ASSERT_EQUAL(0, dengon_ledger_file_rotate(CHEMIN, CHEMIN_OLD, n));
    TEST_ASSERT_EQUAL(1, dengon_ledger_file_rotate(CHEMIN, CHEMIN_OLD, n - 1));
    TEST_ASSERT_EQUAL(-1, taille(CHEMIN));
    TEST_ASSERT_EQUAL((long)n, taille(CHEMIN_OLD));
    nettoyer();
}

void
run_ledger_file(void)
{
    RUN_TEST(test_longueur_d_entree);
    RUN_TEST(test_fichier_absent_vaut_journal_vide);
    RUN_TEST(test_ancre_apres_la_derniere_entree);
    RUN_TEST(test_fin_tronquee_reparee);
    RUN_TEST(test_rotation_de_l_anneau);
}
