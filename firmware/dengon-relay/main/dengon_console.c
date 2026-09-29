// ---------------------------------------------------------------------------
// dengon_console.c — console série du relais (US-308).
//
// Sert la vérification du critère « survit à esp_restart() sans rupture de
// chaîne » sans Wi-Fi (l'export HTTPS est US-309) :
//
//   ledger    imprime ledger.old puis ledger.bin en hexadécimal, entre deux
//             marqueurs ; tools/dump_ledger.py en refait un .bin pour
//             `dengon-verify --pubkey <clé>` (clé imprimée au boot) ;
//   restart   esp_restart() ;
//   relay     compteurs du relais ;
//   wifi      <ssid> [mdp] : réseau Wi-Fi, mémorisé en NVS (US-309) ;
//   dash      id | token <jwt> | status : export vers le dashboard (US-309).
// ---------------------------------------------------------------------------
#include "dengon_console.h"

#include <inttypes.h>
#include <stdio.h>
#include <string.h>

#include "esp_console.h"
#include "esp_log.h"
#include "esp_system.h"

#include "dengon_relay_app.h"
#include "dengon_ship.h"
#include "dengon_store.h"
#include "dengon_wifi.h"

static const char *TAG = "dengon-console";

/* `relay-xxxxxx` + `\0` (dengon_relay_node_id). */
#define DENGON_NODE_ID_LEN 13

#define HEX_LIGNE 32

static void
dump_fichier(const char *chemin)
{
    uint8_t buf[HEX_LIGNE];
    size_t  n;
    FILE   *f = fopen(chemin, "rb");

    if (f == NULL) {
        return;
    }
    while ((n = fread(buf, 1, sizeof(buf), f)) > 0) {
        for (size_t i = 0; i < n; i++) {
            printf("%02x", buf[i]);
        }
        printf("\n");
    }
    fclose(f);
}

static int
cmd_ledger(int argc, char **argv)
{
    (void)argc;
    (void)argv;
    /* Pas de mutex du relais ici : seule la tâche ledger écrit ces fichiers,
       par ajouts fsync-és ; au pire la dernière entrée manque au dump. */
    printf("DENGON-LEDGER-BEGIN\n");
    dump_fichier(DENGON_LEDGER_OLD_PATH);
    dump_fichier(DENGON_LEDGER_PATH);
    printf("DENGON-LEDGER-END\n");
    return 0;
}

static int
cmd_restart(int argc, char **argv)
{
    (void)argc;
    (void)argv;
    printf("esp_restart()\n");
    fflush(stdout);
    esp_restart();
    return 0;
}

static int
cmd_relay(int argc, char **argv)
{
    DengonRelayStats st;

    (void)argc;
    (void)argv;
    dengon_relay_stats(dengon_relay_app_lock(), &st);
    dengon_relay_app_unlock();
    printf("relayés=%" PRIu64 " enveloppes=%" PRIu32 " déposées=%" PRIu64 " remises=%" PRIu64
           " cache=%" PRIu32 " illisibles=%" PRIu64 " non_authentiques=%" PRIu64
           " sans_heure=%" PRIu64 "\n",
           st.relayed, st.envelopes_held, st.envelopes_stored, st.envelopes_handed_off,
           st.cache_len, st.malformed, st.unauthentic, st.clock_unknown);
    return 0;
}

static int
cmd_wifi(int argc, char **argv)
{
    esp_err_t err;

    if (argc < 2 || argc > 3) {
        printf("usage : wifi <ssid> [mot_de_passe]   (actuel : « %s », %s)\n", dengon_wifi_ssid(),
               dengon_wifi_is_up() ? "connecté" : "non connecté");
        return 1;
    }
    err = dengon_wifi_set_credentials(argv[1], argc == 3 ? argv[2] : "");
    printf("wifi : %s\n", err == ESP_OK ? "enregistré, connexion en cours" : esp_err_to_name(err));
    return err == ESP_OK ? 0 : 1;
}

/* `dash id` : de quoi enregistrer le relais auprès du dashboard
   (tools/register_relay.py). */
static void
dash_id(void)
{
    char    node_id[DENGON_NODE_ID_LEN];
    uint8_t pk[32];

    DengonRelay *r = dengon_relay_app_lock();
    dengon_relay_node_id(r, node_id);
    dengon_relay_verifying_key(r, pk);
    dengon_relay_app_unlock();
    printf("node_id  : %s\npub_sign : ", node_id);
    for (size_t i = 0; i < sizeof(pk); i++) {
        printf("%02x", pk[i]);
    }
    printf("\nenregistrement : tools/register_relay.py --node-id %s --pub-sign <pub_sign>\n",
           node_id);
}

static void
dash_status(void)
{
    dengon_ship_stats_t st;

    dengon_ship_get_stats(&st);
    printf("dashboard : " CONFIG_DENGON_DASH_URL "\n"
           "jeton     : %s\nracine CA : %s\nwifi      : %s (« %s », %d dBm)\n"
           "ring      : %u %% plein, %u en attente\n"
           "envoyés   : %" PRIu64 "  perdus/refusés : %" PRIu64 "  dernier statut HTTP : %d\n"
           "pile_min tâche d'envoi : %u o\n",
           st.has_token ? "présent" : "absent", st.has_root_ca ? "embarquée" : "absente",
           dengon_wifi_is_up() ? "connecté" : "non connecté", dengon_wifi_ssid(),
           dengon_wifi_rssi(), st.fill_pct, (unsigned)st.pending, st.sent, st.dropped,
           st.last_status, st.stack_min);
}

static int
cmd_dash(int argc, char **argv)
{
    if (argc == 2 && strcmp(argv[1], "id") == 0) {
        dash_id();
        return 0;
    }
    if (argc == 2 && strcmp(argv[1], "status") == 0) {
        dash_status();
        return 0;
    }
    if (argc == 3 && strcmp(argv[1], "token") == 0) {
        esp_err_t err = dengon_ship_set_token(argv[2]);

        printf("jeton : %s\n", err == ESP_OK ? "enregistré" : esp_err_to_name(err));
        return err == ESP_OK ? 0 : 1;
    }
    printf("usage : dash id | dash status | dash token <jwt>\n");
    return 1;
}

esp_err_t
dengon_console_start(void)
{
    esp_console_repl_t       *repl     = NULL;
    esp_console_repl_config_t cfg      = ESP_CONSOLE_REPL_CONFIG_DEFAULT();
    esp_console_dev_uart_config_t uart = ESP_CONSOLE_DEV_UART_CONFIG_DEFAULT();
    const esp_console_cmd_t   cmds[] = {
        { .command = "ledger", .help = "Exporte le journal (hex) pour dengon-verify",
          .func = cmd_ledger },
        { .command = "restart", .help = "Redémarre la carte (esp_restart)", .func = cmd_restart },
        { .command = "relay", .help = "Compteurs du relais", .func = cmd_relay },
        { .command = "wifi", .help = "wifi <ssid> [mdp] : réseau pour joindre le dashboard",
          .func = cmd_wifi },
        { .command = "dash", .help = "dash id | status | token <jwt> : export vers le dashboard",
          .func = cmd_dash },
    };
    esp_err_t err;

    cfg.prompt = "dengon>";
    /* `dash token <jwt>` : un JWT dépasse vite les 256 caractères par défaut. */
    cfg.max_cmdline_length = 1024;
    for (size_t i = 0; i < sizeof(cmds) / sizeof(cmds[0]); i++) {
        err = esp_console_cmd_register(&cmds[i]);
        if (err != ESP_OK) {
            return err;
        }
    }
    err = esp_console_new_repl_uart(&uart, &cfg, &repl);
    if (err == ESP_OK) {
        err = esp_console_start_repl(repl);
    }
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "console indisponible : %s", esp_err_to_name(err));
    }
    return err;
}
