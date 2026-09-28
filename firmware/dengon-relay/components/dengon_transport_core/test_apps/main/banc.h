// ---------------------------------------------------------------------------
// Banc d'essai — miroir C du trait `BancDEssai` de
// crates/dengon-ble/src/conformance.rs.
//
// Le « transport » testé est dengon_transport_core, piloté exactement comme le
// pilote la glue NimBLE (main/transport_nimble.c) : link_open à la connexion
// GAP, set_mtu à l'échange MTU, announce à l'abonnement, on_rx à la réception,
// link_close avec le code HCI à la déconnexion. La radio est remplacée par un
// compteur d'envois : send/broadcast valident par le cœur, puis « émettent ».
// ---------------------------------------------------------------------------
#pragma once

#include <stddef.h>
#include <stdint.h>

#include "dengon_transport_core.h"

typedef struct {
    dengon_tc_t tc;
    /* Correspondance LinkId -> conn_handle, CONSERVÉE après fermeture : le
       banc doit pouvoir « faire recevoir » sur un lien mort (cas fantôme). */
    dengon_link_id_t ids[64];
    uint16_t         conns[64];
    size_t           n_ids;
    /* Radio simulée. */
    size_t radio_sent;
} banc_t;

/** Événements rendus par un poll, possédés par le banc jusqu'au prochain. */
typedef struct {
    dengon_transport_event_t ev[DENGON_TC_EVQ_CAP];
    size_t                   n;
} lot_t;

/** BancDEssai::config — défaut Rust : double rôle, 8 liens, MTU 517. */
dengon_transport_config_t banc_config(void);

/** BancDEssai::nouveau — transport neuf, non démarré. */
void banc_nouveau(banc_t *b);

/** Transport neuf ET démarré (helper `demarre` de la suite Rust). */
void banc_demarre(banc_t *b);

/** BancDEssai::connecter_un_pair — conn_handle le plus bas libre, MTU 517. */
dengon_link_id_t banc_connecter_un_pair(banc_t *b);

/** BancDEssai::couper — traduit le motif en code HCI, comme la vraie pile. */
void banc_couper(banc_t *b, dengon_link_id_t lien, dengon_disconnect_reason_t motif);

/** BancDEssai::faire_recevoir. */
void banc_faire_recevoir(banc_t *b, dengon_link_id_t lien, const uint8_t *bytes, size_t len);

/** Transport::send / broadcast, radio simulée. */
dengon_tr_err_t banc_send(banc_t *b, dengon_link_id_t lien, const uint8_t *bytes, size_t len);
dengon_tr_err_t banc_broadcast(banc_t *b, const uint8_t *bytes, size_t len);

/** Transport::poll — libère le lot précédent avant de le remplir. */
void banc_poll(banc_t *b, lot_t *lot);

/** Libère les tampons d'un lot. */
void lot_liberer(lot_t *lot);

/** conn_handle d'un lien connu du banc. */
uint16_t banc_conn_de(const banc_t *b, dengon_link_id_t lien);
