// ---------------------------------------------------------------------------
// dengon_ship_policy.c — voir include/dengon_ship_policy.h (US-309).
// ---------------------------------------------------------------------------
#include "dengon_ship_policy.h"

dengon_ship_action_t
dengon_ship_decide(int http_status)
{
    if (http_status >= 200 && http_status < 300) {
        return DENGON_SHIP_COMMIT;
    }
    if (http_status == 401 || http_status == 403) {
        return DENGON_SHIP_AUTH;
    }
    /* 408 (délai) et 429 (trop de requêtes) : passagers, pas la faute du
       batch. Tout autre 4xx : le même batch sera refusé à chaque essai. */
    if (http_status >= 400 && http_status < 500 && http_status != 408 && http_status != 429) {
        return DENGON_SHIP_DROP;
    }
    /* <= 0 (pas de réponse), 1xx/3xx inattendus, 408, 429, 5xx. */
    return DENGON_SHIP_RETRY;
}

uint32_t
dengon_ship_backoff_ms(unsigned failures, uint32_t rnd)
{
    uint32_t delay = DENGON_SHIP_BACKOFF_MAX_MS;

    if (failures == 0) {
        failures = 1;
    }
    /* 1 s << 6 = 64 s dépasse déjà le plafond : pas besoin d'aller plus loin
       (et pas de décalage indéfini sur un grand `failures`). */
    if (failures <= 6) {
        delay = DENGON_SHIP_BACKOFF_MIN_MS << (failures - 1);
        if (delay > DENGON_SHIP_BACKOFF_MAX_MS) {
            delay = DENGON_SHIP_BACKOFF_MAX_MS;
        }
    }
    return delay + rnd % (delay / 4 + 1);
}

const char *
dengon_ship_action_str(dengon_ship_action_t action)
{
    switch (action) {
    case DENGON_SHIP_COMMIT: return "commit";
    case DENGON_SHIP_RETRY:  return "retry";
    case DENGON_SHIP_DROP:   return "drop";
    case DENGON_SHIP_AUTH:   return "auth";
    }
    return "?";
}
