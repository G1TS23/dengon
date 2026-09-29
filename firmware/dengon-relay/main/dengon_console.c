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
//   relay     compteurs du relais.
// ---------------------------------------------------------------------------
#include "dengon_console.h"

#include <inttypes.h>
#include <stdio.h>

#include "esp_console.h"
#include "esp_log.h"
#include "esp_system.h"

#include "dengon_relay_app.h"
#include "dengon_store.h"

static const char *TAG = "dengon-console";

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
    };
    esp_err_t err;

    cfg.prompt = "dengon>";
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
