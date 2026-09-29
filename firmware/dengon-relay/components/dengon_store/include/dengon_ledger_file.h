// ---------------------------------------------------------------------------
// dengon_ledger_file — le journal chaîné du relais sur fichier (US-308).
//
// Le fichier est la simple concaténation des entrées produites par
// dengon-core (`Entry::to_bytes`) : exactement ce que lit `dengon-verify`.
// Format d'une entrée, entiers big-endian :
//
//   seq(8) ‖ ts_ms(8) ‖ name_len(4) ‖ name ‖ payload_len(4) ‖ payload
//         ‖ prev_hash(32) ‖ entry_hash(32) ‖ sig(64)
//
// Ce module est du C pur (stdio seulement) : il tourne sur la cible `linux`
// pour les tests Unity en CI, et sur l'ESP32 au-dessus de littlefs monté
// dans le VFS. Il ne comprend pas le chaînage (c'est dengon-core qui le
// produit et dengon-verify qui le vérifie) : il sait seulement RETROUVER la
// dernière entrée complète après une coupure, pour que le journal reprenne
// exactement là où il s'est arrêté.
// ---------------------------------------------------------------------------
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Longueur d'un hash d'entrée (SHA-256). */
#define DENGON_LEDGER_HASH_LEN 32

/** Partie fixe d'une entrée (tout sauf `name` et `payload`). */
#define DENGON_LEDGER_FIXED_LEN (8 + 8 + 4 + 4 + 32 + 32 + 64)

/** Au-delà, une longueur annoncée est tenue pour de la corruption. */
#define DENGON_LEDGER_FIELD_MAX 4096

/** Ancre de reprise : `seq` de la prochaine entrée, hash de la dernière. */
typedef struct {
    uint64_t next_seq;
    uint8_t  last_hash[DENGON_LEDGER_HASH_LEN];
    /** `false` si le fichier ne contient aucune entrée complète. */
    bool     has_entries;
} dengon_ledger_tail_t;

/**
 * Longueur de l'entrée qui commence à `buf` (`avail` octets disponibles),
 * ou 0 si elle est incomplète ou si une longueur annoncée dépasse
 * DENGON_LEDGER_FIELD_MAX.
 */
size_t dengon_ledger_entry_len(const uint8_t *buf, size_t avail);

/**
 * Relit `path`, retrouve la dernière entrée complète et TRONQUE ce qui suit
 * (écriture interrompue par une coupure ou un `esp_restart()`). Un fichier
 * absent vaut un journal vide.
 *
 * @param[out] tail       ancre après la dernière entrée complète
 * @param[out] truncated  octets retirés en fin de fichier (peut être NULL)
 * @return 0, ou -1 sur erreur d'E/S (errno positionné)
 */
int dengon_ledger_file_recover(const char *path, dengon_ledger_tail_t *tail,
                               size_t *truncated);

/**
 * Ajoute `len` octets (une ou plusieurs entrées complètes) en fin de `path`,
 * puis force l'écriture sur le support (`fflush` + `fsync`) : au retour, les
 * entrées survivent à une coupure.
 *
 * @return 0, ou -1 sur erreur d'E/S
 */
int dengon_ledger_file_append(const char *path, const uint8_t *bytes, size_t len);

/**
 * Anneau : si `path` dépasse `max_bytes`, il devient `old_path` (l'ancien
 * `old_path` est supprimé) et un fichier vide reprend. Les entrées suivantes
 * se vérifient avec `dengon-verify --from-seq/--prev-hash` ancré sur la
 * dernière entrée de `old_path`.
 *
 * @return 1 si une rotation a eu lieu, 0 sinon, -1 sur erreur d'E/S
 */
int dengon_ledger_file_rotate(const char *path, const char *old_path, size_t max_bytes);

#ifdef __cplusplus
}
#endif
