// ---------------------------------------------------------------------------
// Table GATT du service `dengon` (US-114).
//
// ⚠ BLE_UUID128_INIT ATTEND LES OCTETS EN LITTLE-ENDIAN.
//
//   NimBLE stocke ble_uuid128_t.value[] à l'envers de la forme textuelle :
//   ble_uuid_to_str() réimprime value[15] puis value[14] … jusqu'à value[0].
//   Recopier la chaîne de gauche à droite compile, link et tourne — en
//   produisant un UUID INVERSÉ, que rien ne révèle avant le premier scan.
//
//   Contrôle de cohérence, à faire à l'œil : le DERNIER octet de chaque
//   tableau ci-dessous vaut 0x6d (le « m » de « meng »), et le PREMIER est le
//   discriminant (0x00 service, 0x01 RX, 0x02 TX). Les 14 octets du milieu
//   sont identiques pour les trois.
//
//   Les UUID ne sont pas aléatoires : ce sont 14 octets d'ASCII,
//   « meng-dengon-v1 », suivis de 2 octets de discriminant.
//
// Source de vérité : docs/powl/03-network-protocol.md §2 et §6.1 (décision C-2).
// ---------------------------------------------------------------------------
#include <stdint.h>

#include "esp_log.h"
#include "host/ble_hs.h"
#include "host/ble_uuid.h"
#include "os/os_mbuf.h"
#include "services/gap/ble_svc_gap.h"
#include "services/gatt/ble_svc_gatt.h"

#include "dengon_gatt.h"

static const char *TAG = "dengon-gatt";

/* 6d656e67-2d64-656e-676f-6e2d76310000  —  service `dengon` */
const ble_uuid128_t dengon_svc_uuid =
    BLE_UUID128_INIT(0x00, 0x00, 0x31, 0x76, 0x2d, 0x6e, 0x6f, 0x67,
                     0x6e, 0x65, 0x64, 0x2d, 0x67, 0x6e, 0x65, 0x6d);

/* 6d656e67-2d64-656e-676f-6e2d76310001  —  RX : pair -> nœud, write sans réponse */
static const ble_uuid128_t dengon_chr_rx_uuid =
    BLE_UUID128_INIT(0x01, 0x00, 0x31, 0x76, 0x2d, 0x6e, 0x6f, 0x67,
                     0x6e, 0x65, 0x64, 0x2d, 0x67, 0x6e, 0x65, 0x6d);

/* 6d656e67-2d64-656e-676f-6e2d76310002  —  TX : nœud -> pair, notification */
static const ble_uuid128_t dengon_chr_tx_uuid =
    BLE_UUID128_INIT(0x02, 0x00, 0x31, 0x76, 0x2d, 0x6e, 0x6f, 0x67,
                     0x6e, 0x65, 0x64, 0x2d, 0x67, 0x6e, 0x65, 0x6d);

/* Renseigné par NimBLE à l'enregistrement, via le .val_handle de la table. */
static uint16_t s_tx_val_handle;

static int dengon_chr_access(uint16_t conn_handle, uint16_t attr_handle,
                             struct ble_gatt_access_ctxt *ctxt, void *arg);

// Pas de troisième caractéristique « ACK » : docs/powl/03 §2 n'en prévoit pas.
// L'accusé de réception est un TYPE DE PAQUET de la couche 3, qui transite par
// RX/TX comme les autres (décision C-2, qui écarte explicitement la variante à
// trois caractéristiques de docs/oswin/07).
//
// Pas de descripteur CCCD (0x2902) déclaré à la main : NimBLE l'ajoute
// AUTOMATIQUEMENT derrière toute caractéristique portant BLE_GATT_CHR_F_NOTIFY.
// Le déclarer produirait un CCCD en double dans la table.
static const struct ble_gatt_svc_def dengon_gatt_svcs[] = {
    {
        .type = BLE_GATT_SVC_TYPE_PRIMARY,
        .uuid = &dengon_svc_uuid.u,
        .characteristics = (struct ble_gatt_chr_def[]) {
            {
                /* CHAR_RX — le pair y écrit ses paquets, sans réponse ATT :
                   docs/powl/03 §6.1 (économie d'un aller-retour par paquet). */
                .uuid      = &dengon_chr_rx_uuid.u,
                .access_cb = dengon_chr_access,
                .flags     = BLE_GATT_CHR_F_WRITE_NO_RSP,
            },
            {
                /* CHAR_TX — le nœud pousse ses paquets par notification.
                   .val_handle est OBLIGATOIRE : sans lui, l'US-220 n'aura
                   aucun handle à passer à ble_gatts_notify_custom() et ne
                   pourra jamais émettre. C'est NimBLE qui le renseigne. */
                .uuid       = &dengon_chr_tx_uuid.u,
                .access_cb  = dengon_chr_access,
                .flags      = BLE_GATT_CHR_F_NOTIFY,
                .val_handle = &s_tx_val_handle,
            },
            { 0 },   /* sentinelle de fin de liste — son oubli plante NimBLE */
        },
    },
    { 0 },           /* sentinelle de fin de services */
};

uint16_t
dengon_gatt_tx_val_handle(void)
{
    return s_tx_val_handle;
}

static int
dengon_chr_access(uint16_t conn_handle, uint16_t attr_handle,
                  struct ble_gatt_access_ctxt *ctxt, void *arg)
{
    (void)arg;

    switch (ctxt->op) {
    case BLE_GATT_ACCESS_OP_WRITE_CHR:
        /* US-114 : on constate la réception et on jette. Le pipeline de
           routage (déduplication, TTL, relais) est l'US-220 ; le décodage du
           paquet de couche 3 viendra de dengon-core par le FFI de l'US-307. */
        ESP_LOGI(TAG, "RX : %u octets reçus (conn=%u, handle=%u) — ignorés (US-220)",
                 (unsigned)OS_MBUF_PKTLEN(ctxt->om), conn_handle, attr_handle);
        return 0;

    default:
        /* CHAR_TX est en notification seule : aucune lecture ne doit arriver
           ici. Si c'est le cas, le pair ne respecte pas la table. */
        ESP_LOGW(TAG, "opération GATT inattendue : op=%u (handle=%u)",
                 ctxt->op, attr_handle);
        return BLE_ATT_ERR_UNLIKELY;
    }
}

void
dengon_gatt_register_cb(struct ble_gatt_register_ctxt *ctxt, void *arg)
{
    char buf[BLE_UUID_STR_LEN];

    (void)arg;

    switch (ctxt->op) {
    case BLE_GATT_REGISTER_OP_SVC:
        ESP_LOGI(TAG, "service %s -> handle %d",
                 ble_uuid_to_str(ctxt->svc.svc_def->uuid, buf),
                 ctxt->svc.handle);
        break;

    case BLE_GATT_REGISTER_OP_CHR:
        ESP_LOGI(TAG, "caractéristique %s -> def=%d val=%d",
                 ble_uuid_to_str(ctxt->chr.chr_def->uuid, buf),
                 ctxt->chr.def_handle, ctxt->chr.val_handle);
        break;

    case BLE_GATT_REGISTER_OP_DSC:
        ESP_LOGI(TAG, "descripteur %s -> handle %d",
                 ble_uuid_to_str(ctxt->dsc.dsc_def->uuid, buf),
                 ctxt->dsc.handle);
        break;

    default:
        break;
    }
}

int
dengon_gatt_init(void)
{
    int rc;

    ble_svc_gap_init();    /* service GAP 0x1800  (Device Name, Appearance)  */
    ble_svc_gatt_init();   /* service GATT 0x1801 (Service Changed)          */

    /* count_cfg dimensionne les tables internes ; add_svcs les remplit.
       L'ordre est imposé : inverser les deux fait échouer l'enregistrement. */
    rc = ble_gatts_count_cfg(dengon_gatt_svcs);
    if (rc != 0) {
        return rc;
    }

    return ble_gatts_add_svcs(dengon_gatt_svcs);
}
