# Modèles de données & formats

## 1. Base locale client / nœud (`dengon-core::store`, SQLite)

Fichier `dengon.db` (WAL). Chiffré au repos (SQLCipher ou champ-par-champ, cf.
`04-security.md` §7).

```sql
-- identité (une seule ligne)
CREATE TABLE identity (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    peer_id         BLOB NOT NULL,          -- 8 o
    priv_static     BLOB NOT NULL,          -- X25519 (chiffré)
    priv_sign       BLOB NOT NULL,          -- Ed25519 (chiffré)
    pub_static      BLOB NOT NULL,
    pub_sign        BLOB NOT NULL,
    pseudo          TEXT NOT NULL,
    created_ms      INTEGER NOT NULL
);

-- contacts connus
CREATE TABLE contacts (
    peer_id         BLOB PRIMARY KEY,       -- 8 o
    pub_static      BLOB NOT NULL,
    pub_sign        BLOB NOT NULL,
    pseudo          TEXT,
    verified_at     INTEGER,               -- NULL = TOFU non vérifié
    first_seen_ms   INTEGER NOT NULL,
    last_seen_ms    INTEGER,
    key_changed_at  INTEGER,               -- alerte MITM si non NULL
    blocked         INTEGER NOT NULL DEFAULT 0
);

-- conversations (1-à-1 en MVP)
CREATE TABLE conversations (
    conv_id         BLOB PRIMARY KEY,      -- SHA-256(min(a,b)‖max(a,b))[:16]
    peer_id         BLOB NOT NULL REFERENCES contacts(peer_id),
    last_msg_ms     INTEGER,
    unread_count    INTEGER NOT NULL DEFAULT 0
);

-- messages (émis + reçus)
CREATE TABLE messages (
    msg_uuid        BLOB PRIMARY KEY,     -- 16 o, stable bout-en-bout
    conv_id         BLOB NOT NULL REFERENCES conversations(conv_id),
    direction       TEXT NOT NULL CHECK (direction IN ('out','in')),
    author_peer_id  BLOB NOT NULL,
    conv_seq        INTEGER NOT NULL,
    body            TEXT NOT NULL,        -- clair local uniquement
    sent_ms         INTEGER NOT NULL,
    received_ms     INTEGER,
    status          TEXT NOT NULL DEFAULT 'queued'
                    CHECK (status IN ('queued','in_flight','delivered','read','expired','cancelled')),
    status_ms       INTEGER NOT NULL,
    read_ms         INTEGER
);
CREATE INDEX idx_msg_conv ON messages(conv_id, sent_ms);

-- file d'envoi (paquets non confirmés)
CREATE TABLE outbox (
    msg_uuid        BLOB NOT NULL REFERENCES messages(msg_uuid),
    dest_peer_id    BLOB NOT NULL,
    packet          BLOB NOT NULL,        -- paquet L3 encodé, prêt à émettre
    kind            TEXT NOT NULL CHECK (kind IN ('session','envelope')),
    attempts        INTEGER NOT NULL DEFAULT 0,
    first_sent_ms   INTEGER,
    last_sent_ms    INTEGER,
    expires_ms      INTEGER NOT NULL,
    PRIMARY KEY (msg_uuid, dest_peer_id)
);

-- enveloppes scellées portées pour autrui (rôle relais / store-and-forward)
CREATE TABLE held_envelopes (
    msg_log_id      BLOB PRIMARY KEY,     -- SHA-256(msgID)[:16]
    recipient_tag   BLOB NOT NULL,        -- 16 o
    epoch_day       INTEGER NOT NULL,
    packet          BLOB NOT NULL,        -- SEALED_ENVELOPE complet
    copy_budget     INTEGER NOT NULL,
    deposit_ms      INTEGER NOT NULL,
    expires_ms      INTEGER NOT NULL
);
CREATE INDEX idx_env_tag ON held_envelopes(recipient_tag);

-- déduplication
CREATE TABLE seen_set (
    msg_id          BLOB PRIMARY KEY,     -- 32 o
    seen_ms         INTEGER NOT NULL
);

-- cache gossip (messages publics / paquets récents à re-servir)
CREATE TABLE gossip_cache (
    msg_id          BLOB PRIMARY KEY,
    packet          BLOB NOT NULL,
    cached_ms       INTEGER NOT NULL
);

-- sessions Noise (éphémère, peut être en mémoire ; persistée pour reprise rapide)
CREATE TABLE noise_sessions (
    peer_id         BLOB PRIMARY KEY,
    state           BLOB NOT NULL,        -- sérialisation snow (chiffrée)
    established_ms   INTEGER NOT NULL,
    tx_count        INTEGER NOT NULL DEFAULT 0
);

-- journal chaîné signé
CREATE TABLE ledger (
    seq             INTEGER PRIMARY KEY,  -- 0,1,2,… sans trou
    ts_ms           INTEGER NOT NULL,
    event_name      TEXT NOT NULL,
    payload_json    TEXT NOT NULL,        -- canonical JSON
    prev_hash       BLOB NOT NULL,        -- 32 o
    entry_hash      BLOB NOT NULL,        -- 32 o
    sig             BLOB NOT NULL         -- 64 o
);

-- curseur d'expédition vers le VPS (clients opt-in / relais)
CREATE TABLE ship_cursor (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    last_shipped_seq INTEGER NOT NULL DEFAULT -1
);
```

### Adaptation ESP32

Le relais n'a pas SQLite. `Store` y est implémenté ainsi :

| Table | Support ESP32 |
| --- | --- |
| `identity` | NVS chiffrée |
| `held_envelopes` | index en PSRAM + N plus récentes sérialisées en NVS/littlefs |
| `seen_set`, `gossip_cache` | RAM/PSRAM, volatile (reconstruit au boot) |
| `ledger` | append en littlefs (fichier séquentiel) + `seq`/`root` en NVS |
| `ship_cursor` | NVS |
| `messages`, `outbox`, `contacts`, … | **absents** (le relais ne fait pas de messagerie utilisateur) |

---

## 2. Base dashboard (PostgreSQL 16 + TimescaleDB)

```sql
CREATE TABLE nodes (
    node_id      TEXT PRIMARY KEY,          -- 'relay-3f2a9c' | 'client-…'
    kind         TEXT NOT NULL CHECK (kind IN ('relay','client')),
    pub_sign     BYTEA NOT NULL,
    label        TEXT,
    fw_version   TEXT,
    first_seen   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen    TIMESTAMPTZ,
    status       TEXT NOT NULL DEFAULT 'online'
                 CHECK (status IN ('online','stale','suspect','quarantined')),
    whitelisted  BOOLEAN NOT NULL DEFAULT false,
    client_cert_fp TEXT
);

CREATE TABLE events (
    event_id     UUID NOT NULL,
    ts           TIMESTAMPTZ NOT NULL,
    ingested_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    node_id      TEXT NOT NULL REFERENCES nodes(node_id),
    name         TEXT NOT NULL,
    seq          BIGINT,
    entry_hash   BYTEA,
    prev_hash    BYTEA,
    integrity    TEXT NOT NULL DEFAULT 'unverified'
                 CHECK (integrity IN ('ok','broken','fork','gap','unverified','rejected_sig')),
    payload      JSONB NOT NULL,
    PRIMARY KEY (event_id, ts)
);
SELECT create_hypertable('events', 'ts');
CREATE INDEX idx_events_name ON events (name, ts DESC);
CREATE INDEX idx_events_node ON events (node_id, ts DESC);
CREATE INDEX idx_events_msg  ON events ((payload->>'msg_log_id'), ts DESC);
CREATE INDEX idx_events_payload ON events USING gin (payload);
SELECT add_retention_policy('events', INTERVAL '90 days');

-- projection : état par message (entretenue par l'ingest)
CREATE TABLE messages (
    msg_log_id   TEXT PRIMARY KEY,          -- hex 16 o
    conv_hash    TEXT,
    first_seen   TIMESTAMPTZ NOT NULL,
    last_event   TIMESTAMPTZ NOT NULL,
    status       TEXT NOT NULL DEFAULT 'unknown'
                 CHECK (status IN ('queued','in_flight','delivered','read','expired','unknown')),
    status_at    TIMESTAMPTZ,
    hop_count    INT NOT NULL DEFAULT 0,
    delivery_latency_ms BIGINT
);

CREATE TABLE message_hops (
    msg_log_id   TEXT NOT NULL REFERENCES messages(msg_log_id),
    node_id      TEXT NOT NULL REFERENCES nodes(node_id),
    ts           TIMESTAMPTZ NOT NULL,
    ttl_in       INT, ttl_out INT, fanout INT, rssi INT,
    kind         TEXT,                      -- 'relay' | 'envelope_store' | 'envelope_handoff' | 'delivered'
    PRIMARY KEY (msg_log_id, node_id, ts)
);

CREATE TABLE links (
    a_node       TEXT NOT NULL REFERENCES nodes(node_id),
    b_node       TEXT NOT NULL REFERENCES nodes(node_id),
    last_seen    TIMESTAMPTZ NOT NULL,
    rssi_avg     INT,
    pkt_count    BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (a_node, b_node)
);

CREATE TABLE ledger_state (
    node_id      TEXT PRIMARY KEY REFERENCES nodes(node_id),
    height       BIGINT NOT NULL DEFAULT 0,
    root         BYTEA,
    integrity    TEXT NOT NULL DEFAULT 'unverified',
    checked_at   TIMESTAMPTZ
);

-- forks observés
CREATE TABLE ledger_forks (
    node_id      TEXT NOT NULL,
    height       BIGINT NOT NULL,
    root         BYTEA NOT NULL,
    reporters    TEXT[] NOT NULL,
    first_seen   TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (node_id, height, root)
);

CREATE TABLE operators (
    id           BIGSERIAL PRIMARY KEY,
    email        TEXT UNIQUE NOT NULL,
    pw_hash      TEXT NOT NULL,             -- argon2id
    totp_secret  TEXT,
    role         TEXT NOT NULL CHECK (role IN ('viewer','admin')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE alerts (
    id           BIGSERIAL PRIMARY KEY,
    kind         TEXT NOT NULL,
    node_id      TEXT,
    payload      JSONB NOT NULL,
    raised_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at  TIMESTAMPTZ
);
```

Continuous aggregates Timescale utiles : trafic/minute par nœud, taux de livraison
glissant, densité moyenne.

---

## 3. Formats de fils / échange

### 3.1 QR code de contact

```
dengon:v1:<base64url( B )>
B = pseudo_len:u8 ‖ pseudo:utf8 ‖ pub_static:32 ‖ pub_sign:32
```

### 3.2 Code de vérification

```
fpA = SHA-256(pub_static_A ‖ pub_sign_A)         (32 o)
fpB = idem pour B
material = SHA-512( min(fpA,fpB) ‖ max(fpA,fpB) )  (64 o)
pour i in 0..12 :
    g[i] = be_u16(material[2i..2i+2]) mod 100000
affichage : 12 groupes de 5 chiffres, zero-paddés
```

### 3.3 Entrée de journal (canonical JSON, avant hash)

```json
{"event":"envelope.stored","payload":{"copy_budget":4,"epoch_day":20340,"msg_log_id":"4d5e…","recipient_tag":"0011…"},"prev_hash":"<hex32>","seq":10433,"ts_ms":1725800000455}
```

Clés triées lexicographiquement, pas d'espace, entiers minimaux.
`entry_hash = SHA-256(bytes_utf8)` ; `sig = Ed25519(priv_sign, entry_hash)`.

### 3.4 Enveloppe scellée (payload de `SEALED_ENVELOPE`)

```
recipient_tag:16 ‖ epoch_day:u16 ‖ noise_x_ciphertext
  noise_x en clair : app_frame_len:u16 ‖ AppFrame ‖ sender_pub_static:32 ‖ sender_sig:64
  (sender_sig = Ed25519 sur SHA-256(AppFrame ‖ recipient_pub_static))
```

### 3.5 Batch MQTT

Voir `08-observability-events.md` §8.

---

## 4. Rétention & tailles indicatives

| Donnée | Où | Rétention | Ordre de grandeur |
| --- | --- | --- | --- |
| messages (clair) | client | jusqu'à suppression manuelle | ~200 o / msg |
| ledger | client / relais | roulement (garder N dernières ou X jours) | ~300 o / entrée |
| held_envelopes | relais / client porteur | 24 h + budget | ≤ 4 Ko / enveloppe |
| events | dashboard | 90 j (config) | ~400 o / event |
| message_hops | dashboard | suit `messages` | ~80 o / saut |
