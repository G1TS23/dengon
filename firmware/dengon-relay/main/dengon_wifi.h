// ---------------------------------------------------------------------------
// dengon_wifi — Wi-Fi station du relais et heure SNTP (US-309).
//
// Le Wi-Fi ne sert qu'à joindre le dashboard : le maillage reste en BLE, et
// le relais fonctionne à l'identique sans lui (docs/synthese/08 §6). Les
// deux radios partagent l'antenne (coexistence logicielle ESP-IDF).
//
// Identifiants : NVS (espace `dengon_net`, posés par la commande console
// `wifi <ssid> <mdp>`), à défaut CONFIG_DENGON_WIFI_SSID/PASSWORD. Sans
// identifiant, le Wi-Fi reste éteint jusqu'à la commande.
//
// Reconnexion automatique avec backoff (1 s → 60 s). Chaque montée / chute
// est journalisée (`relay.wifi_up` / `relay.wifi_down`, avec la durée de
// l'état précédent). À la première connexion, SNTP met l'horloge murale à
// l'heure : les événements suivants portent un `ts_ms` réel.
// ---------------------------------------------------------------------------
#pragma once

#include <stdbool.h>
#include <stdint.h>

#include "esp_err.h"
#include "freertos/FreeRTOS.h"

/** Longueur maximale d'un SSID (802.11). */
#define DENGON_WIFI_SSID_MAX 32

/** Initialise la pile Wi-Fi et se connecte si des identifiants existent. */
esp_err_t dengon_wifi_start(void);

/** Enregistre de nouveaux identifiants en NVS et (re)connecte. */
esp_err_t dengon_wifi_set_credentials(const char *ssid, const char *password);

/** Attend une adresse IP au plus `timeout` ticks. */
bool dengon_wifi_wait_up(TickType_t timeout);

/** Adresse IP obtenue et pas perdue depuis. */
bool dengon_wifi_is_up(void);

/** RSSI du point d'accès en dBm, ou -120 hors connexion. */
int dengon_wifi_rssi(void);

/** SSID configuré (chaîne vide si aucun). */
const char *dengon_wifi_ssid(void);
