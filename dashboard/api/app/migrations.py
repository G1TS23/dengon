"""Migrations de la base du dashboard.

Chaque migration = ``(version, nom, [instructions SQL])``. Les instructions sont
des littéraux de ce module (pas un fichier lu à l'exécution) : versionnées dans
git, embarquées dans le paquet, sans dépendance au système de fichiers.

Une migration = **une liste d'instructions** (et non un script `;`-séparé) :
`run_migrations` les exécute une par une **dans une transaction unique**, ce que
`executescript` ne permet pas (il committe implicitement).

Chaque `CREATE` porte `IF NOT EXISTS` : si une exécution passée s'est arrêtée
entre le `CREATE` et l'enregistrement dans `schema_migrations` (ancien bug), la
reprise ne plante pas.

Ajouter une migration = ajouter un tuple à la fin de ``MIGRATIONS`` avec la
version suivante. On ne modifie jamais une migration déjà livrée.

Table `raw_batches` : zone d'atterrissage des batchs bruts (US-110), plus
utilisée en écriture depuis US-216 (`/ingest/batch` valide et route vers
`events` directement) — laissée en place, vide, plutôt que retirée par une
migration de suppression (risque inutile sur une table déjà livrée). Les
tables `events`/`nodes` (US-216) sont livrées ; `messages` (US-217) aussi,
mais **pas** `links`/`message_hops` — écart consigné dans
`03-ecarts-conception.md` (hors périmètre de l'US-217, qui ne couvre que la
reconstruction de statut). Réf : docs/synthese/09-dashboard-et-donnees.md
§3 et §11.2.
"""

from __future__ import annotations

_0001_INITIAL: list[str] = [
    """
    CREATE TABLE IF NOT EXISTS raw_batches (
        batch_id     TEXT    PRIMARY KEY,     -- uuid4 généré à la réception
        received_ms  INTEGER NOT NULL,        -- horodatage serveur, ms depuis epoch
        remote_addr  TEXT,                    -- IP source (debug uniquement)
        content_type TEXT,                    -- en-tête Content-Type reçu
        event_count  INTEGER,                 -- nb d'événements deviné, NULL si indéterminé
        body         TEXT    NOT NULL         -- corps JSON UTF-8, tel que reçu
    )
    """,
    "CREATE INDEX IF NOT EXISTS idx_raw_batches_received ON raw_batches (received_ms DESC)",
]

# Schéma repris tel quel de docs/synthese/09-dashboard-et-donnees.md §11.2
# (US-216 seulement `nodes`/`events` ; `messages`/`message_hops`/`links`
# sont pour l'instant absentes de cette migration, pas de table créée sans
# appelant avant leur US, US-217 — même discipline que crates/dengon-core
# ::store, US-207).
_0002_NODES_AND_EVENTS: list[str] = [
    """
    CREATE TABLE IF NOT EXISTS nodes (
        node_id      TEXT    PRIMARY KEY,        -- 'relay-3f2a9c' | 'client-…'
        kind         TEXT    NOT NULL CHECK (kind IN ('relay','client')),
        pub_sign     BLOB    NOT NULL,            -- clé publique Ed25519 (32 o)
        label        TEXT,
        fw_version   TEXT,
        first_seen   TEXT    NOT NULL DEFAULT (datetime('now')),
        last_seen    TEXT,
        status       TEXT    NOT NULL DEFAULT 'online'
                     CHECK (status IN ('online','stale','suspect','quarantined')),
        whitelisted  INTEGER NOT NULL DEFAULT 0
    )
    """,
    """
    CREATE TABLE IF NOT EXISTS events (
        event_id     TEXT    NOT NULL,
        ts_ms        INTEGER NOT NULL,
        ingested_ms  INTEGER NOT NULL DEFAULT (strftime('%s','now')*1000),
        node_id      TEXT    NOT NULL REFERENCES nodes(node_id),
        name         TEXT    NOT NULL,
        seq          INTEGER,
        entry_hash   BLOB,
        prev_hash    BLOB,
        integrity    TEXT    NOT NULL DEFAULT 'unverified'
                     CHECK (integrity IN ('ok','broken','fork','gap','unverified','rejected_sig')),
        payload      TEXT    NOT NULL,
        PRIMARY KEY (event_id)
    )
    """,
    "CREATE INDEX IF NOT EXISTS idx_events_name ON events (name, ts_ms DESC)",
    "CREATE INDEX IF NOT EXISTS idx_events_node ON events (node_id, ts_ms DESC)",
]

# Projection : statut par message (US-217), recalculée entièrement depuis
# `events` à chaque batch touchant son `msg_log_id` (app/projections.py) —
# jamais mise à jour incrémentale. `links`/`message_hops` de §11.2 ne sont
# pas créées ici : hors périmètre de l'US-217 (écart consigné).
_0003_MESSAGES: list[str] = [
    """
    CREATE TABLE IF NOT EXISTS messages (
        msg_log_id           TEXT    PRIMARY KEY,
        conv_hash            TEXT,
        first_seen_ms        INTEGER NOT NULL,
        last_event_ms        INTEGER NOT NULL,
        status               TEXT    NOT NULL DEFAULT 'unknown'
                              CHECK (status IN
                                  ('queued','in_flight','delivered','read','expired','unknown')),
        status_ms            INTEGER,
        hop_count            INTEGER NOT NULL DEFAULT 0,
        delivery_latency_ms  INTEGER
    )
    """,
    "CREATE INDEX IF NOT EXISTS idx_messages_status ON messages (status, status_ms DESC)",
]

# `sig` (US-310) : signature Ed25519 de CETTE entrée de journal chaîné
# (`ledger::Entry.sig`), distincte de `sig` au niveau batch (jamais stockée,
# vérifiée puis jetée par `ingest.py`). `entry_hash`/`prev_hash` existent
# déjà depuis la migration 2 mais n'étaient jusqu'ici jamais renseignés par
# `_insert_events` — app/integrity.py (US-310) en a besoin, avec `sig`, pour
# reconstruire l'export binaire `ledger::Entry::to_bytes()` par nœud et le
# passer à `dengon-verify`. Les trois restent NULL pour un événement qui ne
# les fournit pas (`envelope.schema.json` : optionnels) : cet événement
# n'entre alors simplement pas dans la vérification d'intégrité de son nœud.
_0004_ENTRY_SIGNATURE: list[str] = [
    "ALTER TABLE events ADD COLUMN sig BLOB",
]

# (version, nom, instructions) — ordre = ordre d'application.
MIGRATIONS: list[tuple[int, str, list[str]]] = [
    (1, "initial", _0001_INITIAL),
    (2, "nodes_and_events", _0002_NODES_AND_EVENTS),
    (3, "messages", _0003_MESSAGES),
    (4, "entry_signature", _0004_ENTRY_SIGNATURE),
]
