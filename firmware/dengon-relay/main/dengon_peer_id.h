// ---------------------------------------------------------------------------
// Identité courte du nœud, utilisée dans le manufacturer data de l'annonce
// BLE et dans le nom de l'appareil.
//
// ⚠ BOUCHON (US-114). La conception veut peerID = SHA-256(pub_static)[0..8]
//   (docs/powl/03-network-protocol.md §3). `pub_static` n'existe pas encore :
//   la génération de l'identité Ed25519/X25519 et son stockage en NVS chiffrée
//   sont l'US-307. En attendant, on hache l'adresse MAC d'usine gravée en
//   eFuse, qui a les deux seules propriétés dont le squelette a besoin :
//   stable d'un redémarrage à l'autre, et distincte d'une carte à l'autre.
//
//   Ce n'est PAS une identité cryptographique : une MAC est publique et
//   prédictible. Rien ne doit s'appuyer là-dessus pour authentifier quoi que
//   ce soit. Écart consigné dans docs/suivi/03-ecarts-conception.md.
// ---------------------------------------------------------------------------
#pragma once

#include <stdint.h>

#include "esp_err.h"

/** Longueur du peerID, fixée par docs/powl/03-network-protocol.md §3. */
#define DENGON_PEER_ID_LEN 8

/**
 * Calcule le peerID et le met en cache. À appeler une fois, tôt dans app_main().
 *
 * @return ESP_OK, ou l'erreur remontée par la lecture de l'eFuse / le hachage.
 */
esp_err_t dengon_peer_id_init(void);

/**
 * Recopie le peerID mis en cache. Renvoie des zéros si dengon_peer_id_init()
 * n'a pas encore réussi.
 */
void dengon_peer_id_get(uint8_t out[DENGON_PEER_ID_LEN]);
