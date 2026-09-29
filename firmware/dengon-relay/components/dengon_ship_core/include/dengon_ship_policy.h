// ---------------------------------------------------------------------------
// dengon_ship_policy — que faire d'un batch après la réponse du dashboard
// (US-309).
//
// Codes réellement rendus par `POST /ingest/batch` (dashboard/api, US-216) :
//   202       stocké (y compris un rejeu : `new_event_count: 0`) ;
//   400 / 413 batch définitivement mauvais (schéma, event_id, trop gros) ;
//   401       jeton absent / expiré, relais inconnu, signature refusée ;
//   503       base occupée, `Retry-After` ;
// plus les échecs sans réponse (Wi-Fi, DNS, TLS, délai), passés ici comme
// un statut <= 0.
//
// La règle qui compte : un batch refusé pour de bon (400/413) est RETIRÉ du
// ring, sinon il bloquerait tous ceux qui attendent derrière lui. Un 401, en
// revanche, n'est pas la faute du batch : on le garde et on ralentit, le
// temps qu'un opérateur reposte un jeton (`dash token`).
//
// C pur, testé sur la cible `linux`.
// ---------------------------------------------------------------------------
#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    DENGON_SHIP_COMMIT, /* accepté : retirer du ring */
    DENGON_SHIP_RETRY,  /* panne passagère : garder, réessayer après backoff */
    DENGON_SHIP_DROP,   /* refusé pour de bon : retirer du ring, compter */
    DENGON_SHIP_AUTH,   /* authentification : garder, attendre un jeton */
} dengon_ship_action_t;

/** Premier délai de réessai. */
#define DENGON_SHIP_BACKOFF_MIN_MS 1000u
/** Plafond du backoff exponentiel, et délai fixe après un 401. */
#define DENGON_SHIP_BACKOFF_MAX_MS 60000u

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

#ifdef __cplusplus
}
#endif
