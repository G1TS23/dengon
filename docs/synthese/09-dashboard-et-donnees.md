# dengon — Dashboard d'observabilité & modèles de données

> Fait partie du **contexte global** de `docs/synthese/` — point d'entrée :
> [`00-contexte-global.md`](00-contexte-global.md). Questions encore ouvertes :
> [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).
>
> **Stack retenue (A-5 / C-8)** : backend **FastAPI (Python)** + **SSE** +
> **SQLite** + page web légère ; la vérification de journal chaîné est déléguée
> au binaire Rust **`dengon-verify`** (sous-processus). Ingestion = **HTTPS POST
> par batch** (A-6). Base **effacée après chaque session de démo** (B-4).
> Le **catalogue d'événements** (`powl/08`, §9) reste la source de vérité.

---

## 1. Principe

Le dashboard **observe** le réseau. Il ne transporte aucun message, ne détient
aucune clé utilisateur, ne peut rien injecter. S'il tombe, la messagerie
continue à l'identique. Il reçoit des **événements de métadonnées signés** (des
relais, et des clients qui l'acceptent explicitement), les vérifie, les stocke,
les restitue en vues : parcours d'un message, santé du réseau, état de la
flotte, intégrité des journaux.

## 2. Architecture

| Composant | Techno | Rôle |
| --- | --- | --- |
| reverse-proxy | Caddy 2 ou nginx | TLS (Let's Encrypt), sert la page web |
| `api` | **Python + FastAPI** | endpoint HTTPS `POST /ingest/batch` + REST + **SSE** + reconstruction des projections |
| vérif de journal | **`dengon-verify`** (binaire Rust du workspace) | appelé en sous-processus par `api` : `verify_chain`, détection fork/gap/broken |
| `db` | **SQLite** (fichier, WAL) | événements + projections |
| `web` | page légère (vanilla ou petit framework) | UI opérateur, se rafraîchit via SSE |

Le binaire `dengon-verify` garantit **une seule source de vérité** pour la
logique de vérification (mêmes règles que `dengon-core::ledger`), sans imposer
un backend Rust complet.

> La stack `powl` d'origine (MQTT/Mosquitto → Axum (Rust) → PostgreSQL 16 +
> TimescaleDB → React, en Docker Compose 5 conteneurs) a été **écartée** pour le
> MVP : sur-dimensionnée pour une démo de 5-8 appareils dont les données sont
> effacées à chaque session, et lourde pour une équipe débutante en ~3 semaines.
> Arguments : fiche A-5 de
> [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).

## 3. Ingestion

**Source unique : HTTPS POST par batch** — `POST /ingest/batch`, corps = JSON
canonique (batch de N événements). Auth : `Authorization: Bearer <JWT court>`
(nœud en liste blanche) + **signature Ed25519** du batch (clé = `pub_sign` du
nœud). Volume : sporadique (fenêtres Wi-Fi des relais), faible pour une démo.

**Pipeline** : batch reçu → parse + schéma (rejet si malformé) → vérif signature
Ed25519 → dédup sur `event_id` (`SHA-256(node ‖ seq)`) → si event de journal :
appel `dengon-verify` (`prev_hash == hash(seq-1)` ?) → statut d'intégrité →
INSERT dans `events` → mise à jour des projections (`messages`, `nodes`,
`links`, `message_hops`) → push **SSE** aux opérateurs connectés (filtré).
Idempotent (rejouer un batch ne crée pas de doublon).

**Ce qui est refusé** : signature invalide → `events` avec
`integrity = rejected_sig`, alerte ; `node` inconnu (pas en liste blanche) →
quarantaine, alerte ; `msgID` non haché (format brut détecté) → **rejet strict**
(protection anti-fuite).

**Identité des nœuds** : chaque nœud a un `node_id` **pseudonyme stable** (un
hash, non rattachable à une personne — C-4). C'est ce qui permet de vérifier le
journal chaîné *par appareil* (A-7) et d'en déduire naturellement le compteur
d'appareils actifs (via `peer.*` / `relay.health`), sans battement séparé.

## 4. Modèle de données (résumé — schéma détaillé au §11)

Relations : `nodes ||--o{ events` (émet) ; `nodes ||--o{ ledger_state` ;
`messages ||--o{ events` (référencé par) ; `messages ||--o{ message_hops` ;
`nodes ||--o{ links`. `events` est la table brute (append) ; les autres tables
sont des **projections** entretenues par l'ingest (requêtes rapides),
reconstructibles en rejouant `events`.

## 5. Écrans

- **Parcours d'un message** : entrée = un `msg_log_id` (l'app peut afficher un
  QR/lien « suivre ce message » qui contient déjà le hash) **ou** recherche par
  fenêtre de temps + `node_id` de relais. Affiche : **timeline verticale**
  (chaque saut de `message_hops` avec nœud, horodatage, TTL in/out, fanout,
  RSSI) ; **statut courant** reconstruit avec l'horodatage de chaque transition
  et la **source** de l'info ; badges d'intégrité.
- **Carte / santé du réseau** : graphe des `nodes` + `links` ; couleur = statut
  (`online`/`stale`/`suspect`) ; épaisseur d'arête = trafic ; panneau latéral =
  densité moyenne, latence de relais médiane, taux de livraison, nb
  d'enveloppes en circulation (estimé).
- **Flotte de relais** : tableau `node_id`, label, version fw, uptime, RSSI
  moyen, tailles de buffers, `logs_dropped`, dernière remontée. Alertes : relais
  muet > 5 min, buffer > 90 %, version obsolète.
- **Intégrité des journaux** : par nœud — hauteur, racine courante, dernier
  contrôle, verdict (`ok` / `broken` / `fork` / `gap` / `unverified`). Vue
  « diff » quand un fork est détecté.
- **Recherche de logs** : filtre sur `events` (name, node, payload), fenêtre
  temporelle, export CSV/JSON.

**Alerting** (règles configurables → notif webhook / e-mail) :

| Alerte | Condition |
| --- | --- |
| `relay_silent` | pas de `relay.health` d'un relais depuis 5 min |
| `chain_broken` | `events.integrity = broken` |
| `fork_detected` | 2 racines pour (node, height) |
| `rejected_sig_spike` | > N signatures invalides / min |
| `delivery_drop` | taux de livraison < seuil sur 15 min |
| `clock_skew` | dérive d'horloge d'un nœud > 2 min |

## 6. API (FastAPI) — principales routes

| Méthode | Route | Rôle |
| --- | --- | --- |
| `POST` | `/ingest/batch` | ingestion des nœuds (JWT + signature) |
| `GET` | `/api/messages/:msg_log_id` | parcours + statut + sauts |
| `GET` | `/api/messages?since=&node=&status=` | liste filtrée |
| `GET` | `/api/network/graph` | nœuds + liens (snapshot) |
| `GET` | `/api/nodes` / `/api/nodes/:id` | flotte |
| `GET` | `/api/integrity` | verdicts par nœud (`dengon-verify`) |
| `GET` | `/api/events?…` | recherche de logs |
| `GET` | `/api/stream` | **SSE** — flux temps réel (events, transitions de statut, alertes) |
| `POST` | `/api/nodes` (admin) | enregistrer un relais (pub_sign) |
| `GET` | `/healthz` | liveness |

Auth opérateur : session (cookie httponly + CSRF) ; rôles `viewer` / `admin`.

## 7. Déploiement sur le VPS

`dashboard/` = l'app FastAPI (`api/`) + la page (`web/`) + un fichier de
migration SQLite + `.env.example`. Étapes : (1) créer la base SQLite (script de
migration) ; (2) lancer `uvicorn` (ou `gunicorn` + workers) derrière le
reverse-proxy ; (3) le reverse-proxy obtient le certificat TLS pour
`dashboard.<domaine>` et sert `web/` ; (4) enregistrer chaque relais via
`POST /api/nodes` et lui remettre un JWT ; (5) créer le compte opérateur admin.
Ressources : très léger (< 256 Mo RAM). **Purge** : un script remet la base à
zéro entre deux sessions de démo (B-4).

## 8. Confidentialité — garanties

**Aucun** contenu de message, **aucun** `msg_uuid`, **aucun** `recipient`
identifiable ne peut arriver ni être stocké (rejet au schéma) ; `msgID` toujours
**haché** (`SHA-256(msgID)[:16]`) ; les pseudos n'apparaissent que si un nœud les
publie volontairement (opt-in) ; base effacée après chaque session (B-4) ; le
dashboard est **read-only vis-à-vis du terrain**.

## 9. Catalogue normalisé des événements

> Source de vérité : `powl/08-observability-events.md`. Le catalogue vit dans
> `crates/dengon-core/src/observability/catalog.rs` — ce document doit rester
> synchronisé avec lui.

**Règles générales** : nommage `<domaine>.<action>` en `snake_case`. Enveloppe
commune (tout événement) :

```json
{
  "event_id": "hex(SHA-256(node_id ‖ seq))",
  "node_id": "relay-3f2a… | client-9c1d…",
  "node_kind": "relay | client",
  "seq": 10432,
  "ts_ms": 1725800000123,
  "name": "pkt.relayed",
  "payload": { }
}
```

Le nœud signe `canonical_json(enveloppe_sans_sig)` avec sa clé `sign` ; `sig`
(base64) ajouté **hors** du JSON canonique, dans le batch HTTPS. Les événements
sont **aussi** des entrées de journal ; le dashboard recalcule `prev_hash` via
le mécanisme `ledger` (`dengon-verify`). **Redaction obligatoire avant
émission** : `msgID` → `msg_log_id = hex(SHA-256(msgID)[0..16])` ; jamais de
`msg_uuid`, jamais de texte, jamais de `recipient` en clair ; `peerID` de pairs
gardés (pseudonymes) mais tronqués à 8 o ; `recipient_tag` autorisé (déjà
anonyme et tournant). Batching : N événements par POST ; `event_id` déduplique
côté VPS. Versionnement : champ `schema_version` (entier) dans l'enveloppe du
batch.

**Domaine `pkt` — trafic de paquets**

| `name` | Producteur | `payload` | Sens |
| --- | --- | --- | --- |
| `pkt.seen` | relais, client | `{ msg_log_id, type, ttl_in, size_bucket, from_peer, rssi }` | paquet reçu (avant dédup) |
| `pkt.duplicate` | relais, client | `{ msg_log_id, from_peer }` | déjà dans le seen-set |
| `pkt.relayed` | relais, client | `{ msg_log_id, type, ttl_in, ttl_out, fanout, from_peer }` | rediffusé |
| `pkt.delivered_local` | client | `{ msg_log_id, type }` | paquet pour moi, traité |
| `pkt.dropped` | relais, client | `{ msg_log_id?, reason }` | `reason ∈ {ttl_zero, relay_not_ok, queue_full, quota, too_old}` |
| `pkt.rejected` | relais, client | `{ from_peer, reason }` | `reason ∈ {bad_sig, bad_version, malformed, peerid_mismatch}` |

`size_bucket` ∈ `{256,512,1024,2048}` (jamais la taille exacte).

**Domaine `envelope` — enveloppes scellées**

| `name` | Producteur | `payload` | Sens |
| --- | --- | --- | --- |
| `envelope.stored` | relais, client | `{ msg_log_id, recipient_tag, epoch_day, copy_budget }` | enveloppe acceptée en dépôt |
| `envelope.offered` | relais, client | `{ count, to_peer }` | `ENVELOPE_OFFER` envoyé |
| `envelope.handoff` | relais, client | `{ msg_log_id, recipient_tag, to_peer, budget_after }` | copie transmise à un autre porteur |
| `envelope.delivered` | relais, client | `{ msg_log_id, recipient_tag, to_peer }` | remise à un pair dont le tag matche (probable destinataire) |
| `envelope.expired` | relais, client | `{ msg_log_id, recipient_tag, reason }` | `reason ∈ {ttl, budget_zero, store_full}` |

**Domaine `msg` — cycle de vie (clients opt-in surtout)**

| `name` | Producteur | `payload` | Statut résultant |
| --- | --- | --- | --- |
| `msg.queued` | client émetteur | `{ msg_log_id, conv_hash }` | `queued` |
| `msg.handed_off` | client émetteur | `{ msg_log_id, to_peer, via }` (`via ∈ {session, envelope}`) | `in_flight` |
| `msg.received` | client destinataire | `{ msg_log_id, via }` | (contribue à `delivered`) |
| `msg.delivered` | client émetteur | `{ msg_log_id, latency_ms }` | `delivered` |
| `msg.read` | client émetteur | `{ msg_log_id, latency_ms }` | `read` *(v2)* |
| `msg.expired` | client émetteur | `{ msg_log_id }` | `expired` |
| `ack.observed` | relais | `{ msg_log_id }` | indice de `delivered` |
| `read.observed` | relais | `{ msg_log_id }` | indice de `read` *(v2)* |

`conv_hash` = `SHA-256(min(peerA,peerB) ‖ max(peerA,peerB))[0..8]` (grouper une
conversation **sans** savoir qui elle implique). Reconstruction du statut côté
dashboard : priorité aux événements de l'émetteur (`msg.*`) ; à défaut, inférence
best-effort depuis les événements relais ; statut `unknown` si aucune donnée.

**Domaine `peer` / `link` — topologie**

| `name` | Producteur | `payload` |
| --- | --- | --- |
| `peer.connected` | relais, client | `{ peer, rssi, role }` (`role ∈ {central, peripheral}`) |
| `peer.disconnected` | relais, client | `{ peer, duration_s, pkt_exchanged }` |
| `peer.announce_seen` | relais, client | `{ peer, pseudo?, ledger_height, caps }` |

Le dashboard dérive `links` de la corrélation `peer.connected`/`disconnected`
entre deux `node_id` connus.

**Domaine `attest` / `integrity` — journal chaîné**

| `name` | Producteur | `payload` | Sens |
| --- | --- | --- | --- |
| `attest.emitted` | tout nœud | `{ root, height }` | ma racine de journal courante |
| `attest.observed` | relais, client | `{ subject_node, root, height }` | racine d'un **autre** nœud, vue via `LOG_ATTEST` |
| `integrity.chain_broken` | **dashboard** (dérivé, `dengon-verify`) | `{ node, seq, expected_prev, got_prev }` | `prev_hash` incohérent |
| `integrity.fork_detected` | **dashboard** (dérivé) | `{ node, height, roots: [..], reporters: [..] }` | 2 racines pour une hauteur |
| `integrity.gap` | **dashboard** (dérivé) | `{ node, from_seq, to_seq }` | trou dans `seq` |

`integrity.*` ne viennent pas du terrain : ce sont des conclusions de l'ingest.

**Domaine `relay` / `node` — santé**

| `name` | Producteur | `payload` |
| --- | --- | --- |
| `relay.boot` | relais | `{ fw_version, reset_reason, secure_boot, flash_enc }` |
| `relay.health` | relais (60 s) | `{ uptime_s, rssi_avg, peers, cache_size, envelope_store, log_buffer_pct, logs_dropped, heap_free }` |
| `relay.wifi_up` / `relay.wifi_down` | relais | `{ ssid?, duration_s? }` |
| `relay.overloaded` | relais | `{ subsystem, action }` (`action ∈ {drop_pkt, refuse_envelope, evict_cache}`) |
| `node.clock_skew` | **dashboard** (dérivé) | `{ node, skew_ms }` |
| `client.summary` | client opt-in (périodique) | `{ msgs_sent, msgs_recv, peers_seen, app_version }` (agrégats anonymes) |

**Exemple de batch HTTPS POST** (`POST /ingest/batch`)

```json
{
  "batch_id": "b1f0…",
  "node_id": "relay-3f2a9c",
  "schema_version": 1,
  "events": [
    { "event_id": "9a1c…", "node_id": "relay-3f2a9c", "node_kind": "relay",
      "seq": 10432, "ts_ms": 1725800000123, "name": "pkt.relayed",
      "payload": { "msg_log_id": "4d5e6f7a8b9c0d1e", "type": 4,
                   "ttl_in": 6, "ttl_out": 5, "fanout": 2,
                   "from_peer": "a1b2c3d4e5f60718" } },
    { "event_id": "9a1d…", "node_id": "relay-3f2a9c", "node_kind": "relay",
      "seq": 10433, "ts_ms": 1725800000455, "name": "envelope.stored",
      "payload": { "msg_log_id": "4d5e6f7a8b9c0d1e",
                   "recipient_tag": "00112233445566778899aabbccddeeff",
                   "epoch_day": 20340, "copy_budget": 4 } }
  ],
  "sig": "base64(ed25519(canonical_json(batch_without_sig)))"
}
```

## 10. Reconstruction du statut côté serveur

Table de déduction (formulée dans `olivier/dashboard` v0.2, retenue) :

| Statut affiché | Règle |
| --- | --- |
| **En circulation** | Au moins un `msg.queued` / `msg.handed_off` / `pkt.relayed`, ni `delivered` ni `expired`. |
| **Distribué** | Un `msg.delivered` (émetteur) **ou** `ack.observed` (relais). |
| **Expiré** | Un `msg.expired` reçu, et pas de `delivered`. |
| **Inconnu / partiel** | Aucun événement reçu pour ce message. |

Le serveur doit être **tolérant** : événements en retard, dans le désordre, en
double, ou jamais.

> `olivier/dashboard` v0.2 proposait un **code anonyme différent par message**
> (au lieu d'un `node_id` stable). **Non retenu** : A-7 (journal chaîné signé par
> appareil) impose une identité de nœud stable. Les maquettes d'écrans d'origine
> sont conservées pour mémoire en
> [`11-glossaire-biblio-annexes.md`](11-glossaire-biblio-annexes.md) (annexe B).

---

## 11. Modèles de données

> Source : `powl/09-data-model.md`, adapté (dashboard : SQLite au lieu de
> PostgreSQL/TimescaleDB).

### 11.1 Base locale client / nœud (`dengon-core::store`, SQLite)

Fichier `dengon.db` (WAL). Colonnes sensibles chiffrées **XChaCha20-Poly1305
champ par champ** (B-3) ; clés privées de préférence dans Keystore/Keychain.

```sql
CREATE TABLE identity (            -- une seule ligne
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    peer_id         BLOB NOT NULL,          -- 8 o
    priv_static     BLOB NOT NULL,          -- X25519 (chiffré / coffre plateforme)
    priv_sign       BLOB NOT NULL,          -- Ed25519 (chiffré / coffre plateforme)
    pub_static      BLOB NOT NULL,
    pub_sign        BLOB NOT NULL,
    pseudo          TEXT NOT NULL,
    created_ms      INTEGER NOT NULL
);

CREATE TABLE contacts (
    peer_id         BLOB PRIMARY KEY,       -- 8 o
    pub_static      BLOB NOT NULL,
    pub_sign        BLOB NOT NULL,
    pseudo          TEXT,
    verified_at     INTEGER,               -- NULL = TOFU non vérifié
    first_seen_ms   INTEGER NOT NULL,
    last_seen_ms    INTEGER,
    key_changed_at  INTEGER,               -- alerte MITM si non NULL
    blocked         INTEGER NOT NULL DEFAULT 0   -- UI reportée après v1
);

CREATE TABLE conversations (
    conv_id         BLOB PRIMARY KEY,      -- SHA-256(min(a,b)‖max(a,b))[:16]
    peer_id         BLOB NOT NULL REFERENCES contacts(peer_id),
    last_msg_ms     INTEGER,
    unread_count    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE messages (
    msg_uuid        BLOB PRIMARY KEY,     -- 16 o, stable bout-en-bout
    conv_id         BLOB NOT NULL REFERENCES conversations(conv_id),
    direction       TEXT NOT NULL CHECK (direction IN ('out','in')),
    author_peer_id  BLOB NOT NULL,
    conv_seq        INTEGER NOT NULL,
    body            BLOB NOT NULL,        -- texte chiffré champ par champ (XChaCha20)
    sent_ms         INTEGER NOT NULL,
    received_ms     INTEGER,
    status          TEXT NOT NULL DEFAULT 'queued'
                    CHECK (status IN ('queued','in_flight','delivered','read','expired','cancelled')),
    status_ms       INTEGER NOT NULL,
    read_ms         INTEGER              -- v2
);
CREATE INDEX idx_msg_conv ON messages(conv_id, sent_ms);

CREATE TABLE outbox (               -- paquets non confirmés
    msg_uuid        BLOB NOT NULL REFERENCES messages(msg_uuid),
    dest_peer_id    BLOB NOT NULL,
    packet          BLOB NOT NULL,        -- paquet L3 encodé
    kind            TEXT NOT NULL CHECK (kind IN ('session','envelope')),
    attempts        INTEGER NOT NULL DEFAULT 0,
    first_sent_ms   INTEGER,
    last_sent_ms    INTEGER,
    expires_ms      INTEGER NOT NULL,
    PRIMARY KEY (msg_uuid, dest_peer_id)
);

CREATE TABLE held_envelopes (      -- enveloppes portées pour autrui
    msg_log_id      BLOB PRIMARY KEY,     -- SHA-256(msgID)[:16]
    recipient_tag   BLOB NOT NULL,        -- 16 o
    epoch_day       INTEGER NOT NULL,
    packet          BLOB NOT NULL,        -- SEALED_ENVELOPE complet
    copy_budget     INTEGER NOT NULL,
    deposit_ms      INTEGER NOT NULL,
    expires_ms      INTEGER NOT NULL
);
CREATE INDEX idx_env_tag ON held_envelopes(recipient_tag);

CREATE TABLE seen_set (   msg_id BLOB PRIMARY KEY, seen_ms INTEGER NOT NULL );      -- 32 o
CREATE TABLE recon_cache ( msg_id BLOB PRIMARY KEY, packet BLOB NOT NULL, cached_ms INTEGER NOT NULL );

CREATE TABLE noise_sessions (
    peer_id         BLOB PRIMARY KEY,
    state           BLOB NOT NULL,        -- sérialisation snow (chiffrée)
    established_ms   INTEGER NOT NULL,
    tx_count        INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE ledger (              -- journal chaîné signé
    seq             INTEGER PRIMARY KEY,  -- 0,1,2,… sans trou
    ts_ms           INTEGER NOT NULL,
    event_name      TEXT NOT NULL,
    payload_json    TEXT NOT NULL,        -- canonical JSON
    prev_hash       BLOB NOT NULL,        -- 32 o
    entry_hash      BLOB NOT NULL,        -- 32 o
    sig             BLOB NOT NULL         -- 64 o
);

CREATE TABLE ship_cursor ( id INTEGER PRIMARY KEY CHECK (id = 1), last_shipped_seq INTEGER NOT NULL DEFAULT -1 );
```

**Adaptation ESP32** : `identity` → NVS chiffrée ; `held_envelopes` → index en
SRAM + N plus récentes sérialisées en NVS/littlefs ; `seen_set`, `recon_cache`
→ SRAM, volatile (reconstruit au boot) ; `ledger` → append en littlefs (fichier
séquentiel) + `seq`/`root` en NVS ; `ship_cursor` → NVS ; `messages`, `outbox`,
`contacts`, … → **absents** (le relais ne fait pas de messagerie utilisateur).

### 11.2 Base dashboard (SQLite)

```sql
CREATE TABLE nodes (
    node_id      TEXT PRIMARY KEY,          -- 'relay-3f2a9c' | 'client-…' (pseudonyme stable)
    kind         TEXT NOT NULL CHECK (kind IN ('relay','client')),
    pub_sign     BLOB NOT NULL,
    label        TEXT,
    fw_version   TEXT,
    first_seen   TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen    TEXT,
    status       TEXT NOT NULL DEFAULT 'online'
                 CHECK (status IN ('online','stale','suspect','quarantined')),   -- D-5
    whitelisted  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE events (
    event_id     TEXT NOT NULL,
    ts_ms        INTEGER NOT NULL,
    ingested_ms  INTEGER NOT NULL DEFAULT (strftime('%s','now')*1000),
    node_id      TEXT NOT NULL REFERENCES nodes(node_id),
    name         TEXT NOT NULL,
    seq          INTEGER,
    entry_hash   BLOB,
    prev_hash    BLOB,
    integrity    TEXT NOT NULL DEFAULT 'unverified'
                 CHECK (integrity IN ('ok','broken','fork','gap','unverified','rejected_sig')),  -- D-4
    payload      TEXT NOT NULL,             -- JSON
    PRIMARY KEY (event_id)
);
CREATE INDEX idx_events_name ON events (name, ts_ms DESC);
CREATE INDEX idx_events_node ON events (node_id, ts_ms DESC);
CREATE INDEX idx_events_msg  ON events (json_extract(payload,'$.msg_log_id'), ts_ms DESC);

CREATE TABLE messages (           -- projection : état par message
    msg_log_id   TEXT PRIMARY KEY,          -- hex 16 o
    conv_hash    TEXT,
    first_seen_ms INTEGER NOT NULL,
    last_event_ms INTEGER NOT NULL,
    status       TEXT NOT NULL DEFAULT 'unknown'
                 CHECK (status IN ('queued','in_flight','delivered','read','expired','unknown')),
    status_ms    INTEGER,
    hop_count    INTEGER NOT NULL DEFAULT 0,
    delivery_latency_ms INTEGER
);

CREATE TABLE message_hops (
    msg_log_id   TEXT NOT NULL REFERENCES messages(msg_log_id),
    node_id      TEXT NOT NULL REFERENCES nodes(node_id),
    ts_ms        INTEGER NOT NULL,
    ttl_in       INTEGER, ttl_out INTEGER, fanout INTEGER, rssi INTEGER,
    kind         TEXT,   -- 'relay' | 'envelope_store' | 'envelope_handoff' | 'delivered'
    PRIMARY KEY (msg_log_id, node_id, ts_ms)
);

CREATE TABLE links (
    a_node       TEXT NOT NULL REFERENCES nodes(node_id),
    b_node       TEXT NOT NULL REFERENCES nodes(node_id),
    last_seen_ms INTEGER NOT NULL,
    rssi_avg     INTEGER,
    pkt_count    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (a_node, b_node)
);

CREATE TABLE ledger_state (
    node_id      TEXT PRIMARY KEY REFERENCES nodes(node_id),
    height       INTEGER NOT NULL DEFAULT 0,
    root         BLOB,
    integrity    TEXT NOT NULL DEFAULT 'unverified',
    checked_ms   INTEGER
);

CREATE TABLE ledger_forks (
    node_id      TEXT NOT NULL,
    height       INTEGER NOT NULL,
    root         BLOB NOT NULL,
    reporters    TEXT NOT NULL,          -- JSON array
    first_seen_ms INTEGER NOT NULL,
    PRIMARY KEY (node_id, height, root)
);

CREATE TABLE operators (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    email        TEXT UNIQUE NOT NULL,
    pw_hash      TEXT NOT NULL,             -- argon2id
    totp_secret  TEXT,
    role         TEXT NOT NULL CHECK (role IN ('viewer','admin')),
    created_ms   INTEGER NOT NULL
);

CREATE TABLE alerts (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    kind         TEXT NOT NULL,
    node_id      TEXT,
    payload      TEXT NOT NULL,             -- JSON
    raised_ms    INTEGER NOT NULL,
    resolved_ms  INTEGER
);
```

Pas d'hypertable ni de politique de rétention automatique (SQLite) : la base est
**remise à zéro entre sessions de démo** (B-4). Agrégats (trafic/minute par
nœud, taux de livraison glissant) calculés à la volée par des requêtes.

### 11.3 Formats de fils / échange

**QR code de contact** : `dengon:v1:<base64url( B )>` avec
`B = pseudo_len:u8 ‖ pseudo:utf8 ‖ pub_static:32 ‖ pub_sign:32`.

**Code de vérification** :

```text
fpA = SHA-256(pub_static_A ‖ pub_sign_A)         (32 o)   ;  fpB = idem pour B
material = SHA-512( min(fpA,fpB) ‖ max(fpA,fpB) )  (64 o)
pour i in 0..12 : g[i] = be_u16(material[2i..2i+2]) mod 100000
affichage : 12 groupes de 5 chiffres, zero-paddés
```

**Entrée de journal (canonical JSON, avant hash)** :

```json
{"event":"envelope.stored","payload":{"copy_budget":4,"epoch_day":20340,"msg_log_id":"4d5e…","recipient_tag":"0011…"},"prev_hash":"<hex32>","seq":10433,"ts_ms":1725800000455}
```

Clés triées lexicographiquement, pas d'espace, entiers minimaux.
`entry_hash = SHA-256(bytes_utf8)` ; `sig = Ed25519(priv_sign, entry_hash)`.

**Enveloppe scellée (payload de `SEALED_ENVELOPE`)** :

```text
recipient_tag:16 ‖ epoch_day:u16 ‖ noise_x_ciphertext
  noise_x en clair : app_frame_len:u16 ‖ AppFrame ‖ sender_pub_static:32 ‖ sender_sig:64
  (sender_sig = Ed25519 sur SHA-256(AppFrame ‖ recipient_pub_static))
```

### 11.4 Rétention & tailles indicatives

| Donnée | Où | Rétention | Ordre de grandeur |
| --- | --- | --- | --- |
| messages (clair) | client | jusqu'à suppression manuelle | ~200 o / msg |
| ledger | client / relais | roulement (garder N dernières ou X jours) | ~300 o / entrée |
| held_envelopes | relais / client porteur | 24 h (`MSG_TTL_S`) | ≤ 4 Ko / enveloppe |
| events | dashboard | **effacé après chaque session de démo** (B-4) | ~400 o / event |
| message_hops | dashboard | suit `messages` | ~80 o / saut |
