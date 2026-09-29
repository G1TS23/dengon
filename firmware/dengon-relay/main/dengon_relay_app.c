// ---------------------------------------------------------------------------
// dengon_relay_app.c — voir dengon_relay_app.h (US-308).
// ---------------------------------------------------------------------------
#include "dengon_relay_app.h"

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>

#include "esp_app_desc.h"
#include "esp_flash_encrypt.h"
#include "esp_log.h"
#include "esp_random.h"
#include "esp_secure_boot.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
#include "freertos/task.h"

#include "dengon_store.h"
#include "dengon_transport.h"

static const char *TAG = "dengon-relay";

#define ROUTE_PERIOD_MS      10
#define INVENTORY_PERIOD_MS  1000
#define COURIER_PERIOD_MS    30000
#define LEDGER_WAIT_MS       2000
#define RX_BATCH             8
/* Une trame BLE tient toujours ici (ATT_MTU 517 - 3) ; au-delà, voir
   flush_outgoing(). */
#define FRAME_BUF            (DENGON_TC_FRAME_MAX + 8)
/* Lot d'entrées de journal écrit d'un coup (un fsync par lot). */
#define LEDGER_BATCH         4096

/* Piles des tâches qui appellent dengon-core. Toute entrée de journal est
   signée (Ed25519) et un ANNOUNCE aussi : 6 Ko débordaient au premier
   link_up (« stack overflow in task dengon_route », vu sur carte,
   2026-09-29). Marges mesurées : voir le bilan de santé (`pile_min`). */
#define ROUTE_STACK      16384
#define COURIER_STACK    16384
#define INVENTORY_STACK  8192
#define LEDGER_STACK     8192

static DengonRelay      *s_relay;
static SemaphoreHandle_t s_lock;
static TaskHandle_t      s_ledger_task;
static TaskHandle_t      s_route_task;
static TaskHandle_t      s_inventory_task;

// --- Horloges ---------------------------------------------------------------

static DengonNow
maintenant(void)
{
    struct timeval tv;

    /* Heure murale : sans SNTP (US-309), gettimeofday() part de 1970 et le
       relais apprend l'heure du premier ANNOUNCE authentique reçu. */
    gettimeofday(&tv, NULL);
    return (DengonNow){
        .wall_ms = (uint64_t)tv.tv_sec * 1000u + (uint64_t)tv.tv_usec / 1000u,
        .mono_ms = (uint64_t)(esp_timer_get_time() / 1000),
    };
}

DengonRelay *
dengon_relay_app_lock(void)
{
    xSemaphoreTake(s_lock, portMAX_DELAY);
    return s_relay;
}

void
dengon_relay_app_unlock(void)
{
    xSemaphoreGive(s_lock);
}

// --- Sorties ----------------------------------------------------------------

/* Vide la file de trames à émettre, une par une, HORS du mutex pendant
   l'envoi. `buf` : tampon propre à la tâche appelante. */
static void
flush_outgoing(uint8_t *buf, size_t cap)
{
    for (;;) {
        uint64_t     link = 0;
        size_t       len  = 0;
        uint8_t     *frame = buf;
        DengonStatus st;
        size_t       max_out = 0;
        dengon_tr_err_t tr;

        dengon_relay_app_lock();
        st = dengon_relay_pop_outgoing(s_relay, &link, buf, cap, &len);
        if (st == DENGON_STATUS_BUFFER_TOO_SMALL) {
            /* Ne devrait pas arriver (le relais borne ses trames à 514
               octets) : on la retire quand même pour ne pas bloquer la file. */
            frame = malloc(len);
            st    = frame == NULL ? DENGON_STATUS_NULL_POINTER
                                  : dengon_relay_pop_outgoing(s_relay, &link, frame, len, &len);
        }
        dengon_relay_app_unlock();

        if (st != DENGON_STATUS_OK) {
            if (frame != buf) {
                free(frame);
            }
            return;
        }
        tr = dengon_transport_send(link, frame, len, &max_out);
        if (tr != DENGON_TR_OK) {
            ESP_LOGW(TAG, "trame de %u o vers le lien %" PRIu64 " non émise : %s (max %u)",
                     (unsigned)len, link, dengon_tr_err_str(tr), (unsigned)max_out);
        }
        if (frame != buf) {
            free(frame);
        }
    }
}

static void
notifier_journal(void)
{
    if (s_ledger_task != NULL) {
        xTaskNotifyGive(s_ledger_task);
    }
}

// --- Tâches -----------------------------------------------------------------

static void
route_task(void *arg)
{
    static uint8_t           buf[FRAME_BUF];
    dengon_transport_event_t evs[RX_BATCH];

    (void)arg;
    for (;;) {
        size_t    n   = dengon_transport_poll(evs, RX_BATCH);
        DengonNow now = maintenant();

        dengon_relay_app_lock();
        for (size_t i = 0; i < n; i++) {
            const dengon_transport_event_t *ev = &evs[i];

            switch (ev->kind) {
            case DENGON_EVT_PEER_CONNECTED:
                ESP_LOGI(TAG, "lien %" PRIu64 " ouvert", ev->link);
                dengon_relay_link_up(s_relay, ev->link, now);
                break;
            case DENGON_EVT_PEER_DISCONNECTED:
                ESP_LOGI(TAG, "lien %" PRIu64 " fermé", ev->link);
                dengon_relay_link_down(s_relay, ev->link);
                break;
            case DENGON_EVT_FRAME_RECEIVED:
                dengon_relay_on_frame(s_relay, ev->link, ev->u.frame.bytes, ev->u.frame.len,
                                      now);
                break;
            }
        }
        dengon_relay_poll_routing(s_relay, now);
        dengon_relay_app_unlock();

        for (size_t i = 0; i < n; i++) {
            dengon_transport_event_free(&evs[i]);
        }
        flush_outgoing(buf, sizeof(buf));
        if (n > 0) {
            notifier_journal();
        } else {
            vTaskDelay(pdMS_TO_TICKS(ROUTE_PERIOD_MS));
        }
    }
}

static void
inventory_task(void *arg)
{
    static uint8_t buf[FRAME_BUF];

    (void)arg;
    for (;;) {
        vTaskDelay(pdMS_TO_TICKS(INVENTORY_PERIOD_MS));
        dengon_relay_app_lock();
        dengon_relay_poll_inventory(s_relay, maintenant());
        dengon_relay_app_unlock();
        flush_outgoing(buf, sizeof(buf));
    }
}

static void
courier_task(void *arg)
{
    static uint8_t   buf[FRAME_BUF];
    DengonRelayStats st;

    (void)arg;
    for (;;) {
        /* Plus petite marge de pile jamais vue, en octets (ESP-IDF : le
           « mot » de FreeRTOS vaut un octet). */
        UBaseType_t pile_route = uxTaskGetStackHighWaterMark(s_route_task);
        UBaseType_t pile_courier = uxTaskGetStackHighWaterMark(NULL);
        UBaseType_t pile_inventory = uxTaskGetStackHighWaterMark(s_inventory_task);
        UBaseType_t pile_ledger = uxTaskGetStackHighWaterMark(s_ledger_task);

        vTaskDelay(pdMS_TO_TICKS(COURIER_PERIOD_MS));
        dengon_relay_app_lock();
        dengon_relay_poll_courier(s_relay, maintenant());
        dengon_relay_stats(s_relay, &st);
        dengon_relay_app_unlock();
        flush_outgoing(buf, sizeof(buf));
        notifier_journal();
        ESP_LOGI(TAG,
                 "santé : relayés=%" PRIu64 " enveloppes=%" PRIu32 " (déposées %" PRIu64
                 ", remises %" PRIu64 ") cache=%" PRIu32 " rejets=%" PRIu64 "/%" PRIu64
                 " sans_heure=%" PRIu64 " tas=%" PRIu32
                 " pile_min route/courier/inventory/ledger=%u/%u/%u/%u",
                 st.relayed, st.envelopes_held, st.envelopes_stored, st.envelopes_handed_off,
                 st.cache_len, st.malformed, st.unauthentic, st.clock_unknown,
                 esp_get_free_heap_size(), (unsigned)pile_route, (unsigned)pile_courier,
                 (unsigned)pile_inventory, (unsigned)pile_ledger);
    }
}

/* Écrit dans littlefs toutes les entrées produites, par lots, puis le
   curseur. L'ancre est recalculée depuis la DERNIÈRE entrée écrite du lot
   (seq + 1, entry_hash) : le curseur ne devance jamais le fichier. */
static void
ledger_task(void *arg)
{
    static uint8_t lot[LEDGER_BATCH];

    (void)arg;
    for (;;) {
        size_t       used = 0;
        size_t       last = 0;
        size_t       len  = 0;
        DengonStatus st   = DENGON_STATUS_OK;

        ulTaskNotifyTake(pdTRUE, pdMS_TO_TICKS(LEDGER_WAIT_MS));
        do {
            used = 0;
            dengon_relay_app_lock();
            while (used < sizeof(lot)) {
                st = dengon_relay_pop_ledger(s_relay, lot + used, sizeof(lot) - used, &len);
                if (st != DENGON_STATUS_OK) {
                    break;
                }
                last = used;
                used += len;
            }
            dengon_relay_app_unlock();

            if (used > 0) {
                const uint8_t *e   = lot + last;
                uint64_t       seq = 0;

                for (int i = 0; i < 8; i++) {
                    seq = (seq << 8) | e[i];
                }
                /* entry_hash : 32 octets avant la signature (64). */
                if (dengon_store_append_ledger(lot, used, seq + 1,
                                               lot + used - 64 - DENGON_LEDGER_HASH_LEN)
                    != ESP_OK) {
                    ESP_LOGE(TAG, "journal : écriture perdue (%u o)", (unsigned)used);
                }
            }
            /* Lot plein : il en reste, on recommence sans attendre. */
        } while (st == DENGON_STATUS_BUFFER_TOO_SMALL && used > 0);
    }
}

// --- Démarrage ---------------------------------------------------------------

static const char *
raison_reset(void)
{
    switch (esp_reset_reason()) {
    case ESP_RST_POWERON:   return "poweron";
    case ESP_RST_SW:        return "software";
    case ESP_RST_PANIC:     return "panic";
    case ESP_RST_INT_WDT:   return "int_wdt";
    case ESP_RST_TASK_WDT:  return "task_wdt";
    case ESP_RST_WDT:       return "wdt";
    case ESP_RST_DEEPSLEEP: return "deepsleep";
    case ESP_RST_BROWNOUT:  return "brownout";
    case ESP_RST_EXT:       return "external";
    default:                return "unknown";
    }
}

esp_err_t
dengon_relay_app_init(uint8_t peer_id[8])
{
    uint8_t              dh[DENGON_SECRET_LEN];
    uint8_t              seed[DENGON_SECRET_LEN];
    uint8_t              pk[32];
    char                 pseudo[16];
    char                 boot[160];
    bool                 created = false;
    size_t               truncated = 0;
    dengon_ledger_tail_t anchor;
    uint64_t             routing_seed;
    esp_err_t            err;

    s_lock = xSemaphoreCreateMutex();
    if (s_lock == NULL) {
        return ESP_ERR_NO_MEM;
    }
    err = dengon_store_init();
    if (err == ESP_OK) {
        err = dengon_store_load_secrets(dh, seed, &created);
    }
    if (err == ESP_OK) {
        err = dengon_store_boot_anchor(&anchor, &truncated);
    }
    if (err != ESP_OK) {
        return err;
    }
    if (truncated > 0) {
        ESP_LOGW(TAG, "journal : %u o d'écriture interrompue retirés", (unsigned)truncated);
    }

    /* Graine du jitter de routage : pas un secret, dérivée des secrets pour
       rester reproductible par carte sans solliciter l'aléa. */
    memcpy(&routing_seed, seed + 24, sizeof(routing_seed));
    s_relay = dengon_relay_new(dh, seed, anchor.next_seq,
                               anchor.has_entries ? anchor.last_hash : NULL, NULL,
                               routing_seed);
    memset(dh, 0, sizeof(dh));
    memset(seed, 0, sizeof(seed));
    if (s_relay == NULL) {
        return ESP_ERR_NO_MEM;
    }
    dengon_relay_peer_id(s_relay, peer_id);
    dengon_relay_verifying_key(s_relay, pk);
    snprintf(pseudo, sizeof(pseudo), "relais-%02x%02x", peer_id[0], peer_id[1]);

    ESP_LOGI(TAG, "%s peerID=%02x%02x%02x%02x%02x%02x%02x%02x (%s), journal reprend à seq=%" PRIu64,
             pseudo, peer_id[0], peer_id[1], peer_id[2], peer_id[3], peer_id[4], peer_id[5],
             peer_id[6], peer_id[7], created ? "nouvelle identité" : "identité relue",
             anchor.next_seq);
    ESP_LOGI(TAG,
             "clé de journal (dengon-verify --pubkey) = "
             "%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x"
             "%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x",
             pk[0], pk[1], pk[2], pk[3], pk[4], pk[5], pk[6], pk[7], pk[8], pk[9], pk[10],
             pk[11], pk[12], pk[13], pk[14], pk[15], pk[16], pk[17], pk[18], pk[19], pk[20],
             pk[21], pk[22], pk[23], pk[24], pk[25], pk[26], pk[27], pk[28], pk[29], pk[30],
             pk[31]);

    snprintf(boot, sizeof(boot),
             "{\"flash_enc\":%s,\"fw_version\":\"%s\",\"reset_reason\":\"%s\","
             "\"secure_boot\":%s}",
             esp_flash_encryption_enabled() ? "true" : "false", esp_app_get_description()->version,
             raison_reset(), esp_secure_boot_enabled() ? "true" : "false");
    if (!dengon_relay_record_event(s_relay, "relay.boot", boot, maintenant())) {
        ESP_LOGW(TAG, "relay.boot refusé par le journal");
    }
    return ESP_OK;
}

/* esp_fill_random() : vrai aléa matériel tant que la radio est allumée. */
static void
alea_materiel(uint8_t *buf, uintptr_t len)
{
    esp_fill_random(buf, len);
}

esp_err_t
dengon_relay_app_start(void)
{
    DengonStatus st = dengon_noise_selftest(alea_materiel);

    if (st != DENGON_STATUS_OK) {
        ESP_LOGE(TAG, "auto-test Noise en échec (%u) : pas d'aléa pour snow", (unsigned)st);
        return ESP_FAIL;
    }
    ESP_LOGI(TAG, "auto-test Noise XX sur esp_fill_random : OK");

    if (xTaskCreate(ledger_task, "dengon_ledger", LEDGER_STACK, NULL, 4, &s_ledger_task) != pdPASS
        || xTaskCreate(route_task, "dengon_route", ROUTE_STACK, NULL, 6, &s_route_task) != pdPASS
        || xTaskCreate(inventory_task, "dengon_inventory", INVENTORY_STACK, NULL, 5,
                       &s_inventory_task)
               != pdPASS
        || xTaskCreate(courier_task, "dengon_courier", COURIER_STACK, NULL, 3, NULL) != pdPASS) {
        return ESP_ERR_NO_MEM;
    }
    notifier_journal(); /* relay.boot */
    return ESP_OK;
}
