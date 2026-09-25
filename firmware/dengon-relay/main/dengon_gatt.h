// ---------------------------------------------------------------------------
// Table GATT du service `dengon`.
//
// Source de vérité des UUID : docs/powl/03-network-protocol.md §2 (décision
// C-2 de docs/synthese/01-sujets-a-trancher.md). Ne jamais les redéfinir
// ailleurs : l'application Android et dengon-core doivent lire EXACTEMENT les
// mêmes valeurs, sans quoi les deux moitiés du maillage ne se voient pas.
// ---------------------------------------------------------------------------
#pragma once

#include <stdint.h>

#include "host/ble_hs.h"
#include "host/ble_uuid.h"

/** UUID du service `dengon` — exposé parce que l'annonce doit le publier. */
extern const ble_uuid128_t dengon_svc_uuid;

/**
 * Enregistre le service auprès du host NimBLE.
 * À appeler APRÈS nimble_port_init() et AVANT nimble_port_freertos_init().
 *
 * @return 0, ou un code d'erreur BLE_HS_E*.
 */
int dengon_gatt_init(void);

/**
 * Callback de journalisation des enregistrements, à brancher sur
 * ble_hs_cfg.gatts_register_cb. Il imprime au démarrage les UUID tels que
 * NimBLE les a réellement enregistrés : c'est la preuve textuelle du critère
 * d'acceptation « advertise le service dengon avec son UUID », et le moyen le
 * plus rapide de détecter une erreur d'endianness (voir dengon_gatt.c).
 */
void dengon_gatt_register_cb(struct ble_gatt_register_ctxt *ctxt, void *arg);

/**
 * Handle de valeur de CHAR_TX, nécessaire à ble_gatts_notify_custom().
 * Vaut 0 tant que dengon_gatt_init() n'a pas tourné. Inutilisé par l'US-114,
 * indispensable à l'US-220 : sans lui, le nœud ne pourra jamais émettre.
 */
uint16_t dengon_gatt_tx_val_handle(void);
