// ---------------------------------------------------------------------------
// Bouchon de peerID — voir l'avertissement en tête de dengon_peer_id.h.
// ---------------------------------------------------------------------------
#include <string.h>

#include "esp_log.h"
#include "esp_mac.h"
#include "mbedtls/sha256.h"

#include "dengon_peer_id.h"

static const char *TAG = "dengon-peer";

static uint8_t s_peer_id[DENGON_PEER_ID_LEN];

esp_err_t
dengon_peer_id_init(void)
{
    uint8_t mac[6];
    uint8_t digest[32];
    esp_err_t err;

    err = esp_efuse_mac_get_default(mac);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "lecture de la MAC eFuse impossible : %s", esp_err_to_name(err));
        return err;
    }

    // mbedtls 3.x : mbedtls_sha256(entrée, taille, sortie[32], is224).
    // Le dernier argument à 0 demande bien SHA-256, pas SHA-224.
    if (mbedtls_sha256(mac, sizeof(mac), digest, 0) != 0) {
        ESP_LOGE(TAG, "mbedtls_sha256 a échoué");
        return ESP_FAIL;
    }

    memcpy(s_peer_id, digest, DENGON_PEER_ID_LEN);

    ESP_LOGI(TAG, "peerID (bouchon, SHA-256 de la MAC) = %02x%02x%02x%02x%02x%02x%02x%02x",
             s_peer_id[0], s_peer_id[1], s_peer_id[2], s_peer_id[3],
             s_peer_id[4], s_peer_id[5], s_peer_id[6], s_peer_id[7]);

    return ESP_OK;
}

void
dengon_peer_id_get(uint8_t out[DENGON_PEER_ID_LEN])
{
    memcpy(out, s_peer_id, DENGON_PEER_ID_LEN);
}
