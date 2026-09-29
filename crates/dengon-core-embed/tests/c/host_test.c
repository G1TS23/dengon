/* Programme C hôte de `libdengon_core.a` (US-307).
 *
 * Preuve LITTÉRALE du critère d'acceptation « un programme C hôte lie la
 * bibliothèque et appelle une fonction » : ce binaire lie l'archive produite
 * par `dengon-core-embed`, appelle `dengon_protocol_version()`, puis rejoue
 * les vecteurs de `contracts/packet/vectors_v0.json` (accept/reject) à
 * travers `dengon_decode_reencode()` — la base de la patte firmware du job
 * CI `cross-vectors`, qui remplace le proxy `dengon-conformance` pour cette
 * US (voir `docs/suivi/03-ecarts-conception.md`).
 *
 * `vectors_generated.h` est généré juste avant la compilation par
 * `generate_vectors.py`, pas committé (voir ce script).
 *
 * Aucune dépendance externe : C99 + libc standard.
 */

#include <stdio.h>
#include <string.h>

#include "../../../dengon-core-ffi/include/dengon_core.h"
#include "vectors_generated.h"

/* flag_bits.RESERVED_MASK de contracts/packet/vectors_v0.json : les bits
 * réservés (5-7) sont masqués au décodage, donc absents du ré-encodage
 * (synthese/05:80) — seule différence admise entre l'entrée et la sortie
 * d'un ré-encodage réussi, exactement comme côté Rust (voir
 * dengon-conformance et dengon-core-ffi/tests/decode_reencode.rs). */
#define RESERVED_MASK 0xE0
#define FLAGS_OFFSET 3

static int failures = 0;

static void fail(const char *vector, const char *detail) {
    fprintf(stderr, "ECHEC %s : %s\n", vector, detail);
    failures++;
}

static void check_accept(const DengonVector *v) {
    uint8_t out[4096];
    uintptr_t out_len = 0;
    DengonStatus status = dengon_decode_reencode(v->bytes, v->len, out, sizeof(out), &out_len);
    if (status != DENGON_STATUS_OK) {
        fail(v->name, "refuse alors qu'il devrait etre accepte");
        return;
    }
    if (out_len != v->len) {
        fail(v->name, "longueur re-encodee differente de l'entree");
        return;
    }
    for (size_t i = 0; i < out_len; i++) {
        uint8_t attendu = v->bytes[i];
        if (i == FLAGS_OFFSET) {
            attendu = (uint8_t) (attendu & ~RESERVED_MASK);
        }
        if (out[i] != attendu) {
            fail(v->name, "octet re-encode different de l'entree (masque reserve pres)");
            return;
        }
    }
}

static void check_reject(const DengonVector *v) {
    uint8_t out[4096];
    uintptr_t out_len = 0;
    DengonStatus status = dengon_decode_reencode(v->bytes, v->len, out, sizeof(out), &out_len);
    if (status == DENGON_STATUS_OK) {
        fail(v->name, "accepte alors qu'il devrait etre refuse");
    }
}

int main(void) {
    uint8_t version = dengon_protocol_version();
    printf("dengon_protocol_version() = %u\n", (unsigned) version);
    if (version == 0) {
        fprintf(stderr, "ECHEC : version de protocole nulle, liaison suspecte\n");
        return 1;
    }

    size_t accept_count = sizeof(ACCEPT_VECTORS) / sizeof(ACCEPT_VECTORS[0]);
    size_t reject_count = sizeof(REJECT_VECTORS) / sizeof(REJECT_VECTORS[0]);
    printf("vecteurs : %zu accept, %zu reject\n", accept_count, reject_count);

    for (size_t i = 0; i < accept_count; i++) {
        check_accept(&ACCEPT_VECTORS[i]);
    }
    for (size_t i = 0; i < reject_count; i++) {
        check_reject(&REJECT_VECTORS[i]);
    }

    if (failures > 0) {
        fprintf(stderr, "%d echec(s)\n", failures);
        return 1;
    }
    printf("OK : tous les vecteurs de conformite passent depuis le C\n");
    return 0;
}
