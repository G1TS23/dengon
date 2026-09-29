// ---------------------------------------------------------------------------
// dengon_relay_app — la boucle du relais dengon (US-308).
//
// Quatre tâches FreeRTOS autour d'UN handle dengon-core (libdengon_core.a),
// protégé par un mutex (le handle n'est pas thread-safe) :
//
//   route      transport → dengon_relay_on_frame / link_up / link_down, puis
//              relais jitterés échus (cadence 10 ms) ;
//   inventory  pushs de réconciliation cadencés (1 s) ;
//   courier    expiration des enveloppes détenues + bilan de santé (30 s) ;
//   ledger     entrées de journal produites → littlefs (fsync) → curseur NVS.
//
// Chaque tâche vide ensuite la file de trames à émettre vers le transport,
// HORS du mutex (dengon_transport_send est lui-même thread-safe).
//
// File d'entrée bornée : celle du transport (DENGON_TC_EVQ_CAP) — une
// trame qui ne tient plus est jetée et comptée par le transport, jamais une
// enveloppe déjà acceptée (docs/synthese/08-relais-esp32.md §4).
// ---------------------------------------------------------------------------
#pragma once

#include <stdint.h>

#include "esp_err.h"

#include "dengon_core.h"

/**
 * Crée le relais (identité, reprise du journal) et journalise `relay.boot`.
 * À appeler APRÈS nvs_flash_init() et AVANT le démarrage de la radio : les
 * secrets d'un premier démarrage sont tirés sous bootloader_random_enable().
 *
 * @param[out] peer_id  peerID du relais, pour la configuration du transport
 */
esp_err_t dengon_relay_app_init(uint8_t peer_id[8]);

/**
 * Auto-test Noise sur l'aléa matériel (radio allumée), puis démarrage des
 * quatre tâches. À appeler après dengon_transport_start().
 */
esp_err_t dengon_relay_app_start(void);

/** Handle du relais et son mutex, pour la console (`relay`, `ledger`). */
DengonRelay *dengon_relay_app_lock(void);
void dengon_relay_app_unlock(void);
