// ---------------------------------------------------------------------------
// dengon_store — le `Store` du relais ESP32 (US-308) : NVS + littlefs.
//
// Répartition, docs/synthese/08-relais-esp32.md §3 :
//   - NVS (espace `dengon`) : les secrets du relais (clé statique X25519 et
//     graine Ed25519, générés une fois par esp_fill_random) et le CURSEUR du
//     journal (`seq` de la prochaine entrée + hash de la dernière) ;
//   - littlefs (monté sur /lfs) : le journal lui-même, `ledger.bin`, en
//     anneau de deux fichiers (voir dengon_ledger_file.h).
//
// Ordre d'écriture qui garantit la reprise sans rupture de chaîne : les
// entrées sont d'abord ajoutées et fsync-ées dans `ledger.bin`, PUIS le
// curseur est committé en NVS. Au boot, dengon_store_boot_anchor() prend la
// dernière entrée complète du fichier comme vérité (le curseur ne sert que
// si le fichier est vide, juste après une rotation).
// ---------------------------------------------------------------------------
#pragma once

#include <stddef.h>
#include <stdint.h>

#include "esp_err.h"

#include "dengon_ledger_file.h"

#ifdef __cplusplus
extern "C" {
#endif

#define DENGON_STORE_BASE_PATH  "/lfs"
#define DENGON_LEDGER_PATH      DENGON_STORE_BASE_PATH "/ledger.bin"
#define DENGON_LEDGER_OLD_PATH  DENGON_STORE_BASE_PATH "/ledger.old"
/** Taille d'un fichier de l'anneau : deux fichiers tiennent dans la
    partition de 256 Ko (docs/synthese/08 §5), avec la marge de littlefs. */
#define DENGON_LEDGER_FILE_MAX  (96 * 1024)

#define DENGON_SECRET_LEN 32

/** Monte littlefs (partition `littlefs`, formatée si le montage échoue). */
esp_err_t dengon_store_init(void);

/**
 * Relit les secrets du relais, ou les génère au premier démarrage avec
 * esp_fill_random() — sous bootloader_random_enable(), la radio n'étant pas
 * encore allumée, pour que ce soit du vrai aléa matériel.
 *
 * @param[out] created  true si les secrets viennent d'être créés
 */
esp_err_t dengon_store_load_secrets(uint8_t dh_secret[DENGON_SECRET_LEN],
                                    uint8_t sign_seed[DENGON_SECRET_LEN], bool *created);

/**
 * Ancre de reprise du journal : dernière entrée complète de `ledger.bin`
 * (fin tronquée réparée au passage), sinon curseur NVS, sinon genèse.
 * Le curseur NVS est resynchronisé sur le résultat.
 *
 * @param[out] truncated  octets retirés d'une fin d'écriture interrompue
 */
esp_err_t dengon_store_boot_anchor(dengon_ledger_tail_t *anchor, size_t *truncated);

/**
 * Ajoute des entrées au journal (fsync), fait tourner l'anneau si besoin,
 * puis committe le curseur `next_seq` / `last_hash`.
 */
esp_err_t dengon_store_append_ledger(const uint8_t *entries, size_t len, uint64_t next_seq,
                                     const uint8_t last_hash[DENGON_LEDGER_HASH_LEN]);

#ifdef __cplusplus
}
#endif
