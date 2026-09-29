// ---------------------------------------------------------------------------
// dengon_ship — export du journal du relais vers le dashboard (US-309).
//
//   ledger_task ──push──▶ buffer ring (RAM, CONFIG_DENGON_SHIP_RING_BYTES)
//                              │ peek (≤ CONFIG_DENGON_SHIP_BATCH_MAX)
//                              ▼
//   tâche dengon_ship : dengon_relay_build_batch (signature Ed25519 par
//   dengon-core) → POST CONFIG_DENGON_DASH_URL/ingest/batch, en HTTPS
//   (racine du dashboard épinglée), `Authorization: Bearer <JWT>`
//                              │ statut HTTP → dengon_ship_decide()
//                              ▼
//   commit (2xx) · drop (400/413) · réessai + backoff (réseau, 5xx) ·
//   attente d'un jeton (401)
//
// Hors ligne, le ring se remplit puis écrase ses plus anciennes entrées
// (comptées : `logs_dropped` de relay.health) ; le journal complet reste
// sur littlefs. Au retour du réseau, tout ce qui reste repart dans l'ordre.
//
// Jeton : NVS (espace `dengon_net`, clé `jwt`), posé par la commande console
// `dash token <jwt>` après enregistrement du relais (tools/register_relay.py).
// ---------------------------------------------------------------------------
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "esp_err.h"

typedef struct {
    unsigned fill_pct;       /* remplissage du ring, 0..100 */
    size_t   pending;        /* entrées en attente d'envoi */
    uint64_t dropped;        /* écrasées (ring plein) ou refusées par le serveur */
    uint64_t sent;           /* acceptées par le dashboard */
    int      last_status;    /* dernier statut HTTP (<= 0 : pas de réponse) */
    bool     has_token;
    bool     has_root_ca;
} dengon_ship_stats_t;

/** Prépare le ring (avant la première entrée de journal) et relit le jeton. */
esp_err_t dengon_ship_init(void);

/** Démarre la tâche d'envoi. À appeler après dengon_wifi_start(). */
esp_err_t dengon_ship_start(void);

/** Copie une entrée de journal (`Entry::to_bytes`) dans le ring. */
void dengon_ship_push(const uint8_t *entry, size_t len);

/** Enregistre le JWT en NVS et le prend en compte immédiatement. */
esp_err_t dengon_ship_set_token(const char *jwt);

/** Compteurs, pour `relay.health` et `dash status`. */
void dengon_ship_get_stats(dengon_ship_stats_t *out);
