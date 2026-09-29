// ---------------------------------------------------------------------------
// dengon_demo — tâche de démonstration du transport (US-220).
//
// Elle joue le rôle que tiendra dengon-core en US-308 : elle poll le
// transport, et elle lui fait émettre des octets. Elle ne comprend rien au
// protocole dengon — et c'est voulu : le critère de l'US est de relayer des
// octets OPAQUES entre deux cartes.
//
// Ce qu'elle prouve, dans `idf.py monitor` des deux cartes :
//   - chaque événement du contrat (PeerConnected / FrameReceived /
//     PeerDisconnected + motif) ;
//   - l'intégrité des octets : chaque trame porte un motif déterministe que le
//     récepteur recalcule (« motif intact »), plus un CRC32 comparable d'une
//     carte à l'autre ;
//   - la limite de trame : à chaque nouveau lien, une trame de
//     DENGON_TC_FRAME_MAX octets (acceptée, en morceaux L1 depuis l'US-312)
//     puis un octet de plus (FRAME_TOO_LARGE) ;
//   - la règle n°4 : un send sur un lien qui vient de tomber rend UNKNOWN_PEER ;
//   - bouton BOOT (GPIO0) : fermeture propre de tous les liens (Locale ici,
//     Propre en face).
//
// Désactivable par CONFIG_DENGON_TRANSPORT_DEMO (US-308 la remplacera).
// ---------------------------------------------------------------------------
#include "sdkconfig.h"

/* Démo US-220, compilée seulement si elle remplace le relais (Kconfig). */
#if CONFIG_DENGON_TRANSPORT_DEMO

#include <string.h>

#include "driver/gpio.h"
#include "esp_log.h"
#include "esp_rom_crc.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

#include "dengon_demo.h"
#include "dengon_peer_id.h"
#include "dengon_transport.h"

static const char *TAG = "dengon-demo";

#define DEMO_POLL_MS    100
#define DEMO_BATCH      8
#define DEMO_SMALL_LEN  64
#define DEMO_BOOT_GPIO  GPIO_NUM_0

/* En-tête de la trame de démo : "DGN0" ‖ seq (4, BE) ‖ peerID[0..4] de
   l'émetteur, puis motif octet[i] = (uint8_t)(seq + i). Ce format n'existe
   QUE dans cette démo : pour le transport, ce sont des octets opaques. */
#define DEMO_HDR_LEN 12
static const uint8_t DEMO_MAGIC[4] = { 'D', 'G', 'N', '0' };

static uint8_t s_prefix[4];
static uint8_t s_buf[DENGON_TC_FRAME_MAX + 1];

static size_t
demo_build(uint8_t *out, size_t len, uint32_t seq)
{
    if (len < DEMO_HDR_LEN) {
        return 0;
    }
    memcpy(out, DEMO_MAGIC, 4);
    out[4] = (uint8_t)(seq >> 24);
    out[5] = (uint8_t)(seq >> 16);
    out[6] = (uint8_t)(seq >> 8);
    out[7] = (uint8_t)seq;
    memcpy(&out[8], s_prefix, 4);
    for (size_t i = DEMO_HDR_LEN; i < len; i++) {
        out[i] = (uint8_t)(seq + i);
    }
    return len;
}

static void
demo_log_frame(const dengon_transport_event_t *ev)
{
    const uint8_t *b = ev->u.frame.bytes;
    size_t len = ev->u.frame.len;
    uint32_t crc = esp_rom_crc32_le(0, b, len);
    uint32_t seq;
    bool intact = true;

    if (len < DEMO_HDR_LEN || memcmp(b, DEMO_MAGIC, 4) != 0) {
        ESP_LOGI(TAG, "FrameReceived link#%llu : %u o, crc32=%08lx (hors démo)",
                 (unsigned long long)ev->link, (unsigned)len, (unsigned long)crc);
        return;
    }

    seq = ((uint32_t)b[4] << 24) | ((uint32_t)b[5] << 16) | ((uint32_t)b[6] << 8) | b[7];
    for (size_t i = DEMO_HDR_LEN; i < len; i++) {
        if (b[i] != (uint8_t)(seq + i)) {
            intact = false;
            break;
        }
    }
    ESP_LOGI(TAG, "FrameReceived link#%llu : %u o de %02x%02x%02x%02x, seq=%lu, "
                  "crc32=%08lx, motif %s",
             (unsigned long long)ev->link, (unsigned)len, b[8], b[9], b[10], b[11],
             (unsigned long)seq, (unsigned long)crc, intact ? "intact" : "CORROMPU");
}

/* À chaque nouveau lien : la plus grande trame acceptée, puis un octet de trop. */
static void
demo_probe_mtu(dengon_link_id_t link, uint32_t seq)
{
    size_t max = 0;
    dengon_tr_err_t err;

    /* On tente 514 ; si le lien a un MTU plus petit, le transport dit combien. */
    err = dengon_transport_send(link, s_buf, demo_build(s_buf, DENGON_TC_FRAME_MAX, seq), &max);
    if (err == DENGON_TR_FRAME_TOO_LARGE) {
        err = dengon_transport_send(link, s_buf, demo_build(s_buf, max, seq), NULL);
    } else {
        max = DENGON_TC_FRAME_MAX;
    }
    ESP_LOGI(TAG, "sonde link#%llu : trame de %u o (max) -> %s",
             (unsigned long long)link, (unsigned)max, dengon_tr_err_str(err));

    err = dengon_transport_send(link, s_buf, demo_build(s_buf, max + 1, seq), NULL);
    ESP_LOGI(TAG, "sonde link#%llu : trame de %u o (max + 1) -> %s (attendu : trame trop grande)",
             (unsigned long long)link, (unsigned)(max + 1), dengon_tr_err_str(err));
}

static void
demo_task(void *arg)
{
    dengon_transport_event_t ev[DEMO_BATCH];
    const TickType_t period = pdMS_TO_TICKS(CONFIG_DENGON_DEMO_PERIOD_MS);
    TickType_t last = xTaskGetTickCount();
    uint32_t seq = 0;
    int boot_prev = 1;

    (void)arg;

    for (;;) {
        size_t n = dengon_transport_poll(ev, DEMO_BATCH);

        for (size_t i = 0; i < n; i++) {
            switch (ev[i].kind) {
            case DENGON_EVT_PEER_CONNECTED:
                if (ev[i].u.connected.has_rssi) {
                    ESP_LOGI(TAG, "PeerConnected link#%llu (rssi=%d dBm)",
                             (unsigned long long)ev[i].link, ev[i].u.connected.rssi);
                } else {
                    ESP_LOGI(TAG, "PeerConnected link#%llu (rssi inconnu)",
                             (unsigned long long)ev[i].link);
                }
                demo_probe_mtu(ev[i].link, seq++);
                break;

            case DENGON_EVT_PEER_DISCONNECTED: {
                dengon_tr_err_t err;

                ESP_LOGI(TAG, "PeerDisconnected link#%llu, motif %s",
                         (unsigned long long)ev[i].link, dengon_disc_reason_str(ev[i].u.reason));
                /* Règle n°4 du contrat, observée sur matériel. */
                err = dengon_transport_send(ev[i].link, s_buf, 1, NULL);
                ESP_LOGI(TAG, "send sur link#%llu fermé -> %s (attendu : pair inconnu)",
                         (unsigned long long)ev[i].link, dengon_tr_err_str(err));
                break;
            }

            case DENGON_EVT_FRAME_RECEIVED:
                demo_log_frame(&ev[i]);
                break;
            }
            dengon_transport_event_free(&ev[i]);
        }

        if (xTaskGetTickCount() - last >= period) {
            last = xTaskGetTickCount();
            (void)dengon_transport_broadcast(s_buf, demo_build(s_buf, DEMO_SMALL_LEN, seq));
            ESP_LOGD(TAG, "broadcast seq=%lu", (unsigned long)seq);
            seq++;
        }

        /* Front descendant sur BOOT : fermeture propre de tous les liens. */
        {
            int boot = gpio_get_level(DEMO_BOOT_GPIO);

            if (boot_prev == 1 && boot == 0) {
                ESP_LOGI(TAG, "bouton BOOT : fermeture de tous les liens");
                dengon_transport_disconnect_all();
            }
            boot_prev = boot;
        }

        vTaskDelay(pdMS_TO_TICKS(DEMO_POLL_MS));
    }
}

void
dengon_demo_start(void)
{
    uint8_t peer_id[DENGON_PEER_ID_LEN];
    const gpio_config_t boot = {
        .pin_bit_mask = 1ULL << DEMO_BOOT_GPIO,
        .mode = GPIO_MODE_INPUT,
        .pull_up_en = GPIO_PULLUP_ENABLE,
    };

    dengon_peer_id_get(peer_id);
    memcpy(s_prefix, peer_id, sizeof(s_prefix));

    if (gpio_config(&boot) != ESP_OK) {
        ESP_LOGW(TAG, "GPIO0 indisponible : pas de fermeture au bouton");
    }

    /* 4 Ko : ESP_LOG + snprintf internes ; la trame de 515 o est statique. */
    if (xTaskCreate(demo_task, "dengon_demo", 4096, NULL, 5, NULL) != pdPASS) {
        ESP_LOGE(TAG, "création de la tâche de démo impossible");
    }
}

#endif /* CONFIG_DENGON_TRANSPORT_DEMO */
