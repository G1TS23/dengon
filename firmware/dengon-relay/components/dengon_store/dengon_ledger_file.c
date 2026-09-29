// ---------------------------------------------------------------------------
// dengon_ledger_file.c — voir dengon_ledger_file.h (US-308).
// ---------------------------------------------------------------------------
#include "dengon_ledger_file.h"

#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static uint32_t
be32(const uint8_t *p)
{
    return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) | ((uint32_t)p[2] << 8) | p[3];
}

static uint64_t
be64(const uint8_t *p)
{
    return ((uint64_t)be32(p) << 32) | be32(p + 4);
}

size_t
dengon_ledger_entry_len(const uint8_t *buf, size_t avail)
{
    size_t   pos = 16;
    uint32_t name_len;
    uint32_t payload_len;

    if (avail < pos + 4) {
        return 0;
    }
    name_len = be32(buf + pos);
    if (name_len > DENGON_LEDGER_FIELD_MAX) {
        return 0;
    }
    pos += 4 + name_len;
    if (avail < pos + 4) {
        return 0;
    }
    payload_len = be32(buf + pos);
    if (payload_len > DENGON_LEDGER_FIELD_MAX) {
        return 0;
    }
    pos += 4 + payload_len + 32 + 32 + 64;
    return avail < pos ? 0 : pos;
}

/* Lit exactement `n` octets ; `false` en fin de fichier ou sur erreur. */
static bool
read_exact(FILE *f, uint8_t *buf, size_t n)
{
    return n == 0 || fread(buf, 1, n, f) == n;
}

int
dengon_ledger_file_recover(const char *path, dengon_ledger_tail_t *tail, size_t *truncated)
{
    /* Une entrée tient toujours dans ce tampon : partie fixe + deux champs
       bornés par DENGON_LEDGER_FIELD_MAX. Statique : 8 Ko de pile en moins
       pour la tâche appelante (appel unique, au boot). */
    static uint8_t buf[DENGON_LEDGER_FIXED_LEN + 2 * DENGON_LEDGER_FIELD_MAX];
    struct stat    st;
    FILE          *f;
    long           good = 0;

    memset(tail, 0, sizeof(*tail));
    if (truncated != NULL) {
        *truncated = 0;
    }
    if (stat(path, &st) != 0) {
        return errno == ENOENT ? 0 : -1;
    }
    f = fopen(path, "rb");
    if (f == NULL) {
        return -1;
    }
    for (;;) {
        uint32_t name_len;
        uint32_t payload_len;
        size_t   len;

        /* seq ‖ ts ‖ name_len */
        if (!read_exact(f, buf, 20)) {
            break;
        }
        name_len = be32(buf + 16);
        if (name_len > DENGON_LEDGER_FIELD_MAX || !read_exact(f, buf + 20, name_len + 4)) {
            break;
        }
        payload_len = be32(buf + 20 + name_len);
        if (payload_len > DENGON_LEDGER_FIELD_MAX) {
            break;
        }
        len = 24 + name_len + payload_len + 32 + 32 + 64;
        if (!read_exact(f, buf + 24 + name_len, len - 24 - name_len)) {
            break;
        }
        /* Entrée complète : l'ancre avance. */
        tail->has_entries = true;
        tail->next_seq    = be64(buf) + 1;
        memcpy(tail->last_hash, buf + len - 64 - 32, DENGON_LEDGER_HASH_LEN);
        good += (long)len;
    }
    fclose(f);

    if ((long)st.st_size > good) {
        if (truncated != NULL) {
            *truncated = (size_t)((long)st.st_size - good);
        }
        if (truncate(path, good) != 0) {
            return -1;
        }
    }
    return 0;
}

int
dengon_ledger_file_append(const char *path, const uint8_t *bytes, size_t len)
{
    FILE *f;
    int   rc = 0;

    if (len == 0) {
        return 0;
    }
    f = fopen(path, "ab");
    if (f == NULL) {
        return -1;
    }
    if (fwrite(bytes, 1, len, f) != len || fflush(f) != 0 || fsync(fileno(f)) != 0) {
        rc = -1;
    }
    if (fclose(f) != 0) {
        rc = -1;
    }
    return rc;
}

int
dengon_ledger_file_rotate(const char *path, const char *old_path, size_t max_bytes)
{
    struct stat st;

    if (stat(path, &st) != 0) {
        return errno == ENOENT ? 0 : -1;
    }
    if ((size_t)st.st_size <= max_bytes) {
        return 0;
    }
    if (unlink(old_path) != 0 && errno != ENOENT) {
        return -1;
    }
    return rename(path, old_path) == 0 ? 1 : -1;
}
