// ---------------------------------------------------------------------------
// dengon_wifi.c — voir dengon_wifi.h (US-309).
// ---------------------------------------------------------------------------
#include "dengon_wifi.h"

#include <inttypes.h>
#include <stdio.h>
#include <string.h>

#include "esp_event.h"
#include "esp_log.h"
#include "esp_netif.h"
#include "esp_netif_sntp.h"
#include "esp_random.h"
#include "esp_timer.h"
#include "esp_wifi.h"
#include "freertos/event_groups.h"
#include "nvs.h"
#include "sdkconfig.h"

#include "dengon_relay_app.h"
#include "dengon_ship_policy.h"

static const char *TAG = "dengon-wifi";

#define NVS_NS       "dengon_net"
#define NVS_SSID     "ssid"
#define NVS_PASSWORD "pass"
#define BIT_UP       BIT0
#define PASSWORD_MAX 64

static EventGroupHandle_t s_events;
static esp_timer_handle_t s_retry_timer;
static char               s_ssid[DENGON_WIFI_SSID_MAX + 1];
static char               s_password[PASSWORD_MAX + 1];
static unsigned           s_failures;
static bool               s_was_up;
static bool               s_sntp_started;
/* Posé par la console (nouveaux identifiants), consommé par le gestionnaire
   d'événements : seul ce dernier écrit `s_failures` et `s_was_up`. */
static volatile bool      s_reset_backoff;
/* Instant (µs, horloge monotone) du dernier changement d'état, pour le
   `duration_s` des événements. */
static int64_t            s_since_us;

static uint64_t
duree_s(void)
{
    int64_t now = esp_timer_get_time();
    uint64_t d  = (uint64_t)(now - s_since_us) / 1000000u;

    s_since_us = now;
    return d;
}

static void
journaliser(const char *name)
{
    /* Le SSID n'est journalisé que s'il est en ASCII imprimable, sans `"` ni
       `\` : un SSID peut légalement contenir des octets de contrôle ou non
       UTF-8, qui donneraient un JSON invalide, refusé ensuite par dengon-core
       (revue PR #120). Le champ est facultatif au contrat. */
    char payload[96];
    bool ssid_sur = true;

    for (const char *c = s_ssid; *c != '\0'; c++) {
        if (*c < 0x20 || *c > 0x7e || *c == '"' || *c == '\\') {
            ssid_sur = false;
            break;
        }
    }

    if (ssid_sur) {
        snprintf(payload, sizeof(payload), "{\"duration_s\":%" PRIu64 ",\"ssid\":\"%s\"}",
                 duree_s(), s_ssid);
    } else {
        snprintf(payload, sizeof(payload), "{\"duration_s\":%" PRIu64 "}", duree_s());
    }
    dengon_relay_app_record(name, payload);
}

static void
connecter(void *arg)
{
    (void)arg;
    if (s_ssid[0] != '\0') {
        esp_wifi_connect();
    }
}

static void
sur_evenement(void *arg, esp_event_base_t base, int32_t id, void *data)
{
    (void)arg;
    if (base == WIFI_EVENT && id == WIFI_EVENT_STA_START) {
        connecter(NULL);
    } else if (base == WIFI_EVENT && id == WIFI_EVENT_STA_DISCONNECTED) {
        const wifi_event_sta_disconnected_t *d = data;
        uint32_t                             delai;

        xEventGroupClearBits(s_events, BIT_UP);
        if (s_reset_backoff) {
            s_reset_backoff = false;
            s_failures      = 0;
        }
        if (s_was_up) {
            s_was_up = false;
            ESP_LOGW(TAG, "Wi-Fi perdu (raison %u)", d->reason);
            journaliser("relay.wifi_down");
        }
        s_failures++;
        delai = dengon_ship_backoff_ms(s_failures, esp_random());
        ESP_LOGI(TAG, "reconnexion à « %s » dans %" PRIu32 " ms (raison %u)", s_ssid, delai,
                 d->reason);
        esp_timer_stop(s_retry_timer);
        esp_timer_start_once(s_retry_timer, (uint64_t)delai * 1000u);
    } else if (base == IP_EVENT && id == IP_EVENT_STA_GOT_IP) {
        const ip_event_got_ip_t *ip = data;

        ESP_LOGI(TAG, "connecté à « %s », IP " IPSTR, s_ssid, IP2STR(&ip->ip_info.ip));
        s_failures = 0;
        s_was_up   = true;
        journaliser("relay.wifi_up");
        if (!s_sntp_started) {
            esp_sntp_config_t cfg = ESP_NETIF_SNTP_DEFAULT_CONFIG(CONFIG_DENGON_SNTP_SERVER);

            s_sntp_started = esp_netif_sntp_init(&cfg) == ESP_OK;
        }
        xEventGroupSetBits(s_events, BIT_UP);
    }
}

static void
lire_nvs(void)
{
    nvs_handle_t h;
    size_t       len;

    strlcpy(s_ssid, CONFIG_DENGON_WIFI_SSID, sizeof(s_ssid));
    strlcpy(s_password, CONFIG_DENGON_WIFI_PASSWORD, sizeof(s_password));
    if (nvs_open(NVS_NS, NVS_READONLY, &h) != ESP_OK) {
        return;
    }
    len = sizeof(s_ssid);
    if (nvs_get_str(h, NVS_SSID, s_ssid, &len) == ESP_OK) {
        len = sizeof(s_password);
        if (nvs_get_str(h, NVS_PASSWORD, s_password, &len) != ESP_OK) {
            s_password[0] = '\0';
        }
    }
    nvs_close(h);
}

static esp_err_t
appliquer(void)
{
    wifi_config_t cfg = { 0 };

    strlcpy((char *)cfg.sta.ssid, s_ssid, sizeof(cfg.sta.ssid));
    strlcpy((char *)cfg.sta.password, s_password, sizeof(cfg.sta.password));
    /* Réseau ouvert accepté si aucun mot de passe n'est donné. */
    cfg.sta.threshold.authmode = s_password[0] != '\0' ? WIFI_AUTH_WPA2_PSK : WIFI_AUTH_OPEN;
    return esp_wifi_set_config(WIFI_IF_STA, &cfg);
}

esp_err_t
dengon_wifi_start(void)
{
    wifi_init_config_t          init  = WIFI_INIT_CONFIG_DEFAULT();
    const esp_timer_create_args_t timer = { .callback = connecter, .name = "wifi_retry" };
    esp_err_t                   err;

    s_events   = xEventGroupCreate();
    s_since_us = esp_timer_get_time();
    if (s_events == NULL) {
        return ESP_ERR_NO_MEM;
    }
    lire_nvs();

    err = esp_netif_init();
    if (err == ESP_OK) {
        err = esp_event_loop_create_default();
        if (err == ESP_ERR_INVALID_STATE) { /* déjà créée */
            err = ESP_OK;
        }
    }
    if (err == ESP_OK) {
        err = esp_netif_create_default_wifi_sta() != NULL ? ESP_OK : ESP_FAIL;
    }
    if (err == ESP_OK) {
        err = esp_wifi_init(&init);
    }
    if (err == ESP_OK) {
        err = esp_timer_create(&timer, &s_retry_timer);
    }
    if (err == ESP_OK) {
        err = esp_event_handler_register(WIFI_EVENT, ESP_EVENT_ANY_ID, sur_evenement, NULL);
    }
    if (err == ESP_OK) {
        err = esp_event_handler_register(IP_EVENT, IP_EVENT_STA_GOT_IP, sur_evenement, NULL);
    }
    if (err == ESP_OK) {
        /* Identifiants en NVS nous-mêmes : pas de copie par le pilote. */
        err = esp_wifi_set_storage(WIFI_STORAGE_RAM);
    }
    if (err == ESP_OK) {
        err = esp_wifi_set_mode(WIFI_MODE_STA);
    }
    if (err == ESP_OK) {
        err = appliquer();
    }
    if (err == ESP_OK) {
        err = esp_wifi_start();
    }
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Wi-Fi indisponible : %s", esp_err_to_name(err));
        return err;
    }
    if (s_ssid[0] == '\0') {
        ESP_LOGW(TAG, "aucun réseau configuré : commande console `wifi <ssid> <mdp>`");
    }
    return ESP_OK;
}

esp_err_t
dengon_wifi_set_credentials(const char *ssid, const char *password)
{
    nvs_handle_t h;
    esp_err_t    err;

    if (ssid == NULL || ssid[0] == '\0' || strlen(ssid) > DENGON_WIFI_SSID_MAX
        || strlen(password) > PASSWORD_MAX) {
        return ESP_ERR_INVALID_ARG;
    }
    err = nvs_open(NVS_NS, NVS_READWRITE, &h);
    if (err != ESP_OK) {
        return err;
    }
    err = nvs_set_str(h, NVS_SSID, ssid);
    if (err == ESP_OK) {
        err = nvs_set_str(h, NVS_PASSWORD, password);
    }
    if (err == ESP_OK) {
        err = nvs_commit(h);
    }
    nvs_close(h);
    if (err != ESP_OK) {
        return err;
    }
    strlcpy(s_ssid, ssid, sizeof(s_ssid));
    strlcpy(s_password, password, sizeof(s_password));
    esp_timer_stop(s_retry_timer);
    err = appliquer();
    if (err != ESP_OK) {
        return err;
    }
    /* Une seule voie de (re)connexion, le minuteur : connecté, on coupe et le
       gestionnaire STA_DISCONNECTED réarme le minuteur ; sinon on l'arme
       nous-mêmes. Appeler aussi esp_wifi_connect() ici lancerait deux
       tentatives concurrentes (revue PR #120). Remettre `s_failures` à zéro
       se fait dans le gestionnaire, seul à l'écrire. */
    s_reset_backoff = true;
    if (dengon_wifi_is_up()) {
        return esp_wifi_disconnect();
    }
    return esp_timer_start_once(s_retry_timer, 1000);
}

bool
dengon_wifi_wait_up(TickType_t timeout)
{
    return (xEventGroupWaitBits(s_events, BIT_UP, pdFALSE, pdTRUE, timeout) & BIT_UP) != 0;
}

bool
dengon_wifi_is_up(void)
{
    return s_events != NULL && (xEventGroupGetBits(s_events) & BIT_UP) != 0;
}

int
dengon_wifi_rssi(void)
{
    int rssi = -120;

    if (!dengon_wifi_is_up() || esp_wifi_sta_get_rssi(&rssi) != ESP_OK) {
        return -120;
    }
    return rssi;
}

const char *
dengon_wifi_ssid(void)
{
    return s_ssid;
}
