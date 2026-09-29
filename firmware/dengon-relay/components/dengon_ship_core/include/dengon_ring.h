// ---------------------------------------------------------------------------
// dengon_ring — buffer ring des événements à expédier au dashboard (US-309).
//
// Pendant que le Wi-Fi ou le VPS est injoignable, les entrées du journal
// s'accumulent ici, puis repartent dans l'ordre au retour du réseau. La
// mémoire est BORNÉE : le tampon est fourni une fois pour toutes par
// l'appelant (statique sur l'ESP32) et rien n'est jamais alloué. Plein, le
// ring écrase ses enregistrements les PLUS ANCIENS, entiers, et les compte
// dans `dropped` (docs/synthese/08-relais-esp32.md §7, `logs_dropped`).
// Le journal complet, lui, reste sur littlefs pour dengon-verify.
//
// Un enregistrement est un bloc d'octets opaque (ici une entrée de journal
// `Entry::to_bytes`), stocké précédé de sa longueur (u32, 4 octets). Le ring
// replie ses octets en bout de tampon : un enregistrement peut chevaucher
// la fin et le début.
//
// Envoi en deux temps, sans perte ni doublon :
//   1. `dengon_ring_peek` copie les plus anciens enregistrements et rend
//      l'identifiant du premier ;
//   2. après un 2xx, `dengon_ring_commit(first_id, n)` les retire.
// Si le ring a écrasé entre-temps une partie de ces enregistrements (réseau
// lent et ring plein), `commit` ne retire que ceux qui restent : les
// identifiants sont croissants, jamais réutilisés.
//
// C pur, sans ESP-IDF : testé sur la cible `linux` (test_apps/). Pas de
// verrou : l'appelant sérialise les accès (mutex dans dengon_ship.c).
// ---------------------------------------------------------------------------
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Octets de préfixe (longueur) par enregistrement. */
#define DENGON_RING_PREFIX 4u

typedef struct {
    uint8_t *buf;
    size_t   cap;
    size_t   head;     /* position du plus ancien enregistrement */
    size_t   used;     /* octets occupés, préfixes compris */
    size_t   count;    /* enregistrements présents */
    uint64_t head_id;  /* identifiant du plus ancien */
    uint64_t dropped;  /* enregistrements écrasés ou refusés, depuis l'init */
} dengon_ring_t;

/** Prépare `ring` sur `buf` (`cap` octets). Le tampon doit survivre au ring. */
void dengon_ring_init(dengon_ring_t *ring, uint8_t *buf, size_t cap);

/**
 * Ajoute un enregistrement de `len` octets, en écrasant les plus anciens si
 * besoin. `false` (et `dropped` incrémenté) si l'enregistrement ne tiendrait
 * pas même dans un ring vide, ou si `len` est nul.
 */
bool dengon_ring_push(dengon_ring_t *ring, const uint8_t *rec, size_t len);

/**
 * Copie dans `out`, bout à bout et SANS préfixe, au plus `max_records` des
 * plus anciens enregistrements, tant qu'ils tiennent dans `out_cap`. Rend le
 * nombre copié ; `*out_len` : octets écrits ; `*first_id` : identifiant du
 * premier. Si le tout premier ne tient pas dans `out_cap`, rend 0 et
 * `*out_len` porte sa taille (l'appelant peut le retirer par `commit`).
 */
size_t dengon_ring_peek(const dengon_ring_t *ring, size_t max_records, uint8_t *out,
                        size_t out_cap, size_t *out_len, uint64_t *first_id);

/**
 * Retire les enregistrements [first_id, first_id + n) encore présents.
 * Rend le nombre réellement retiré.
 */
size_t dengon_ring_commit(dengon_ring_t *ring, uint64_t first_id, size_t n);

/** Enregistrements présents. */
static inline size_t
dengon_ring_count(const dengon_ring_t *ring)
{
    return ring->count;
}

/** Remplissage en pourcentage des octets (0..100), pour `relay.health`. */
unsigned dengon_ring_fill_pct(const dengon_ring_t *ring);

#ifdef __cplusplus
}
#endif
