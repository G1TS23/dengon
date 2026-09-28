// ---------------------------------------------------------------------------
// dengon_transport — l'implémentation NimBLE du contrat `Transport` (US-220).
//
// Miroir C, méthode pour méthode, du trait Rust d'US-105
// (crates/dengon-ble/src/transport.rs) : start / poll / send / broadcast, avec
// les mêmes événements, les mêmes erreurs et les mêmes 5 règles de
// déconnexion brutale. C'est la surface que l'adaptateur Rust d'US-307
// enveloppera pour faire tourner la suite de conformité contre la radio.
//
// Le transport ne comprend RIEN à ce qu'il transporte : octets opaques, 1 trame
// = 1 PDU ATT (au plus ATT_MTU - 3 octets). La fragmentation protocole est
// faite en amont par dengon-core (protocol::fragment, US-202).
//
// Thread-safety : toutes les fonctions sont appelables depuis n'importe quelle
// tâche ; les callbacks NimBLE tournent dans la tâche host.
// ---------------------------------------------------------------------------
#pragma once

#include <stddef.h>
#include <stdint.h>

#include "host/ble_hs.h"

#include "dengon_transport_core.h"

/**
 * Démarre la pile NimBLE, puis l'annonce (cfg->advertise) et le scan
 * (cfg->scan) dès que le contrôleur est synchronisé.
 *
 * Pré-requis : nvs_flash_init() déjà fait (calibration PHY du contrôleur).
 * cfg->max_connections est borné à CONFIG_BT_NIMBLE_MAX_CONNECTIONS.
 *
 * @return OK ; ALREADY_STARTED ; BACKEND si NimBLE refuse de démarrer.
 */
dengon_tr_err_t dengon_transport_start(const dengon_transport_config_t *cfg);

/**
 * Retire jusqu'à `cap` événements. Ne bloque jamais, 0 = rien de neuf.
 * Chaque événement rendu doit être passé à dengon_transport_event_free().
 */
size_t dengon_transport_poll(dengon_transport_event_t *out, size_t cap);

/** Libère le tampon d'un FRAME_RECEIVED. */
void dengon_transport_event_free(dengon_transport_event_t *ev);

/**
 * Envoie une trame à un pair. OK = remis à la pile BLE, PAS reçu par le pair.
 *
 * @param max_out renseigné (si non NULL) quand la trame est trop grande.
 * @return OK ; NOT_STARTED ; UNKNOWN_PEER ; FRAME_TOO_LARGE ; BACKEND.
 */
dengon_tr_err_t dengon_transport_send(dengon_link_id_t link, const uint8_t *bytes, size_t len,
                                      size_t *max_out);

/**
 * Diffuse à tous les pairs connectés, au mieux. OK même sans aucun pair.
 *
 * @return OK ; NOT_STARTED ; FRAME_TOO_LARGE (> 514 octets).
 */
dengon_tr_err_t dengon_transport_broadcast(const uint8_t *bytes, size_t len);

/**
 * Ferme proprement tous les liens (motif Locale de notre côté, Propre en face).
 * Sert à la démo et, plus tard, à l'arrêt du relais.
 */
void dengon_transport_disconnect_all(void);

/**
 * Point d'entrée des écritures sur CHAR_RX (rôle périphérique), appelé par
 * dengon_gatt.c depuis la tâche host. Interne au firmware.
 */
void dengon_transport_on_rx(uint16_t conn_handle, const struct os_mbuf *om);
