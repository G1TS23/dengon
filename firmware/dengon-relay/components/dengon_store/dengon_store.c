// ---------------------------------------------------------------------------
// dengon_store.c — voir dengon_store.h (US-308).
// ---------------------------------------------------------------------------
#include "dengon_store.h"

#include <string.h>

#include "bootloader_random.h"
#include "esp_littlefs.h"
#include "esp_log.h"
#include "esp_random.h"
#include "nvs.h"

static const char *TAG = "dengon-store";

#define NVS_NAMESPACE  "dengon"
#define KEY_DH         "dh_secret"
#define KEY_SIGN       "sign_seed"
#define KEY_CURSOR     "ledger_cur"

/* Curseur sérialisé : next_seq (8, big-endian) ‖ last_hash (32). */
#define CURSOR_LEN (8 + DENGON_LEDGER_HASH_LEN)

esp_err_t
dengon_store_init(void)
{
    const esp_vfs_littlefs_conf_t conf = {
        .base_path              = DENGON_STORE_BASE_PATH,
        .partition_label        = "littlefs",
        .format_if_mount_failed = true,
        .dont_mount             = false,
    };
    esp_err_t err = esp_vfs_littlefs_register(&conf);
    size_t    total = 0;
    size_t    used  = 0;

    if (err != ESP_OK) {
        ESP_LOGE(TAG, "montage littlefs impossible : %s", esp_err_to_name(err));
        return err;
    }
    if (esp_littlefs_info(conf.partition_label, &total, &used) == ESP_OK) {
        ESP_LOGI(TAG, "littlefs monté sur %s : %u / %u octets", DENGON_STORE_BASE_PATH,
                 (unsigned)used, (unsigned)total);
    }
    return ESP_OK;
}

static esp_err_t
get_blob(nvs_handle_t h, const char *key, uint8_t *out, size_t len)
{
    size_t    got = len;
    esp_err_t err = nvs_get_blob(h, key, out, &got);

    if (err == ESP_OK && got != len) {
        return ESP_ERR_INVALID_SIZE;
    }
    return err;
}

esp_err_t
dengon_store_load_secrets(uint8_t dh_secret[DENGON_SECRET_LEN],
                          uint8_t sign_seed[DENGON_SECRET_LEN], bool *created)
{
    nvs_handle_t h;
    esp_err_t    err = nvs_open(NVS_NAMESPACE, NVS_READWRITE, &h);

    *created = false;
    if (err != ESP_OK) {
        return err;
    }
    err = get_blob(h, KEY_DH, dh_secret, DENGON_SECRET_LEN);
    if (err == ESP_OK) {
        err = get_blob(h, KEY_SIGN, sign_seed, DENGON_SECRET_LEN);
    }
    if (err == ESP_ERR_NVS_NOT_FOUND) {
        /* Premier démarrage. Radio éteinte : sans source d'entropie active,
           esp_fill_random() ne serait qu'un PRNG. bootloader_random_enable()
           branche le bruit du SAR ADC, et DOIT être coupé avant d'allumer
           le Wi-Fi ou le BT (documentation ESP-IDF, « Random Number
           Generation »). */
        bootloader_random_enable();
        esp_fill_random(dh_secret, DENGON_SECRET_LEN);
        esp_fill_random(sign_seed, DENGON_SECRET_LEN);
        bootloader_random_disable();

        err = nvs_set_blob(h, KEY_DH, dh_secret, DENGON_SECRET_LEN);
        if (err == ESP_OK) {
            err = nvs_set_blob(h, KEY_SIGN, sign_seed, DENGON_SECRET_LEN);
        }
        if (err == ESP_OK) {
            err = nvs_commit(h);
        }
        *created = err == ESP_OK;
    }
    nvs_close(h);
    return err;
}

static esp_err_t
save_cursor(uint64_t next_seq, const uint8_t last_hash[DENGON_LEDGER_HASH_LEN])
{
    uint8_t      blob[CURSOR_LEN];
    nvs_handle_t h;
    esp_err_t    err;

    for (int i = 0; i < 8; i++) {
        blob[i] = (uint8_t)(next_seq >> (56 - 8 * i));
    }
    memcpy(blob + 8, last_hash, DENGON_LEDGER_HASH_LEN);
    err = nvs_open(NVS_NAMESPACE, NVS_READWRITE, &h);
    if (err != ESP_OK) {
        return err;
    }
    err = nvs_set_blob(h, KEY_CURSOR, blob, sizeof(blob));
    if (err == ESP_OK) {
        err = nvs_commit(h);
    }
    nvs_close(h);
    return err;
}

static esp_err_t
load_cursor(dengon_ledger_tail_t *cur)
{
    uint8_t      blob[CURSOR_LEN];
    nvs_handle_t h;
    esp_err_t    err = nvs_open(NVS_NAMESPACE, NVS_READONLY, &h);

    memset(cur, 0, sizeof(*cur));
    if (err != ESP_OK) {
        return err;
    }
    err = get_blob(h, KEY_CURSOR, blob, sizeof(blob));
    nvs_close(h);
    if (err != ESP_OK) {
        return err;
    }
    for (int i = 0; i < 8; i++) {
        cur->next_seq = (cur->next_seq << 8) | blob[i];
    }
    memcpy(cur->last_hash, blob + 8, DENGON_LEDGER_HASH_LEN);
    cur->has_entries = true;
    return ESP_OK;
}

esp_err_t
dengon_store_boot_anchor(dengon_ledger_tail_t *anchor, size_t *truncated)
{
    dengon_ledger_tail_t fichier;
    dengon_ledger_tail_t curseur;
    esp_err_t            err;

    if (dengon_ledger_file_recover(DENGON_LEDGER_PATH, &fichier, truncated) != 0) {
        ESP_LOGE(TAG, "relecture de %s impossible", DENGON_LEDGER_PATH);
        return ESP_FAIL;
    }
    err = load_cursor(&curseur);
    if (err != ESP_OK && err != ESP_ERR_NVS_NOT_FOUND) {
        return err;
    }

    if (fichier.has_entries) {
        /* Le fichier fait foi : il est écrit AVANT le curseur. Un curseur en
           retard = coupure entre les deux écritures, rattrapé ici. */
        if (curseur.has_entries && curseur.next_seq > fichier.next_seq) {
            ESP_LOGW(TAG, "curseur NVS (%llu) en avance sur %s (%llu) : fichier incomplet",
                     (unsigned long long)curseur.next_seq, DENGON_LEDGER_PATH,
                     (unsigned long long)fichier.next_seq);
        }
        *anchor = fichier;
    } else {
        /* Fichier vide : premier boot (genèse), ou juste après une rotation. */
        *anchor = curseur;
    }
    return anchor->has_entries ? save_cursor(anchor->next_seq, anchor->last_hash) : ESP_OK;
}

esp_err_t
dengon_store_append_ledger(const uint8_t *entries, size_t len, uint64_t next_seq,
                           const uint8_t last_hash[DENGON_LEDGER_HASH_LEN])
{
    int rot;

    if (dengon_ledger_file_append(DENGON_LEDGER_PATH, entries, len) != 0) {
        ESP_LOGE(TAG, "écriture de %s impossible", DENGON_LEDGER_PATH);
        return ESP_FAIL;
    }
    /* Curseur d'abord : juste après une rotation, `ledger.bin` est vide et
       c'est lui qui porte l'ancre au prochain boot. */
    if (save_cursor(next_seq, last_hash) != ESP_OK) {
        return ESP_FAIL;
    }
    rot = dengon_ledger_file_rotate(DENGON_LEDGER_PATH, DENGON_LEDGER_OLD_PATH,
                                    DENGON_LEDGER_FILE_MAX);
    if (rot < 0) {
        return ESP_FAIL;
    }
    if (rot > 0) {
        ESP_LOGI(TAG, "journal : rotation, reprise à seq=%llu", (unsigned long long)next_seq);
    }
    return ESP_OK;
}
