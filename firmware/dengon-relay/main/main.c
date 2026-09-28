// ---------------------------------------------------------------------------
// firmware/dengon-relay/main/main.c — point d'entrée du relais dengon.
//
// Depuis l'US-220, tout le Bluetooth vit dans transport_nimble.c : annonce,
// scan, connexions dans les deux rôles, émission et réception d'octets
// opaques. main.c ne fait plus que préparer ce dont le transport a besoin et
// le démarrer. Ce qui n'est PAS ici, et où ça ira :
//   - libdengon_core.a en FFI, vraie identité ....... US-307
//   - boucle du cœur (routage, relais dengon) ....... US-308
//   - Wi-Fi, HTTPS, journal chaîné .................. US-309 et suivantes
//
// Références : docs/synthese/08-relais-esp32.md §3,
//              docs/powl/03-network-protocol.md §6.
// ---------------------------------------------------------------------------
#include "esp_log.h"
#include "nvs_flash.h"
#include "sdkconfig.h"

#include "dengon_peer_id.h"
#include "dengon_transport.h"
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

    ESP_ERROR_CHECK(dengon_peer_id_init());
    dengon_peer_id_get(cfg.local_peer_id);

    tr = dengon_transport_start(&cfg);
    if (tr != DENGON_TR_OK) {
        ESP_LOGE(TAG, "démarrage du transport impossible : %s", dengon_tr_err_str(tr));
        return;
    }

#if CONFIG_DENGON_TRANSPORT_DEMO
    dengon_demo_start();
#endif
}
