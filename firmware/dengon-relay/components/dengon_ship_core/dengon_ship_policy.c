// ---------------------------------------------------------------------------
// dengon_ship_policy.c — voir include/dengon_ship_policy.h (US-309).
// ---------------------------------------------------------------------------
#include "dengon_ship_policy.h"

#include <string.h>

dengon_ship_action_t
dengon_ship_decide(int http_status)
{
    if (http_status >= 200 && http_status < 300) {
        return DENGON_SHIP_COMMIT;
    }
    if (http_status == 401 || http_status == 403) {
        return DENGON_SHIP_AUTH;
    }
    /* Seuls codes par lesquels le dashboard refuse le CONTENU d'un lot : le
       même lot sera refusé à chaque essai. */
    if (http_status == 400 || http_status == 413 || http_status == 422) {
        return DENGON_SHIP_DROP;
    }
    /* Pas de réponse, délai (408), trop de requêtes (429), serveur (5xx). */
    if (http_status <= 0 || http_status == 408 || http_status == 429
        || (http_status >= 500 && http_status < 600)) {
        return DENGON_SHIP_RETRY;
    }
    /* 1xx, 3xx, autres 4xx (404 d'un proxy, 405…) : rien ne dit que le lot
       est en cause — probablement une mauvaise configuration. On le garde. */
    return DENGON_SHIP_HOLD;
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
    case DENGON_SHIP_HOLD:   return "hold";
    }
    return "?";
}

void
dengon_ship_state_init(dengon_ship_state_t *st)
{
    memset(st, 0, sizeof(*st));
}

size_t
dengon_ship_batch_limit(const dengon_ship_state_t *st, size_t batch_max)
{
    return st->single_left > 0 ? 1 : batch_max;
}

/* Le lot en tête est traité (accepté ou retiré) : fin d'un essai. */
static void
lot_traite(dengon_ship_state_t *st)
{
    st->failures        = 0;
    st->server_failures = 0;
    if (st->single_left > 0) {
        st->single_left--;
    }
}

/* Passe en lots d'une entrée pour les `n` entrées du lot en tête. */
static void
isoler(dengon_ship_state_t *st, size_t n, dengon_ship_step_t *s)
{
    st->single_left     = n;
    st->server_failures = 0;
    s->isolate          = true;
}

dengon_ship_step_t
dengon_ship_step(dengon_ship_state_t *st, int http_status, bool build_rejected, size_t n,
                 uint32_t period_ms, uint32_t rnd)
{
    dengon_ship_step_t s = { 0 };

    if (build_rejected) {
        /* Une entrée au moins est hors contrat. Seule, on la retire ; sinon
           on la cherche une par une au lieu de jeter tout le lot. */
        s.action = DENGON_SHIP_DROP;
        if (n <= 1) {
            s.remove   = true;
            s.rejected = true;
            s.delay_ms = period_ms;
            lot_traite(st);
        } else {
            isoler(st, n, &s);
        }
        return s;
    }

    s.action = dengon_ship_decide(http_status);
    switch (s.action) {
    case DENGON_SHIP_COMMIT:
        s.remove   = true;
        s.delay_ms = period_ms;
        lot_traite(st);
        break;
    case DENGON_SHIP_DROP:
        s.remove   = true;
        s.rejected = true;
        s.delay_ms = period_ms;
        lot_traite(st);
        break;
    case DENGON_SHIP_AUTH:
    case DENGON_SHIP_HOLD:
        /* Rien à apprendre d'un backoff : attente fixe, compteurs remis à
           zéro pour ne pas repartir d'un backoff périmé ensuite. */
        st->failures        = 0;
        st->server_failures = 0;
        s.delay_ms          = DENGON_SHIP_BACKOFF_MAX_MS;
        break;
    case DENGON_SHIP_RETRY:
        s.delay_ms = dengon_ship_backoff_ms(++st->failures, rnd);
        /* Le serveur a répondu 5xx : si c'est chaque fois ce lot-là, il est
           peut-être en cause. Une panne réseau (<= 0), jamais. */
        if (http_status >= 500 && ++st->server_failures >= DENGON_SHIP_POISON_LIMIT) {
            if (n <= 1) {
                s.remove   = true;
                s.rejected = true;
                s.delay_ms = period_ms;
                lot_traite(st);
            } else {
                isoler(st, n, &s);
            }
        }
        break;
    }
    return s;
}
