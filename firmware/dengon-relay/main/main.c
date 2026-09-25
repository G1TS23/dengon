// ---------------------------------------------------------------------------
// firmware/dengon-relay/main/main.c — squelette du relais dengon (US-114).
//
// Périmètre volontairement étroit : rôle PERIPHERAL seulement (annonce BLE +
// table GATT déclarée). Ce qui n'est PAS ici, et où ça ira :
//   - scan, rôle central, relais de paquets ......... US-220
//   - libdengon_core.a en FFI, vraie identité ....... US-307
//   - Wi-Fi, HTTPS, journal chaîné .................. US-309 et suivantes
// Les rôles CENTRAL et OBSERVER sont d'ailleurs coupés dans sdkconfig.defaults :
// ce firmware est techniquement incapable de scanner.
//
// SÉQUENCE D'INITIALISATION IMPOSÉE PAR ESP-IDF, dans cet ordre exact :
//   nvs_flash_init()             le contrôleur BT y range sa calibration PHY
//   nimble_port_init()           contrôleur + host, mais le host NE TOURNE PAS
//   ble_hs_cfg.*                 les callbacks, AVANT le démarrage de la tâche
//   dengon_gatt_init()           la table doit exister avant le premier client
//   nimble_port_freertos_init()  c'est CELA qui lance réellement le host
// Puis, seulement une fois le contrôleur synchronisé (sync_cb), on annonce :
// avant sync, le contrôleur n'a pas d'adresse et ble_gap_adv_start() renvoie
// BLE_HS_EAGAIN.
//
// Références : docs/synthese/08-relais-esp32.md §3,
//              docs/powl/03-network-protocol.md §6.
// ---------------------------------------------------------------------------
#include <stdio.h>
#include <string.h>

#include "esp_log.h"
#include "nvs_flash.h"

#include "host/ble_att.h"
#include "host/ble_hs.h"
#include "host/util/util.h"
#include "nimble/nimble_port.h"
#include "nimble/nimble_port_freertos.h"
#include "services/gap/ble_svc_gap.h"

#include "dengon_gatt.h"
#include "dengon_peer_id.h"

static const char *TAG = "dengon-relay";

/* docs/powl/03 §6.3 : négocier ATT_MTU = 517. Le repli sur 23 est AUTOMATIQUE
   (l'échange ATT retient le minimum des deux côtés) : rien à coder pour ça. */
#define DENGON_ATT_PREFERRED_MTU 517

/* Manufacturer data (7 octets) = Company ID (2) ‖ peerID[0..4] (4) ‖ flags (1).
   docs/powl/03 §6.1 ne décrit que les 5 derniers, mais le champ AD 0xFF du Core
   Bluetooth EXIGE un Company ID en tête, et NimBLE n'en ajoute aucun : c'est à
   nous de le poser. 0xFFFF est l'identifiant que le Bluetooth SIG réserve aux
   tests et à l'usage interne — le seul choix légitime tant que dengon n'a pas
   d'identifiant attribué. Écart consigné dans 03-ecarts-conception.md. */
#define DENGON_ADV_COMPANY_ID 0xFFFFu

/* Bitfield de l'octet `flags` de l'annonce. À ne pas confondre avec les `flags`
   du paquet de couche 3 (docs/powl/03 §3.1), qui n'ont aucun rapport. Défini
   ici faute de spécification — écart consigné. */
#define DENGON_ADV_F_RELAY        0x01u /* nœud d'infrastructure fixe          */
#define DENGON_ADV_F_COURIER      0x02u /* porte des enveloppes en dépôt       */
#define DENGON_ADV_F_ACCEPTS_CONN 0x04u /* accepte les connexions GATT         */
#define DENGON_ADV_F_HAS_UPLINK   0x08u /* dispose d'un lien IP vers le VPS    */

static uint8_t s_own_addr_type;
static uint8_t s_mfg_data[7];
static char    s_device_name[sizeof("dengon-relay-ffff")];

/* Fourni par le portage ESP32 de NimBLE (components/bt/host/nimble/...). */
void ble_store_config_init(void);

static void dengon_advertise(void);

/* --- 1. Construction des charges utiles d'annonce ------------------------ */

static void
dengon_build_adv_payloads(void)
{
    uint8_t peer_id[DENGON_PEER_ID_LEN];

    dengon_peer_id_get(peer_id);

    /* Le Company ID part en little-endian sur l'air (Core Spec, Vol 3). */
    s_mfg_data[0] = (uint8_t)(DENGON_ADV_COMPANY_ID & 0xFFu);
    s_mfg_data[1] = (uint8_t)(DENGON_ADV_COMPANY_ID >> 8);
    memcpy(&s_mfg_data[2], peer_id, 4); /* peerID[0..4], docs/powl/03 §6.1 */
    s_mfg_data[6] = DENGON_ADV_F_RELAY | DENGON_ADV_F_ACCEPTS_CONN;

    /* Le suffixe distingue deux cartes posées côte à côte — indispensable dès
       l'US-220, et il rend la capture nRF Connect auto-explicative. */
    snprintf(s_device_name, sizeof(s_device_name), "dengon-relay-%02x%02x",
             peer_id[0], peer_id[1]);
}

/* --- 2. Événements GAP --------------------------------------------------- */

static int
dengon_gap_event(struct ble_gap_event *event, void *arg)
{
    (void)arg;

    switch (event->type) {
    case BLE_GAP_EVENT_CONNECT:
        ESP_LOGI(TAG, "connexion %s (status=%d, conn=%u)",
                 event->connect.status == 0 ? "établie" : "échouée",
                 event->connect.status, event->connect.conn_handle);
        if (event->connect.status != 0) {
            dengon_advertise(); /* échec : on se réannonce immédiatement */
        }
        return 0;

    case BLE_GAP_EVENT_DISCONNECT:
        /* ⚠ Un périphérique NimBLE ARRÊTE d'annoncer dès qu'il est connecté.
           Sans ce réarmement, la carte disparaît définitivement du scanner
           après la première connexion — et l'on croit à un plantage. */
        ESP_LOGI(TAG, "déconnexion (raison=%d) — réarmement de l'annonce",
                 event->disconnect.reason);
        dengon_advertise();
        return 0;

    case BLE_GAP_EVENT_ADV_COMPLETE:
        ESP_LOGI(TAG, "annonce terminée (raison=%d) — réarmement",
                 event->adv_complete.reason);
        dengon_advertise();
        return 0;

    case BLE_GAP_EVENT_MTU:
        /* Preuve, dans le moniteur, du MTU exigé par docs/powl/03 §6.3. */
        ESP_LOGI(TAG, "ATT MTU négocié = %u (souhaité %d, plancher 23)",
                 event->mtu.value, DENGON_ATT_PREFERRED_MTU);
        return 0;

    case BLE_GAP_EVENT_SUBSCRIBE:
        ESP_LOGI(TAG, "abonnement : handle=%u notify=%d (CHAR_TX val=%u)",
                 event->subscribe.attr_handle, event->subscribe.cur_notify,
                 dengon_gatt_tx_val_handle());
        return 0;

    default:
        return 0;
    }
}

/* --- 3. Annonce ---------------------------------------------------------- */

static void
dengon_advertise(void)
{
    struct ble_hs_adv_fields adv = { 0 };
    struct ble_hs_adv_fields rsp = { 0 };
    struct ble_gap_adv_params params = { 0 };
    int rc;

    /* Paquet principal : 3 + 18 + 9 = 30 octets sur les 31 disponibles.
       ⚠ LE BUDGET EST SATURÉ. Y ajouter le nom (2 + n octets) ou le
       tx_pwr_lvl de l'exemple bleprph (3 octets) fait renvoyer BLE_HS_EMSGSIZE
       par ble_gap_adv_set_fields — à l'EXÉCUTION, pas à la compilation. D'où le
       nom relégué en réponse de scan. L'UUID de service, lui, doit rester dans
       le paquet principal : c'est sur lui que les pairs filtrent leur scan. */
    adv.flags = BLE_HS_ADV_F_DISC_GEN | BLE_HS_ADV_F_BREDR_UNSUP; /*  3 o */
    adv.uuids128 = &dengon_svc_uuid;                              /* 18 o */
    adv.num_uuids128 = 1;
    adv.uuids128_is_complete = 1;
    adv.mfg_data = s_mfg_data;                                    /*  9 o */
    adv.mfg_data_len = (uint8_t)sizeof(s_mfg_data);

    rc = ble_gap_adv_set_fields(&adv);
    if (rc != 0) {
        ESP_LOGE(TAG, "ble_gap_adv_set_fields a échoué : rc=%d "
                      "(0x0C = BLE_HS_EMSGSIZE, le paquet dépasse 31 octets)", rc);
        return;
    }

    /* Réponse de scan : nom (2 + 17 = 19 o) + puissance d'émission (3 o).
       nRF Connect émet un SCAN_REQ sur une annonce ADV_IND : le nom s'affiche
       donc quand même, sans rien coûter au paquet principal. */
    rsp.name = (const uint8_t *)s_device_name;
    rsp.name_len = (uint8_t)strlen(s_device_name);
    rsp.name_is_complete = 1;
    rsp.tx_pwr_lvl_is_present = 1;
    rsp.tx_pwr_lvl = BLE_HS_ADV_TX_PWR_LVL_AUTO; /* la pile remplit la valeur */

    rc = ble_gap_adv_rsp_set_fields(&rsp);
    if (rc != 0) {
        ESP_LOGE(TAG, "ble_gap_adv_rsp_set_fields a échoué : rc=%d", rc);
        return;
    }

    /* Connectable et découvrable en général : nRF Connect doit pouvoir se
       connecter pour LIRE la table GATT. 100-150 ms : le relais est sur
       secteur, on privilégie la vitesse de découverte.
       ⚠ Ne pas confondre avec ANNOUNCE_ISOLATED_S / ANNOUNCE_CONNECTED_S de
       docs/powl/03 §2, qui cadencent un PAQUET de couche 3, pas la radio. */
    params.conn_mode = BLE_GAP_CONN_MODE_UND;
    params.disc_mode = BLE_GAP_DISC_MODE_GEN;
    params.itvl_min = BLE_GAP_ADV_FAST_INTERVAL2_MIN;
    params.itvl_max = BLE_GAP_ADV_FAST_INTERVAL2_MAX;

    rc = ble_gap_adv_start(s_own_addr_type, NULL, BLE_HS_FOREVER, &params,
                           dengon_gap_event, NULL);
    if (rc != 0) {
        ESP_LOGE(TAG, "ble_gap_adv_start a échoué : rc=%d", rc);
        return;
    }

    ESP_LOGI(TAG, "annonce en cours sous le nom « %s »", s_device_name);
}

/* --- 4. Callbacks du host ------------------------------------------------ */

static void
dengon_on_reset(int reason)
{
    ESP_LOGE(TAG, "pile BLE réinitialisée par le contrôleur (raison=%d)", reason);
}

static void
dengon_on_sync(void)
{
    int rc;

    /* S'assurer qu'une adresse identité existe, puis choisir son type. Sans
       cela, ble_gap_adv_start() échoue : le contrôleur n'a pas d'adresse. */
    rc = ble_hs_util_ensure_addr(0);
    if (rc != 0) {
        ESP_LOGE(TAG, "ble_hs_util_ensure_addr a échoué : rc=%d", rc);
        return;
    }

    rc = ble_hs_id_infer_auto(0, &s_own_addr_type);
    if (rc != 0) {
        ESP_LOGE(TAG, "ble_hs_id_infer_auto a échoué : rc=%d", rc);
        return;
    }

    dengon_advertise();
}

static void
dengon_host_task(void *param)
{
    (void)param;

    ESP_LOGI(TAG, "tâche host NimBLE démarrée");
    nimble_port_run(); /* ne rend la main qu'à nimble_port_stop() */
    nimble_port_freertos_deinit();
}

/* --- 5. Point d'entrée --------------------------------------------------- */

void
app_main(void)
{
    esp_err_t err;
    int rc;

    /* NVS d'abord : le contrôleur BT y stocke sa calibration PHY. Sans cela,
       nimble_port_init() échoue à l'exécution. Le cycle erase/retry couvre le
       cas d'une partition NVS héritée d'un firmware précédent. */
    err = nvs_flash_init();
    if (err == ESP_ERR_NVS_NO_FREE_PAGES || err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_ERROR_CHECK(nvs_flash_erase());
        err = nvs_flash_init();
    }
    ESP_ERROR_CHECK(err);

    ESP_ERROR_CHECK(dengon_peer_id_init());
    dengon_build_adv_payloads();

    ESP_ERROR_CHECK(nimble_port_init());

    /* Les callbacks doivent être posés AVANT nimble_port_freertos_init(). */
    ble_hs_cfg.reset_cb = dengon_on_reset;
    ble_hs_cfg.sync_cb = dengon_on_sync;
    ble_hs_cfg.gatts_register_cb = dengon_gatt_register_cb;
    ble_hs_cfg.store_status_cb = ble_store_util_status_rr;

    rc = dengon_gatt_init();
    if (rc != 0) {
        ESP_LOGE(TAG, "dengon_gatt_init a échoué : rc=%d", rc);
        return;
    }

    rc = ble_svc_gap_device_name_set(s_device_name);
    if (rc != 0) {
        ESP_LOGE(TAG, "ble_svc_gap_device_name_set a échoué : rc=%d", rc);
        return;
    }

    /* Doublon volontaire de CONFIG_BT_NIMBLE_ATT_PREFERRED_MTU : la valeur
       Kconfig fixe le défaut à la compilation, cet appel la rend explicite dans
       le code et survivra à un sdkconfig régénéré à la main. 517 <= 527
       (BLE_ATT_MTU_MAX), sinon l'appel renverrait BLE_HS_EINVAL. */
    rc = ble_att_set_preferred_mtu(DENGON_ATT_PREFERRED_MTU);
    if (rc != 0) {
        ESP_LOGE(TAG, "ble_att_set_preferred_mtu a échoué : rc=%d", rc);
        return;
    }

    ble_store_config_init();

    /* C'est seulement ici que le host démarre. L'oublier donne un firmware qui
       boote, n'affiche aucune erreur, et n'annonce rien. */
    nimble_port_freertos_init(dengon_host_task);
}
