// ---------------------------------------------------------------------------
// dengon_ship_policy — que faire d'un batch après la réponse du dashboard
// (US-309).
//
// Codes réellement rendus par `POST /ingest/batch` (dashboard/api, US-216) :
//   202       stocké (y compris un rejeu : `new_event_count: 0`) ;
//   400 / 413 batch définitivement mauvais (schéma, event_id, trop gros) ;
//   401       jeton absent / expiré, relais inconnu, signature refusée,
//             `node_id` différent de celui du jeton ;
//   503       base occupée, `Retry-After` ;
// plus les échecs sans réponse (Wi-Fi, DNS, TLS, délai), passés ici comme
// un statut <= 0.
//
// Deux règles :
//   - un lot n'est RETIRÉ sans avoir été accepté que si le serveur l'a
//     refusé pour son contenu (400/413/422) — jamais sur un code qui peut
//     venir d'une mauvaise configuration (404 d'un proxy, 3xx…) : ceux-là
//     gardent le lot et attendent ;
//   - une entrée « empoisonnée » (hors contrat, ou qui fait échouer le
//     serveur en 5xx à chaque essai) ne doit pas bloquer la file ni faire
//     jeter ses voisines : on repasse alors en lots d'UNE entrée pour
//     l'isoler, et seule celle-là est retirée.
//
// C pur, testé sur la cible `linux`.
// ---------------------------------------------------------------------------
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    DENGON_SHIP_COMMIT, /* accepté : retirer du ring */
    DENGON_SHIP_RETRY,  /* panne passagère : garder, réessayer après backoff */
    DENGON_SHIP_DROP,   /* refusé pour son contenu : retirer du ring, compter */
    DENGON_SHIP_AUTH,   /* 401/403 : garder, attendre (jeton, enregistrement) */
    DENGON_SHIP_HOLD,   /* réponse inattendue (3xx, 404…) : garder, attendre */
} dengon_ship_action_t;

/** Premier délai de réessai. */
#define DENGON_SHIP_BACKOFF_MIN_MS 1000u
/** Plafond du backoff exponentiel, et délai fixe après AUTH / HOLD. */
#define DENGON_SHIP_BACKOFF_MAX_MS 60000u
/** 5xx consécutifs sur un même lot avant de l'isoler entrée par entrée. */
#define DENGON_SHIP_POISON_LIMIT 5u

/** Action pour un statut HTTP (`<= 0` : pas de réponse). */
dengon_ship_action_t dengon_ship_decide(int http_status);

/**
 * Délai avant la tentative suivante, après `failures` échecs consécutifs
 * (>= 1) : 1 s, 2 s, 4 s… plafonné à 60 s, plus une gigue de 0 à 25 % tirée
 * de `rnd` pour que plusieurs relais ne reviennent pas tous à la même
 * milliseconde.
 */
uint32_t dengon_ship_backoff_ms(unsigned failures, uint32_t rnd);

/** Nom lisible de l'action, pour les journaux série. */
const char *dengon_ship_action_str(dengon_ship_action_t action);

/** État de la tâche d'envoi entre deux lots. */
typedef struct {
    unsigned failures;        /* échecs consécutifs (backoff) */
    unsigned server_failures; /* 5xx consécutifs sur le lot en tête */
    size_t   single_left;     /* > 0 : mode « une entrée par lot », restantes */
} dengon_ship_state_t;

/** Ce que la tâche doit faire du lot qu'elle vient de tenter. */
typedef struct {
    dengon_ship_action_t action;
    bool                 remove;   /* retirer le lot du ring */
    bool                 rejected; /* … en le comptant comme perdu */
    bool                 isolate;  /* on vient de passer en lots d'une entrée */
    uint32_t             delay_ms; /* pause avant le lot suivant */
} dengon_ship_step_t;

void dengon_ship_state_init(dengon_ship_state_t *st);

/** Nombre d'entrées à mettre dans le prochain lot. */
size_t dengon_ship_batch_limit(const dengon_ship_state_t *st, size_t batch_max);

/**
 * Décide du sort d'un lot de `n` entrées. `build_rejected` : dengon-core a
 * refusé de construire le batch (entrée hors contrat) — rien n'a été posté,
 * `http_status` est ignoré. `period_ms` : pause après un lot traité.
 */
dengon_ship_step_t dengon_ship_step(dengon_ship_state_t *st, int http_status,
                                    bool build_rejected, size_t n, uint32_t period_ms,
                                    uint32_t rnd);

#ifdef __cplusplus
}
#endif
