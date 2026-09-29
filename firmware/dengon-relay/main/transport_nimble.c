// ---------------------------------------------------------------------------
// transport_nimble.c — le transport BLE du relais, sur NimBLE (US-220).
//
// Ce fichier ne contient QUE de la traduction : événements GAP/GATT de NimBLE
// -> appels au cœur pur (components/dengon_transport_core), et route rendue
// par le cœur -> écriture ou notification ATT. Toute la sémantique du contrat
// (LinkId, ordre des événements, motifs, erreurs) est dans le cœur, testée sur
// l'hôte. Ce qui est ici ne se vérifie qu'avec deux cartes.
//
// RÔLE GATT DOUBLE (docs/powl/03 §6.1) — sur chaque LIEN, un seul rôle :
//
//   nous = CENTRAL (on a initié)          nous = PÉRIPHÉRIQUE (le pair a initié)
//   ─────────────────────────────         ──────────────────────────────────────
//   TX : write sans réponse sur le        TX : notification sur NOTRE CHAR_TX
//        CHAR_RX du pair
//   RX : notification reçue du            RX : écriture reçue sur NOTRE CHAR_RX
//        CHAR_TX du pair                       (dengon_gatt.c -> on_rx)
//   prêt quand : MTU échangé, service     prêt quand : le pair s'abonne à notre
//     et caractéristiques découverts,       CHAR_TX (ou paresseusement à sa
//     abonnement au CHAR_TX écrit           première écriture)
//
// Qui est central ? Règle anti-boucle : le plus petit peerID initie
// (dengon_adv_should_initiate). Sans elle, deux cartes se connecteraient
// l'une à l'autre simultanément et gâcheraient un lien sur les trois.
//
// SURVIE À UNE COUPURE BRUTALE : un périphérique NimBLE cesse d'annoncer dès
// qu'il est connecté, et le scan est suspendu pendant une connexion sortante.
// Chaque fin de lien (DISCONNECT) et chaque fin de procédure (ADV_COMPLETE,
// DISC_COMPLETE, échec de connexion) passe donc par ensure_advertising() et
// ensure_scanning(). Oublier l'un d'eux donne une carte qui « meurt » après la
// première coupure — c'est le critère d'acceptation n°4 de l'US.
//
// CONCURRENCE : les callbacks NimBLE tournent dans la tâche host ; poll / send
// dans la tâche applicative. Le cœur est protégé par s_lock, et AUCUN appel à
// NimBLE n'est fait sous ce verrou : send prend la route sous verrou, le
// relâche, puis émet.
// ---------------------------------------------------------------------------
#include <stdio.h>
#include <string.h>

#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"

#include "host/ble_att.h"
#include "host/ble_hs.h"
#include "host/util/util.h"
#include "nimble/nimble_port.h"
#include "nimble/nimble_port_freertos.h"
#include "services/gap/ble_svc_gap.h"

#include "dengon_adv.h"
#include "dengon_gatt.h"
#include "dengon_transport.h"

static const char *TAG = "dengon-tr";

/* Durée d'une passe de scan. À chaque fin de passe (DISC_COMPLETE) le scan est
   relancé, ce qui réinitialise le filtre de doublons du contrôleur : un pair
   parti puis revenu redevient visible. Un scan « pour toujours » avec filtre
   de doublons ne reverrait JAMAIS une carte qu'il a déjà vue une fois. */
#define DENGON_SCAN_PASS_MS 10000

/* Délai maximal d'établissement d'une connexion sortante. */
#define DENGON_CONNECT_TIMEOUT_MS 5000

/* Valeur écrite dans le CCCD pour s'abonner aux notifications (LE). */
static const uint8_t CCCD_NOTIFY[2] = { 0x01, 0x00 };

/* --- État ------------------------------------------------------------------ */

static dengon_tc_t       s_tc;
static SemaphoreHandle_t s_lock;

static uint8_t s_own_addr_type;
static uint8_t s_own_addr[6];
static bool    s_synced;

/* Une seule connexion sortante à la fois : ble_gap_connect() refuse la
   seconde (BLE_HS_EALREADY), et le scan doit rester coupé pendant ce temps. */
static bool    s_connecting;
static int8_t  s_connecting_rssi;

static uint8_t s_mfg_data[DENGON_ADV_MFG_LEN];
static char    s_device_name[sizeof("dengon-relay-ffff")];

/* Découverte GATT côté central, par connexion. Accédé uniquement depuis la
   tâche host (callbacks GATT) : pas de verrou. */
typedef struct {
    bool     used;
    uint16_t conn_handle;
    uint16_t svc_start;
    uint16_t svc_end;
    uint16_t rx_val;  /* CHAR_RX du pair : on y écrit          */
    uint16_t tx_val;  /* CHAR_TX du pair : il nous notifie     */
    uint16_t tx_cccd; /* son CCCD : on y écrit 0x0001          */
} disc_state_t;

static disc_state_t s_disc[DENGON_TC_MAX_LINKS];

/* Tampon de copie d'un morceau L1 reçu (une PDU ATT, ≤ MTU - 3 = 514).
   Tâche host seulement. */
static uint8_t s_rx_buf[DENGON_TC_ATT_MTU_MAX - DENGON_TC_ATT_OVERHEAD];

static int  gap_event(struct ble_gap_event *event, void *arg);
static void ensure_advertising(void);
static void ensure_scanning(void);

static void lock(void)   { xSemaphoreTake(s_lock, portMAX_DELAY); }
static void unlock(void) { xSemaphoreGive(s_lock); }

/* --- 1. Petits utilitaires ------------------------------------------------- */

static size_t
link_count(void)
{
    size_t n;

    lock();
    n = dengon_tc_link_count(&s_tc);
    unlock();
    return n;
}

static bool
below_quota(void)
{
    return link_count() < s_tc.max_links;
}

static disc_state_t *
disc_get(uint16_t conn_handle, bool create)
{
    disc_state_t *free_slot = NULL;

    for (size_t i = 0; i < DENGON_TC_MAX_LINKS; i++) {
        if (s_disc[i].used && s_disc[i].conn_handle == conn_handle) {
            return &s_disc[i];
        }
        if (!s_disc[i].used && free_slot == NULL) {
            free_slot = &s_disc[i];
        }
    }
    if (create && free_slot != NULL) {
        memset(free_slot, 0, sizeof(*free_slot));
        free_slot->used = true;
        free_slot->conn_handle = conn_handle;
        return free_slot;
    }
    return NULL;
}

static void
disc_forget(uint16_t conn_handle)
{
    disc_state_t *d = disc_get(conn_handle, false);

    if (d != NULL) {
        d->used = false;
    }
}

/* Un morceau L1 est arrivé sur `conn_handle` : copie hors du mbuf, puis
   réassemblage ; la trame complète part en FrameReceived dans la file du
   cœur (US-312 : format de morceau de l'app Android). */
static void
deliver_rx(uint16_t conn_handle, const struct os_mbuf *om)
{
    uint16_t len = 0;
    dengon_tr_err_t err;
    uint32_t dropped;
    uint32_t bad;

    /* 1 morceau = 1 PDU ATT : une PDU ne dépasse jamais MTU - 3 <= 514. Un
       mbuf plus long serait une violation de la pile, on le jette. */
    if (OS_MBUF_PKTLEN(om) > sizeof(s_rx_buf) ||
        ble_hs_mbuf_to_flat(om, s_rx_buf, sizeof(s_rx_buf), &len) != 0) {
        ESP_LOGW(TAG, "RX conn=%u : trame illisible ou trop longue (%u o), jetée",
                 conn_handle, (unsigned)OS_MBUF_PKTLEN(om));
        return;
    }

    lock();
    err = dengon_tc_on_chunk(&s_tc, conn_handle, s_rx_buf, len);
    dropped = s_tc.dropped_frames;
    bad = s_tc.bad_chunks;
    unlock();

    if (err == DENGON_TR_BACKEND) {
        ESP_LOGW(TAG, "RX conn=%u : morceau de %u o jeté (file pleine : %lu, invalides : %lu)",
                 conn_handle, len, (unsigned long)dropped, (unsigned long)bad);
    } else if (err != DENGON_TR_OK) {
        ESP_LOGW(TAG, "RX conn=%u : %s", conn_handle, dengon_tr_err_str(err));
    }
}

/* Le lien est prêt dans les deux sens : PeerConnected. */
static void
link_ready(uint16_t conn_handle, const char *why)
{
    bool now;

    lock();
    now = dengon_tc_link_announce(&s_tc, conn_handle);
    unlock();
    if (now) {
        ESP_LOGI(TAG, "lien conn=%u prêt (%s) -> PeerConnected", conn_handle, why);
    }
}

/* --- 2. Annonce (rôle périphérique) --------------------------------------- */

static void
build_adv_payloads(const uint8_t peer_id[8])
{
    dengon_adv_build_mfg(peer_id, DENGON_ADV_F_RELAY | DENGON_ADV_F_ACCEPTS_CONN, s_mfg_data);

    /* Le suffixe distingue deux cartes posées côte à côte. */
    snprintf(s_device_name, sizeof(s_device_name), "dengon-relay-%02x%02x",
             peer_id[0], peer_id[1]);
}

static void
advertise(void)
{
    struct ble_hs_adv_fields adv = { 0 };
    struct ble_hs_adv_fields rsp = { 0 };
    struct ble_gap_adv_params params = { 0 };
    int rc;

    /* Paquet principal : 3 + 18 + 9 = 30 octets sur 31. ⚠ BUDGET SATURÉ :
       tout ajout fait renvoyer BLE_HS_EMSGSIZE — à l'exécution. L'UUID de
       service reste ici : c'est sur lui que les pairs filtrent leur scan. */
    adv.flags = BLE_HS_ADV_F_DISC_GEN | BLE_HS_ADV_F_BREDR_UNSUP;
    adv.uuids128 = &dengon_svc_uuid;
    adv.num_uuids128 = 1;
    adv.uuids128_is_complete = 1;
    adv.mfg_data = s_mfg_data;
    adv.mfg_data_len = (uint8_t)sizeof(s_mfg_data);

    rc = ble_gap_adv_set_fields(&adv);
    if (rc != 0) {
        ESP_LOGE(TAG, "ble_gap_adv_set_fields a échoué : rc=%d "
                      "(0x0C = BLE_HS_EMSGSIZE, le paquet dépasse 31 octets)", rc);
        return;
    }

    /* Réponse de scan : le nom, relégué ici faute de place. */
    rsp.name = (const uint8_t *)s_device_name;
    rsp.name_len = (uint8_t)strlen(s_device_name);
    rsp.name_is_complete = 1;
    rsp.tx_pwr_lvl_is_present = 1;
    rsp.tx_pwr_lvl = BLE_HS_ADV_TX_PWR_LVL_AUTO;

    rc = ble_gap_adv_rsp_set_fields(&rsp);
    if (rc != 0) {
        ESP_LOGE(TAG, "ble_gap_adv_rsp_set_fields a échoué : rc=%d", rc);
        return;
    }

    /* 100-150 ms : le relais est sur secteur, on privilégie la découverte. */
    params.conn_mode = BLE_GAP_CONN_MODE_UND;
    params.disc_mode = BLE_GAP_DISC_MODE_GEN;
    params.itvl_min = BLE_GAP_ADV_FAST_INTERVAL2_MIN;
    params.itvl_max = BLE_GAP_ADV_FAST_INTERVAL2_MAX;

    rc = ble_gap_adv_start(s_own_addr_type, NULL, BLE_HS_FOREVER, &params, gap_event, NULL);
    if (rc != 0 && rc != BLE_HS_EALREADY) {
        ESP_LOGE(TAG, "ble_gap_adv_start a échoué : rc=%d", rc);
        return;
    }
    ESP_LOGI(TAG, "annonce en cours sous le nom « %s »", s_device_name);
}

static void
ensure_advertising(void)
{
    if (!s_synced || !s_tc.cfg.advertise || ble_gap_adv_active()) {
        return;
    }
    /* Quota atteint : on se tait. Un pair qui se connecterait serait aussitôt
       rejeté, autant ne pas l'inviter. */
    if (!below_quota()) {
        ESP_LOGI(TAG, "quota de %u liens atteint : annonce suspendue",
                 (unsigned)s_tc.max_links);
        return;
    }
    advertise();
}

/* --- 3. Scan et connexion sortante (rôle central) -------------------------- */

static bool
adv_has_dengon_service(const struct ble_hs_adv_fields *f)
{
    for (int i = 0; i < f->num_uuids128; i++) {
        if (ble_uuid_cmp(&f->uuids128[i].u, &dengon_svc_uuid.u) == 0) {
            return true;
        }
    }
    return false;
}

static void
on_disc(const struct ble_gap_disc_desc *d)
{
    struct ble_hs_adv_fields f;
    struct ble_gap_conn_desc desc;
    dengon_adv_info_t info;
    int rc;

    /* Seules les annonces connectables nous intéressent. */
    if (d->event_type != BLE_HCI_ADV_RPT_EVTYPE_ADV_IND &&
        d->event_type != BLE_HCI_ADV_RPT_EVTYPE_DIR_IND) {
        return;
    }
    if (ble_hs_adv_parse_fields(&f, d->data, d->length_data) != 0 ||
        !adv_has_dengon_service(&f) ||
        !dengon_adv_parse_mfg(f.mfg_data, f.mfg_data_len, &info)) {
        return;
    }

    /* Déjà relié à cette carte (dans un sens ou dans l'autre) ? */
    if (ble_gap_conn_find_by_addr(&d->addr, &desc) == 0) {
        return;
    }
    if (s_connecting || !below_quota()) {
        return;
    }

    if (!dengon_adv_should_initiate(s_tc.cfg.local_peer_id, info.peer_prefix,
                                    s_own_addr, d->addr.val)) {
        ESP_LOGD(TAG, "pair %02x%02x%02x%02x vu : peerID plus petit, c'est à lui d'initier",
                 info.peer_prefix[0], info.peer_prefix[1], info.peer_prefix[2],
                 info.peer_prefix[3]);
        return;
    }

    ESP_LOGI(TAG, "pair %02x%02x%02x%02x vu (rssi=%d, flags=0x%02x) : connexion",
             info.peer_prefix[0], info.peer_prefix[1], info.peer_prefix[2],
             info.peer_prefix[3], d->rssi, info.flags);

    /* Le scan doit être arrêté avant ble_gap_connect(), sinon BLE_HS_EBUSY. */
    s_connecting = true;
    s_connecting_rssi = d->rssi;
    ble_gap_disc_cancel();

    rc = ble_gap_connect(s_own_addr_type, &d->addr, DENGON_CONNECT_TIMEOUT_MS, NULL,
                         gap_event, NULL);
    if (rc != 0) {
        ESP_LOGW(TAG, "ble_gap_connect a échoué : rc=%d", rc);
        s_connecting = false;
        ensure_scanning();
    }
}

static void
ensure_scanning(void)
{
    struct ble_gap_disc_params dp = { 0 };
    int rc;

    if (!s_synced || !s_tc.cfg.scan || s_connecting || ble_gap_disc_active()) {
        return;
    }
    if (!below_quota()) {
        return;
    }

    /* Passif : tout ce qu'il faut (UUID + manufacturer data) est dans le
       paquet principal, inutile de solliciter la réponse de scan. */
    dp.passive = 1;
    dp.filter_duplicates = 1;

    rc = ble_gap_disc(s_own_addr_type, DENGON_SCAN_PASS_MS, &dp, gap_event, NULL);
    if (rc != 0 && rc != BLE_HS_EALREADY) {
        ESP_LOGE(TAG, "ble_gap_disc a échoué : rc=%d", rc);
    }
}

/* --- 4. Découverte GATT et abonnement (rôle central) ----------------------- */

static void
central_fail(uint16_t conn_handle, const char *step, int status)
{
    /* Le lien n'a jamais été annoncé : le cœur le fermera en silence. */
    ESP_LOGW(TAG, "conn=%u : échec de %s (status=%d) — lien abandonné",
             conn_handle, step, status);
    ble_gap_terminate(conn_handle, BLE_ERR_REM_USER_CONN_TERM);
}

static int
on_subscribed(uint16_t conn_handle, const struct ble_gatt_error *error,
              struct ble_gatt_attr *attr, void *arg)
{
    disc_state_t *d = disc_get(conn_handle, false);

    (void)attr;
    (void)arg;

    if (error->status != 0 || d == NULL) {
        central_fail(conn_handle, "l'abonnement au CHAR_TX", error->status);
        return 0;
    }

    lock();
    dengon_tc_link_set_peer_rx(&s_tc, conn_handle, d->rx_val);
    unlock();
    link_ready(conn_handle, "central, abonné");
    return 0;
}

static int
on_dsc(uint16_t conn_handle, const struct ble_gatt_error *error, uint16_t chr_val_handle,
       const struct ble_gatt_dsc *dsc, void *arg)
{
    disc_state_t *d = disc_get(conn_handle, false);
    int rc;

    (void)chr_val_handle;
    (void)arg;

    if (d == NULL) {
        return 0;
    }
    if (error->status == 0) {
        if (d->tx_cccd == 0 &&
            ble_uuid_cmp(&dsc->uuid.u, BLE_UUID16_DECLARE(BLE_GATT_DSC_CLT_CFG_UUID16)) == 0) {
            d->tx_cccd = dsc->handle;
        }
        return 0;
    }
    if (error->status != BLE_HS_EDONE || d->tx_cccd == 0) {
        central_fail(conn_handle, "la découverte du CCCD", error->status);
        return 0;
    }

    rc = ble_gattc_write_flat(conn_handle, d->tx_cccd, CCCD_NOTIFY, sizeof(CCCD_NOTIFY),
                              on_subscribed, NULL);
    if (rc != 0) {
        central_fail(conn_handle, "l'écriture du CCCD", rc);
    }
    return 0;
}

static int
on_chr(uint16_t conn_handle, const struct ble_gatt_error *error,
       const struct ble_gatt_chr *chr, void *arg)
{
    disc_state_t *d = disc_get(conn_handle, false);
    int rc;

    (void)arg;

    if (d == NULL) {
        return 0;
    }
    if (error->status == 0) {
        if (ble_uuid_cmp(&chr->uuid.u, &dengon_chr_rx_uuid.u) == 0) {
            d->rx_val = chr->val_handle;
        } else if (ble_uuid_cmp(&chr->uuid.u, &dengon_chr_tx_uuid.u) == 0) {
            d->tx_val = chr->val_handle;
        }
        return 0;
    }
    if (error->status != BLE_HS_EDONE || d->rx_val == 0 || d->tx_val == 0) {
        central_fail(conn_handle, "la découverte de CHAR_RX / CHAR_TX", error->status);
        return 0;
    }

    /* Le CCCD suit la valeur de CHAR_TX ; on cherche jusqu'à la fin du
       service plutôt que de supposer tx_val + 1. */
    rc = ble_gattc_disc_all_dscs(conn_handle, d->tx_val, d->svc_end, on_dsc, NULL);
    if (rc != 0) {
        central_fail(conn_handle, "le lancement de la découverte des descripteurs", rc);
    }
    return 0;
}

static int
on_svc(uint16_t conn_handle, const struct ble_gatt_error *error,
       const struct ble_gatt_svc *svc, void *arg)
{
    disc_state_t *d = disc_get(conn_handle, false);
    int rc;

    (void)arg;

    if (d == NULL) {
        return 0;
    }
    if (error->status == 0) {
        d->svc_start = svc->start_handle;
        d->svc_end = svc->end_handle;
        return 0;
    }
    if (error->status != BLE_HS_EDONE || d->svc_start == 0) {
        central_fail(conn_handle, "la découverte du service dengon", error->status);
        return 0;
    }

    rc = ble_gattc_disc_all_chrs(conn_handle, d->svc_start, d->svc_end, on_chr, NULL);
    if (rc != 0) {
        central_fail(conn_handle, "le lancement de la découverte des caractéristiques", rc);
    }
    return 0;
}

static int
on_mtu(uint16_t conn_handle, const struct ble_gatt_error *error, uint16_t mtu, void *arg)
{
    int rc;

    (void)mtu;
    (void)arg;

    /* Le MTU n'est PAS enregistré ici : BLE_GAP_EVENT_MTU l'a déjà fait, et
       c'est le seul chemin (il sert aussi au rôle périphérique). NimBLE émet
       cet événement AVANT d'appeler ce callback (ble_att_clt_rx_mtu :
       ble_gap_mtu_event() puis ble_gattc_rx_mtu()) : le lien porte donc déjà
       le bon MTU quand la découverte démarre.
       Un refus d'échange n'est pas fatal : on reste à 23 (20 octets utiles),
       le transport découpe en morceaux plus fins. */
    if (error->status != 0) {
        ESP_LOGW(TAG, "conn=%u : échange MTU refusé (status=%d), on reste à 23",
                 conn_handle, error->status);
    }

    /* Les procédures ATT sont enchaînées, jamais lancées en parallèle : ATT
       n'autorise qu'une requête en vol par lien. */
    rc = ble_gattc_disc_svc_by_uuid(conn_handle, &dengon_svc_uuid.u, on_svc, NULL);
    if (rc != 0) {
        central_fail(conn_handle, "le lancement de la découverte du service", rc);
    }
    return 0;
}

/* --- 5. Événements GAP ------------------------------------------------------ */

static void
on_connect(const struct ble_gap_event *event)
{
    uint16_t conn = event->connect.conn_handle;
    struct ble_gap_conn_desc desc;
    dengon_link_role_t role;
    dengon_tr_err_t err;
    bool central;
    int rc;

    if (event->connect.status != 0) {
        /* Échec d'une connexion SORTANTE (timeout…) ou d'une annonce. */
        ESP_LOGW(TAG, "connexion échouée (status=%d)", event->connect.status);
        s_connecting = false;
        ensure_scanning();
        ensure_advertising();
        return;
    }

    if (ble_gap_conn_find(conn, &desc) != 0) {
        ESP_LOGE(TAG, "conn=%u : descripteur introuvable", conn);
        return;
    }
    central = desc.role == BLE_GAP_ROLE_MASTER;
    role = central ? DENGON_ROLE_CENTRAL : DENGON_ROLE_PERIPHERAL;
    if (central) {
        s_connecting = false;
    }

    lock();
    err = dengon_tc_link_open(&s_tc, conn, role, NULL);
    if (err == DENGON_TR_OK && central) {
        dengon_tc_link_set_rssi(&s_tc, conn, s_connecting_rssi);
    }
    unlock();

    if (err != DENGON_TR_OK) {
        /* Quota atteint (typiquement : deux pairs se sont connectés à nous
           pendant la même fenêtre). Jamais annoncé, donc jamais vu du cœur. */
        ESP_LOGW(TAG, "conn=%u refusée : %s", conn, dengon_tr_err_str(err));
        ble_gap_terminate(conn, BLE_ERR_REM_USER_CONN_TERM);
        return;
    }

    ESP_LOGI(TAG, "connexion établie conn=%u, rôle %s", conn,
             central ? "central (on a initié)" : "périphérique (le pair a initié)");

    if (central) {
        if (disc_get(conn, true) == NULL) {
            central_fail(conn, "l'allocation de l'état de découverte", BLE_HS_ENOMEM);
            return;
        }
        rc = ble_gattc_exchange_mtu(conn, on_mtu, NULL);
        if (rc != 0) {
            central_fail(conn, "le lancement de l'échange MTU", rc);
        }
    }

    /* L'annonce s'est arrêtée (périphérique) ou le scan a été coupé
       (central) : on relance ce qui peut l'être, dans la limite du quota. */
    ensure_advertising();
    ensure_scanning();
}

static void
on_disconnect(const struct ble_gap_event *event)
{
    uint16_t conn = event->disconnect.conn.conn_handle;
    int reason = event->disconnect.reason;
    uint8_t hci;
    bool emitted;

    /* NimBLE rend BLE_HS_ERR_HCI_BASE + code HCI. Un code hors de cette plage
       (erreur du host) n'a rien d'annoncé : traité comme une coupure. */
    if (reason >= BLE_HS_ERR_HCI_BASE && reason < BLE_HS_ERR_HCI_BASE + 0x100) {
        hci = (uint8_t)(reason - BLE_HS_ERR_HCI_BASE);
    } else {
        hci = DENGON_HCI_CONN_SPVN_TMO;
    }

    lock();
    emitted = dengon_tc_link_close(&s_tc, conn, hci);
    unlock();
    disc_forget(conn);

    ESP_LOGI(TAG, "déconnexion conn=%u (HCI 0x%02x -> %s)%s", conn, hci,
             dengon_disc_reason_str(dengon_tc_map_hci_reason(hci)),
             emitted ? " -> PeerDisconnected" : " (lien jamais annoncé)");

    /* ⚠ LE point du critère n°4 : sans ces deux appels, la carte disparaît
       après la première coupure. */
    ensure_advertising();
    ensure_scanning();
}

static int
gap_event(struct ble_gap_event *event, void *arg)
{
    (void)arg;

    switch (event->type) {
    case BLE_GAP_EVENT_CONNECT:
        on_connect(event);
        return 0;

    case BLE_GAP_EVENT_DISCONNECT:
        on_disconnect(event);
        return 0;

    case BLE_GAP_EVENT_ADV_COMPLETE:
        ensure_advertising();
        return 0;

    case BLE_GAP_EVENT_DISC:
        on_disc(&event->disc);
        return 0;

    case BLE_GAP_EVENT_DISC_COMPLETE:
        /* Fin de passe (ou annulation avant connexion) : on relance, ce qui
           remet à zéro le filtre de doublons. */
        ensure_scanning();
        return 0;

    case BLE_GAP_EVENT_MTU:
        /* SEUL endroit où le MTU d'un lien est enregistré, dans les deux
           rôles : que l'échange vienne de nous (central) ou du pair. */
        lock();
        dengon_tc_link_set_mtu(&s_tc, event->mtu.conn_handle, event->mtu.value);
        unlock();
        ESP_LOGI(TAG, "conn=%u : ATT MTU négocié = %u (trame max %u o)",
                 event->mtu.conn_handle, event->mtu.value,
                 (unsigned)(event->mtu.value - DENGON_TC_ATT_OVERHEAD));
        return 0;

    case BLE_GAP_EVENT_SUBSCRIBE:
        /* Côté périphérique : le pair s'abonne à NOTRE CHAR_TX. */
        if (event->subscribe.attr_handle == dengon_gatt_tx_val_handle() &&
            event->subscribe.cur_notify) {
            link_ready(event->subscribe.conn_handle, "périphérique, pair abonné");
        }
        return 0;

    case BLE_GAP_EVENT_NOTIFY_RX: {
        /* Côté central : le pair nous notifie depuis SON CHAR_TX. Le mbuf
           appartient à la pile, qui le libère au retour : on copie. */
        disc_state_t *d = disc_get(event->notify_rx.conn_handle, false);

        if (d != NULL && event->notify_rx.attr_handle == d->tx_val) {
            deliver_rx(event->notify_rx.conn_handle, event->notify_rx.om);
        }
        return 0;
    }

    default:
        return 0;
    }
}

/* --- 6. Callbacks du host --------------------------------------------------- */

static void
on_reset(int reason)
{
    ESP_LOGE(TAG, "pile BLE réinitialisée par le contrôleur (raison=%d)", reason);
    s_synced = false;
}

static void
on_sync(void)
{
    int rc;

    /* Sans adresse identité, ble_gap_adv_start() et ble_gap_disc() échouent. */
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
    /* Notre adresse sert à départager une égalité de préfixe de peerID. */
    rc = ble_hs_id_copy_addr(s_own_addr_type, s_own_addr, NULL);
    if (rc != 0) {
        ESP_LOGE(TAG, "ble_hs_id_copy_addr a échoué : rc=%d", rc);
        return;
    }

    s_synced = true;
    ensure_advertising();
    ensure_scanning();
    ESP_LOGI(TAG, "contrôleur synchronisé : annonce=%d scan=%d, %u liens max",
             s_tc.cfg.advertise, s_tc.cfg.scan, (unsigned)s_tc.max_links);
}

static void
host_task(void *param)
{
    (void)param;

    ESP_LOGI(TAG, "tâche host NimBLE démarrée");
    nimble_port_run(); /* ne rend la main qu'à nimble_port_stop() */
    nimble_port_freertos_deinit();
}

/* Fourni par le portage ESP32 de NimBLE. */
void ble_store_config_init(void);

/* --- 7. API publique -------------------------------------------------------- */

dengon_tr_err_t
dengon_transport_start(const dengon_transport_config_t *cfg)
{
    dengon_tr_err_t err;
    size_t eff = 0;
    uint16_t mtu;
    int rc;

    if (s_lock == NULL) {
        s_lock = xSemaphoreCreateMutex();
        if (s_lock == NULL) {
            return DENGON_TR_BACKEND;
        }
        dengon_tc_init(&s_tc);
    }

    lock();
    err = dengon_tc_start(&s_tc, cfg, CONFIG_BT_NIMBLE_MAX_CONNECTIONS, &eff);
    unlock();
    if (err != DENGON_TR_OK) {
        return err;
    }
    if (eff < cfg->max_connections) {
        ESP_LOGW(TAG, "max_connections=%u demandé, borné à %u (CONFIG_BT_NIMBLE_MAX_CONNECTIONS)",
                 (unsigned)cfg->max_connections, (unsigned)eff);
    }

    build_adv_payloads(cfg->local_peer_id);

    /* Séquence imposée par ESP-IDF : nimble_port_init (host arrêté) ->
       callbacks -> table GATT -> nimble_port_freertos_init (host lancé). */
    if (nimble_port_init() != ESP_OK) {
        ESP_LOGE(TAG, "nimble_port_init a échoué");
        goto backend;
    }

    ble_hs_cfg.reset_cb = on_reset;
    ble_hs_cfg.sync_cb = on_sync;
    ble_hs_cfg.gatts_register_cb = dengon_gatt_register_cb;
    ble_hs_cfg.store_status_cb = ble_store_util_status_rr;

    rc = dengon_gatt_init();
    if (rc != 0) {
        ESP_LOGE(TAG, "dengon_gatt_init a échoué : rc=%d", rc);
        goto backend;
    }
    rc = ble_svc_gap_device_name_set(s_device_name);
    if (rc != 0) {
        ESP_LOGE(TAG, "ble_svc_gap_device_name_set a échoué : rc=%d", rc);
        goto backend;
    }

    /* 517 visé ; borné à [23, 517] pour ne pas se faire rendre BLE_HS_EINVAL. */
    mtu = cfg->preferred_mtu;
    if (mtu < DENGON_TC_ATT_MTU_MIN) {
        mtu = DENGON_TC_ATT_MTU_MIN;
    } else if (mtu > DENGON_TC_ATT_MTU_MAX) {
        mtu = DENGON_TC_ATT_MTU_MAX;
    }
    rc = ble_att_set_preferred_mtu(mtu);
    if (rc != 0) {
        ESP_LOGE(TAG, "ble_att_set_preferred_mtu(%u) a échoué : rc=%d", mtu, rc);
        goto backend;
    }

    ble_store_config_init();

    /* C'est ICI que le host démarre ; on_sync suivra. */
    nimble_port_freertos_init(host_task);
    return DENGON_TR_OK;

backend:
    /* Le transport n'a pas démarré : il doit le rester aux yeux de l'appelant. */
    lock();
    dengon_tc_init(&s_tc);
    unlock();
    return DENGON_TR_BACKEND;
}

size_t
dengon_transport_poll(dengon_transport_event_t *out, size_t cap)
{
    size_t n;

    if (s_lock == NULL) {
        return 0; /* avant start : Vec vide */
    }
    lock();
    n = dengon_tc_poll(&s_tc, out, cap);
    unlock();
    return n;
}

void
dengon_transport_event_free(dengon_transport_event_t *ev)
{
    dengon_tc_event_free(ev);
}

/* Émission d'un morceau L1 : en-tête ‖ données, construit dans le mbuf
   (pas de tampon sur la pile de l'appelant). */
static int
emit_chunk(const dengon_tc_route_t *r, uint8_t hdr, const uint8_t *data, size_t n)
{
    struct os_mbuf *om = ble_hs_mbuf_from_flat(&hdr, DENGON_TC_CHUNK_HDR);

    if (om == NULL) {
        return BLE_HS_ENOMEM; /* plus de mbuf : pile saturée */
    }
    if (n > 0 && os_mbuf_append(om, data, (uint16_t)n) != 0) {
        os_mbuf_free_chain(om);
        return BLE_HS_ENOMEM;
    }
    /* Les deux appels consomment le mbuf, même en cas d'erreur. */
    if (r->role == DENGON_ROLE_CENTRAL) {
        return ble_gattc_write_no_rsp(r->conn_handle, r->peer_rx_handle, om);
    }
    return ble_gatts_notify_custom(r->conn_handle, dengon_gatt_tx_val_handle(), om);
}

/* Émission effective sur une route validée par le cœur. Hors verrou. La
   trame part en morceaux L1 au MTU du lien (US-312). */
static dengon_tr_err_t
emit(const dengon_tc_route_t *r, const uint8_t *bytes, size_t len)
{
    size_t total = dengon_tc_chunk_count(len, r->mtu);
    int rc = 0;

    for (size_t i = 0; i < total && rc == 0; i++) {
        size_t off = 0;
        size_t n = 0;
        uint8_t hdr = dengon_tc_chunk_at(len, r->mtu, i, &off, &n);

        rc = emit_chunk(r, hdr, bytes + off, n);
    }

    if (rc == 0) {
        return DENGON_TR_OK;
    }
    /* Le pair est tombé entre la prise de route et l'émission : condition
       normale, pas une erreur de pile. */
    if (rc == BLE_HS_ENOTCONN) {
        return DENGON_TR_UNKNOWN_PEER;
    }
    ESP_LOGW(TAG, "émission conn=%u : rc=%d", r->conn_handle, rc);
    return DENGON_TR_BACKEND;
}

dengon_tr_err_t
dengon_transport_send(dengon_link_id_t link, const uint8_t *bytes, size_t len, size_t *max_out)
{
    dengon_tc_route_t route;
    dengon_tr_err_t err;

    if (s_lock == NULL) {
        return DENGON_TR_NOT_STARTED;
    }
    lock();
    err = dengon_tc_route_for_send(&s_tc, link, len, &route, max_out);
    unlock();
    if (err != DENGON_TR_OK) {
        return err;
    }
    return emit(&route, bytes, len);
}

dengon_tr_err_t
dengon_transport_broadcast(const uint8_t *bytes, size_t len)
{
    dengon_tc_route_t routes[DENGON_TC_MAX_LINKS];
    dengon_tr_err_t err;
    size_t n = 0;

    if (s_lock == NULL) {
        return DENGON_TR_NOT_STARTED;
    }
    lock();
    err = dengon_tc_routes_for_broadcast(&s_tc, len, routes, DENGON_TC_MAX_LINKS, &n);
    unlock();
    if (err != DENGON_TR_OK) {
        return err;
    }

    /* Au mieux : un échec vers un pair n'empêche pas de servir les autres. */
    for (size_t i = 0; i < n; i++) {
        (void)emit(&routes[i], bytes, len);
    }
    return DENGON_TR_OK;
}

void
dengon_transport_disconnect_all(void)
{
    uint16_t conns[DENGON_TC_MAX_LINKS];
    size_t n = 0;

    if (s_lock == NULL) {
        return;
    }
    lock();
    for (size_t i = 0; i < DENGON_TC_MAX_LINKS; i++) {
        if (s_tc.links[i].used) {
            conns[n++] = s_tc.links[i].conn_handle;
        }
    }
    unlock();

    for (size_t i = 0; i < n; i++) {
        /* 0x13 envoyé au pair (Propre chez lui) ; nous recevrons 0x16 (Locale). */
        ble_gap_terminate(conns[i], BLE_ERR_REM_USER_CONN_TERM);
    }
}

void
dengon_transport_on_rx(uint16_t conn_handle, const struct os_mbuf *om)
{
    deliver_rx(conn_handle, om);
}
