// ---------------------------------------------------------------------------
// dengon_ship.c — voir dengon_ship.h (US-309).
// ---------------------------------------------------------------------------
#include "dengon_ship.h"

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "esp_http_client.h"
#include "esp_log.h"
#include "esp_random.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
#include "freertos/task.h"
#include "nvs.h"
#include "sdkconfig.h"

#include "dengon_core.h"
#include "dengon_relay_app.h"
#include "dengon_ring.h"
#include "dengon_ship_policy.h"
#include "dengon_wifi.h"

static const char *TAG = "dengon-ship";

#define NVS_NS      "dengon_net"
#define NVS_JWT     "jwt"
#define JWT_MAX     1024
/* 12 Ko : le handshake TLS (mbedTLS, ECDSA) passe sur cette pile. Avec 8 Ko,
   marge mesurée sur carte : 1 188 o seulement (revue PR #120, point 10). */
#define SHIP_STACK  12288
#define SHIP_PRIO   2
/* Lot d'entrées binaires passé à dengon_relay_build_batch : une entrée de
   journal pèse ~150 o + nom + payload (< 400 o en pratique). */
#define ENTRIES_BUF 6144
#define HTTP_TIMEOUT_MS 15000
#define IDLE_WAIT_MS    2000

/* Un booléen Kconfig à `n` n'est pas défini du tout côté C. */
#ifdef CONFIG_DENGON_DASH_SKIP_CN_CHECK
#define SKIP_CN_CHECK true
#else
#define SKIP_CN_CHECK false
#endif

/* Racine de l'autorité du dashboard (Caddy `tls internal`), embarquée au
   build (main/CMakeLists.txt) ; vide si main/certs/dashboard_root.pem était
   absent : l'export est alors coupé, le ring continue de se remplir. */
extern const char dashboard_root_pem_start[] asm("_binary_dashboard_root_pem_start");

static uint8_t       s_ring_buf[CONFIG_DENGON_SHIP_RING_BYTES];
static TaskHandle_t  s_task;
static volatile bool s_new_token;

/* Tout ce qui suit est partagé entre la tâche d'envoi, `ledger_task`
   (push), la console (jeton, stats) et `courier_task` (relay.health) :
   toujours sous `s_lock`. Les compteurs 64 bits ne sont pas atomiques sur
   Xtensa 32 bits. */
static SemaphoreHandle_t s_lock;
static dengon_ring_t     s_ring;
static char              s_jwt[JWT_MAX];
static uint64_t          s_sent;
static uint64_t          s_rejected;
static int               s_last_status;

// --- Ring ---------------------------------------------------------------------

esp_err_t
dengon_ship_init(void)
{
    nvs_handle_t h;
    size_t       len = sizeof(s_jwt);

    s_lock = xSemaphoreCreateMutex();
    if (s_lock == NULL) {
        return ESP_ERR_NO_MEM;
    }
    dengon_ring_init(&s_ring, s_ring_buf, sizeof(s_ring_buf));
    if (nvs_open(NVS_NS, NVS_READONLY, &h) == ESP_OK) {
        if (nvs_get_str(h, NVS_JWT, s_jwt, &len) != ESP_OK) {
            s_jwt[0] = '\0';
        }
        nvs_close(h);
    }
    return ESP_OK;
}

void
dengon_ship_push(const uint8_t *entry, size_t len)
{
    if (s_lock == NULL) {
        return;
    }
    xSemaphoreTake(s_lock, portMAX_DELAY);
    dengon_ring_push(&s_ring, entry, len);
    xSemaphoreGive(s_lock);
    if (s_task != NULL) {
        xTaskNotifyGive(s_task);
    }
}

void
dengon_ship_get_stats(dengon_ship_stats_t *out)
{
    memset(out, 0, sizeof(*out));
    if (s_lock != NULL) {
        xSemaphoreTake(s_lock, portMAX_DELAY);
        out->fill_pct = dengon_ring_fill_pct(&s_ring);
        out->pending  = dengon_ring_count(&s_ring);
        out->dropped     = s_ring.dropped + s_rejected;
        out->sent        = s_sent;
        out->last_status = s_last_status;
        out->has_token   = s_jwt[0] != '\0';
        xSemaphoreGive(s_lock);
    }
    out->has_root_ca = dashboard_root_pem_start[0] != '\0';
    out->stack_min   = s_task != NULL ? (unsigned)uxTaskGetStackHighWaterMark(s_task) : 0;
}

esp_err_t
dengon_ship_set_token(const char *jwt)
{
    nvs_handle_t h;
    esp_err_t    err;

    if (jwt == NULL || jwt[0] == '\0' || strlen(jwt) >= sizeof(s_jwt)) {
        return ESP_ERR_INVALID_ARG;
    }
    err = nvs_open(NVS_NS, NVS_READWRITE, &h);
    if (err != ESP_OK) {
        return err;
    }
    err = nvs_set_str(h, NVS_JWT, jwt);
    if (err == ESP_OK) {
        err = nvs_commit(h);
    }
    nvs_close(h);
    if (err == ESP_OK) {
        xSemaphoreTake(s_lock, portMAX_DELAY);
        strlcpy(s_jwt, jwt, sizeof(s_jwt));
        xSemaphoreGive(s_lock);
        s_new_token = true;
        if (s_task != NULL) {
            xTaskNotifyGive(s_task);
        }
    }
    return err;
}

// --- Envoi --------------------------------------------------------------------

/* POST du corps ; rend le statut HTTP, ou -1 sans réponse. */
static int
poster(const uint8_t *body, size_t len)
{
    static char auth[sizeof("Bearer ") + JWT_MAX];
    int         status = -1;
    esp_err_t   err;

    esp_http_client_config_t cfg = {
        .url        = CONFIG_DENGON_DASH_URL "/ingest/batch",
        .method     = HTTP_METHOD_POST,
        .cert_pem   = dashboard_root_pem_start,
        .timeout_ms = HTTP_TIMEOUT_MS,
        /* Certificat au nom d'une IP (SAN IP, pas de CN) : voir Kconfig. La
           chaîne reste vérifiée jusqu'à la racine épinglée. */
        .skip_cert_common_name_check = SKIP_CN_CHECK,
        .keep_alive_enable           = false,
    };
    esp_http_client_handle_t c = esp_http_client_init(&cfg);

    if (c == NULL) {
        return -1;
    }
    xSemaphoreTake(s_lock, portMAX_DELAY);
    snprintf(auth, sizeof(auth), "Bearer %s", s_jwt);
    xSemaphoreGive(s_lock);
    esp_http_client_set_header(c, "Authorization", auth);
    esp_http_client_set_header(c, "Content-Type", "application/json");
    esp_http_client_set_post_field(c, (const char *)body, (int)len);
    err = esp_http_client_perform(c);
    if (err == ESP_OK) {
        status = esp_http_client_get_status_code(c);
    } else {
        ESP_LOGW(TAG, "POST sans réponse : %s", esp_err_to_name(err));
    }
    esp_http_client_cleanup(c);
    return status;
}

/* Construit le batch signé ; NULL (et `*st`) si impossible. À libérer.

   Une seule construction (canonicalisation + SHA-256 + Ed25519, sous le
   mutex du relais) dans le cas courant : le JSON d'une entrée pèse son
   binaire (~150 o de hash/signature en moins) plus ~250 o d'enveloppe
   (`event_id`, `node_id`, clés), d'où l'estimation. Si un payload très
   échappé la dépasse, `BUFFER_TOO_SMALL` rend la taille exacte et on
   reconstruit une fois. */
static uint8_t *
construire(const uint8_t *entries, size_t len, size_t n, size_t *body_len, DengonStatus *st)
{
    size_t   cap  = len + n * 256 + 256;
    uint8_t *body = malloc(cap);

    if (body == NULL) {
        *st = DENGON_STATUS_NULL_POINTER;
        return NULL;
    }
    DengonRelay *r = dengon_relay_app_lock();
    *st            = dengon_relay_build_batch(r, entries, len, body, cap, body_len);
    if (*st == DENGON_STATUS_BUFFER_TOO_SMALL) {
        uint8_t *plus = realloc(body, *body_len);

        if (plus == NULL) {
            *st = DENGON_STATUS_NULL_POINTER;
        } else {
            body = plus;
            cap  = *body_len;
            *st  = dengon_relay_build_batch(r, entries, len, body, cap, body_len);
        }
    }
    dengon_relay_app_unlock();
    if (*st != DENGON_STATUS_OK) {
        free(body);
        return NULL;
    }
    return body;
}

static void
retirer(uint64_t first, size_t n, bool rejete)
{
    xSemaphoreTake(s_lock, portMAX_DELAY);
    size_t done = dengon_ring_commit(&s_ring, first, n);
    if (rejete) {
        s_rejected += done;
    } else {
        s_sent += done;
    }
    xSemaphoreGive(s_lock);
}

/* Après un 401 : on ne réessaie qu'au bout de `ms`, ou dès qu'un nouveau
   jeton est posé — pas à chaque nouvelle entrée du journal. */
static void
attendre_jeton(uint32_t ms)
{
    for (uint32_t t = 0; t < ms && !s_new_token; t += 1000) {
        vTaskDelay(pdMS_TO_TICKS(1000));
    }
    s_new_token = false;
}

static bool
export_possible(void)
{
    bool jeton;

    xSemaphoreTake(s_lock, portMAX_DELAY);
    jeton = s_jwt[0] != '\0';
    xSemaphoreGive(s_lock);
    return jeton && dashboard_root_pem_start[0] != '\0';
}

static void
journaliser_issue(const dengon_ship_step_t *step, size_t n, int status)
{
    if (step->isolate) {
        ESP_LOGW(TAG, "lot de %u entrées suspect (%s) : reprise entrée par entrée",
                 (unsigned)n, status > 0 ? "5xx répétés" : "hors contrat");
        return;
    }
    switch (step->action) {
    case DENGON_SHIP_COMMIT:
        ESP_LOGI(TAG, "%u événements acceptés (%d)", (unsigned)n, status);
        break;
    case DENGON_SHIP_DROP:
        ESP_LOGE(TAG, "lot de %u événements refusé (%d) : retiré", (unsigned)n, status);
        break;
    case DENGON_SHIP_AUTH:
        /* Le dashboard rend 401 pour plusieurs causes (dashboard/api,
           ingest.py) : on ne peut pas les distinguer ici. */
        ESP_LOGW(TAG,
                 "%d : jeton expiré, relais inconnu du dashboard (ré-enregistré avec une "
                 "autre clé ?) ou signature refusée ; vérifier `dash id` puis `dash token`",
                 status);
        break;
    case DENGON_SHIP_HOLD:
        ESP_LOGW(TAG, "réponse inattendue %d (URL, proxy ?) : lot gardé, réessai dans 60 s",
                 status);
        break;
    case DENGON_SHIP_RETRY:
        if (step->remove) {
            ESP_LOGE(TAG, "entrée en échec serveur répété (%d) : retirée", status);
        } else {
            ESP_LOGW(TAG, "envoi en échec (%d), réessai dans %" PRIu32 " ms", status,
                     step->delay_ms);
        }
        break;
    }
}

static void
ship_task(void *arg)
{
    static uint8_t      entries[ENTRIES_BUF];
    dengon_ship_state_t etat;
    bool                avertir = true;

    (void)arg;
    dengon_ship_state_init(&etat);
    for (;;) {
        uint64_t           first    = 0;
        size_t             len      = 0;
        size_t             body_len = 0;
        size_t             n;
        uint8_t           *body;
        DengonStatus       st;
        int                status = 0;
        dengon_ship_step_t step;

        if (!export_possible()) {
            if (avertir) {
                ESP_LOGW(TAG, "export coupé : %s",
                         dashboard_root_pem_start[0] == '\0'
                             ? "racine du dashboard absente du firmware "
                               "(main/certs/dashboard_root.pem)"
                             : "pas de jeton (`dash token <jwt>`)");
                avertir = false;
            }
            ulTaskNotifyTake(pdTRUE, pdMS_TO_TICKS(IDLE_WAIT_MS));
            continue;
        }
        avertir = true;
        if (!dengon_wifi_wait_up(pdMS_TO_TICKS(IDLE_WAIT_MS))) {
            continue;
        }

        xSemaphoreTake(s_lock, portMAX_DELAY);
        n = dengon_ring_peek(&s_ring, dengon_ship_batch_limit(&etat, CONFIG_DENGON_SHIP_BATCH_MAX),
                             entries, sizeof(entries), &len, &first);
        xSemaphoreGive(s_lock);
        if (n == 0) {
            if (len > 0) {
                ESP_LOGE(TAG, "entrée de %u o plus grande que le lot : retirée", (unsigned)len);
                retirer(first, 1, true);
            } else {
                ulTaskNotifyTake(pdTRUE, pdMS_TO_TICKS(IDLE_WAIT_MS));
            }
            continue;
        }

        body = construire(entries, len, n, &body_len, &st);
        if (body == NULL && st != DENGON_STATUS_DECODE) {
            ESP_LOGE(TAG, "batch non construit (%d), réessai", (int)st);
            vTaskDelay(pdMS_TO_TICKS(DENGON_SHIP_BACKOFF_MIN_MS));
            continue;
        }
        if (body != NULL) {
            status = poster(body, body_len);
            free(body);
            xSemaphoreTake(s_lock, portMAX_DELAY);
            s_last_status = status;
            xSemaphoreGive(s_lock);
        }

        step = dengon_ship_step(&etat, status, body == NULL, n, CONFIG_DENGON_SHIP_PERIOD_MS,
                                esp_random());
        journaliser_issue(&step, n, status);
        if (step.remove) {
            retirer(first, n, step.rejected);
        }
        if (step.action == DENGON_SHIP_AUTH) {
            /* Réveil anticipé si un nouveau jeton est posé. */
            attendre_jeton(step.delay_ms);
        } else if (step.delay_ms > 0) {
            /* Délai ferme : une nouvelle entrée n'apprend rien sur le réseau. */
            vTaskDelay(pdMS_TO_TICKS(step.delay_ms));
        }
    }
}

esp_err_t
dengon_ship_start(void)
{
    if (xTaskCreate(ship_task, "dengon_ship", SHIP_STACK, NULL, SHIP_PRIO, &s_task) != pdPASS) {
        return ESP_ERR_NO_MEM;
    }
    return ESP_OK;
}
