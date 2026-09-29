// ---------------------------------------------------------------------------
// dengon_ring.c — voir include/dengon_ring.h (US-309).
// ---------------------------------------------------------------------------
#include "dengon_ring.h"

#include <string.h>

static size_t
wrap(const dengon_ring_t *r, size_t pos)
{
    return pos >= r->cap ? pos - r->cap : pos;
}

/* Copie `len` octets vers le ring à partir de `pos`, en repliant. */
static void
copy_in(dengon_ring_t *r, size_t pos, const uint8_t *src, size_t len)
{
    size_t first = r->cap - pos;

    if (first > len) {
        first = len;
    }
    memcpy(r->buf + pos, src, first);
    memcpy(r->buf, src + first, len - first);
}

/* Copie `len` octets depuis le ring à partir de `pos`, en repliant. */
static void
copy_out(const dengon_ring_t *r, size_t pos, uint8_t *dst, size_t len)
{
    size_t first = r->cap - pos;

    if (first > len) {
        first = len;
    }
    memcpy(dst, r->buf + pos, first);
    memcpy(dst + first, r->buf, len - first);
}

static size_t
record_len(const dengon_ring_t *r, size_t pos)
{
    uint8_t p[DENGON_RING_PREFIX];

    copy_out(r, pos, p, sizeof(p));
    return (size_t)p[0] | (size_t)p[1] << 8 | (size_t)p[2] << 16 | (size_t)p[3] << 24;
}

/* Retire le plus ancien enregistrement. */
static void
pop_front(dengon_ring_t *r)
{
    size_t total = DENGON_RING_PREFIX + record_len(r, r->head);

    r->head = wrap(r, r->head + total);
    r->used -= total;
    r->count--;
    r->head_id++;
}

void
dengon_ring_init(dengon_ring_t *ring, uint8_t *buf, size_t cap)
{
    memset(ring, 0, sizeof(*ring));
    ring->buf = buf;
    ring->cap = cap;
}

bool
dengon_ring_push(dengon_ring_t *ring, const uint8_t *rec, size_t len)
{
    size_t  total = DENGON_RING_PREFIX + len;
    uint8_t p[DENGON_RING_PREFIX];

    if (len == 0 || total > ring->cap) {
        ring->dropped++;
        return false;
    }
    while (ring->cap - ring->used < total) {
        pop_front(ring);
        ring->dropped++;
    }
    p[0] = (uint8_t)len;
    p[1] = (uint8_t)(len >> 8);
    p[2] = (uint8_t)(len >> 16);
    p[3] = (uint8_t)(len >> 24);

    size_t tail = wrap(ring, ring->head + ring->used);
    copy_in(ring, tail, p, sizeof(p));
    copy_in(ring, wrap(ring, tail + DENGON_RING_PREFIX), rec, len);
    ring->used += total;
    ring->count++;
    return true;
}

size_t
dengon_ring_peek(const dengon_ring_t *ring, size_t max_records, uint8_t *out, size_t out_cap,
                 size_t *out_len, uint64_t *first_id)
{
    size_t pos = ring->head;
    size_t n   = 0;

    *out_len  = 0;
    *first_id = ring->head_id;
    while (n < max_records && n < ring->count) {
        size_t len = record_len(ring, pos);

        if (*out_len + len > out_cap) {
            if (n == 0) {
                *out_len = len;
            }
            break;
        }
        copy_out(ring, wrap(ring, pos + DENGON_RING_PREFIX), out + *out_len, len);
        *out_len += len;
        pos = wrap(ring, pos + DENGON_RING_PREFIX + len);
        n++;
    }
    return n;
}

size_t
dengon_ring_commit(dengon_ring_t *ring, uint64_t first_id, size_t n)
{
    uint64_t end     = first_id + n;
    size_t   removed = 0;

    /* Ceux d'avant head_id ont déjà été écrasés : rien à faire pour eux. */
    while (ring->count > 0 && ring->head_id >= first_id && ring->head_id < end) {
        pop_front(ring);
        removed++;
    }
    return removed;
}

unsigned
dengon_ring_fill_pct(const dengon_ring_t *ring)
{
    if (ring->cap == 0) {
        return 0;
    }
    return (unsigned)((ring->used * 100u + ring->cap - 1) / ring->cap);
}
