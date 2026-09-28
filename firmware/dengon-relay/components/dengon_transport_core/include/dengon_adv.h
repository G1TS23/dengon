// ---------------------------------------------------------------------------
// dengon_adv — format du manufacturer data de l'annonce, et règle anti-boucle
// de connexion (US-220). C pur, testable sur hôte.
//
// Manufacturer data (7 octets) = Company ID (2, LE) ‖ peerID[0..4] (4) ‖ flags (1).
// docs/powl/03 §6.1 n'en décrit que les 5 derniers ; le Company ID est exigé
// par le champ AD 0xFF du Core Bluetooth (écart consigné en US-114). Tout
// décodeur doit donc SAUTER les deux premiers octets.
// ---------------------------------------------------------------------------
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** 0xFFFF : identifiant réservé par le Bluetooth SIG aux tests / usage interne. */
#define DENGON_ADV_COMPANY_ID 0xFFFFu

#define DENGON_ADV_MFG_LEN         7
#define DENGON_ADV_PEER_PREFIX_LEN 4

/* Bitfield de l'octet `flags` de l'annonce — sans rapport avec les `flags` du
   paquet de couche 3 (docs/powl/03 §3.1). Défini en US-114 faute de spec. */
#define DENGON_ADV_F_RELAY        0x01u /* nœud d'infrastructure fixe          */
#define DENGON_ADV_F_COURIER      0x02u /* porte des enveloppes en dépôt       */
#define DENGON_ADV_F_ACCEPTS_CONN 0x04u /* accepte les connexions GATT         */
#define DENGON_ADV_F_HAS_UPLINK   0x08u /* dispose d'un lien IP vers le VPS    */

/** Ce qu'on lit dans l'annonce d'un pair. */
typedef struct {
    uint8_t peer_prefix[DENGON_ADV_PEER_PREFIX_LEN];
    uint8_t flags;
} dengon_adv_info_t;

/** Fabrique les 7 octets de manufacturer data. */
void dengon_adv_build_mfg(const uint8_t local_peer_id[8], uint8_t flags,
                          uint8_t out[DENGON_ADV_MFG_LEN]);

/**
 * Décode le manufacturer data d'un pair.
 *
 * @return false si le champ est trop court ou ne porte pas notre Company ID —
 *         ce n'est alors pas un nœud dengon, même s'il annonce notre UUID.
 */
bool dengon_adv_parse_mfg(const uint8_t *data, size_t len, dengon_adv_info_t *out);

/**
 * Règle anti-boucle (docs/powl/03 §6.1) : quand deux nœuds se découvrent, seul
 * celui dont le peerID est le plus PETIT initie la connexion GATT.
 *
 * On ne voit que les 4 premiers octets du peerID distant : la comparaison se
 * fait sur ces 4 octets. En cas d'égalité (1 chance sur 2^32), on départage sur
 * les adresses BLE, comparées octet à octet dans la représentation NimBLE —
 * les deux nœuds font le même calcul, la décision reste antisymétrique.
 *
 * @return true si NOUS devons initier. false si c'est au pair, ou si préfixe
 *         ET adresse sont égaux (c'est notre propre annonce).
 */
bool dengon_adv_should_initiate(const uint8_t local_peer_id[8],
                                const uint8_t remote_prefix[DENGON_ADV_PEER_PREFIX_LEN],
                                const uint8_t local_addr[6], const uint8_t remote_addr[6]);

#ifdef __cplusplus
}
#endif
