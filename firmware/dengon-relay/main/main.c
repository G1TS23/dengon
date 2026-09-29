// ---------------------------------------------------------------------------
// firmware/dengon-relay/main/main.c — point d'entrée du relais dengon.
//
// Ordre de démarrage (US-308) :
//   1. NVS (calibration PHY du contrôleur BT, secrets et curseur du relais) ;
//   2. relais dengon (dengon_relay_app.c) : littlefs, identité — tirée sous
//      bootloader_random_enable() au premier démarrage, donc AVANT la radio —
//      et reprise du journal chaîné ;
//   3. transport NimBLE (transport_nimble.c, US-220), avec le vrai peerID ;
//   4. auto-test Noise sur l'aléa matériel, tâches route / inventory /
//      courier / ledger ;
//   5. Wi-Fi station + SNTP, puis tâche d'export HTTPS du journal vers le
//      dashboard (US-309) — le relais fonctionne à l'identique sans eux ;
//   6. console série.
//
// Références : docs/synthese/08-relais-esp32.md §3,
//              docs/powl/03-network-protocol.md §6.
// ---------------------------------------------------------------------------
#include "esp_log.h"
#include "nvs_flash.h"
#include "sdkconfig.h"

#include "dengon_console.h"
#include "dengon_peer_id.h"
#include "dengon_relay_app.h"
#include "dengon_ship.h"
#include "dengon_transport.h"
#include "dengon_wifi.h"
#if CONFIG_DENGON_TRANSPORT_DEMO
#include "dengon_demo.h"
#endif

static const char *TAG = "dengon-relay";

void
app_main(void)
{
    dengon_transport_config_t cfg = {
        .advertise       = true,
        .scan            = true,
        /* Défaut du contrat Rust ; le transport le borne à
           CONFIG_BT_NIMBLE_MAX_CONNECTIONS (3) et le signale. */
        .max_connections = 8,
        /* docs/powl/03 §6.3 : 517 visé, repli automatique sur 23. */
        .preferred_mtu   = 517,
    };
    dengon_tr_err_t tr;
    esp_err_t err;

    /* NVS d'abord : le contrôleur BT y stocke sa calibration PHY. Le cycle
       erase/retry couvre une partition héritée d'un firmware précédent. */
    err = nvs_flash_init();
    if (err == ESP_ERR_NVS_NO_FREE_PAGES || err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_ERROR_CHECK(nvs_flash_erase());
        err = nvs_flash_init();
    }
    ESP_ERROR_CHECK(err);

#if CONFIG_DENGON_TRANSPORT_DEMO
    /* Démo US-220 : octets opaques, pas de dengon-core ; peerID bouchon. */
    ESP_ERROR_CHECK(dengon_peer_id_init());
    dengon_peer_id_get(cfg.local_peer_id);
#else
    ESP_ERROR_CHECK(dengon_relay_app_init(cfg.local_peer_id));
    dengon_peer_id_set(cfg.local_peer_id);
#endif

    tr = dengon_transport_start(&cfg);
    if (tr != DENGON_TR_OK) {
        ESP_LOGE(TAG, "démarrage du transport impossible : %s", dengon_tr_err_str(tr));
        return;
    }

#if CONFIG_DENGON_TRANSPORT_DEMO
    dengon_demo_start();
#else
    ESP_ERROR_CHECK(dengon_relay_app_start());
    /* Pas d'ESP_ERROR_CHECK : sans Wi-Fi, le relais doit continuer à
       relayer ; l'export restera simplement en attente. */
    if (dengon_wifi_start() != ESP_OK || dengon_ship_start() != ESP_OK) {
        ESP_LOGE(TAG, "export vers le dashboard indisponible");
    }
    ESP_ERROR_CHECK(dengon_console_start());
#endif
}
