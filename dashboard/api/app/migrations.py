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

Table `raw_batches` : zone d'atterrissage des batchs bruts (US-110). La
validation de schéma + signature Ed25519 (US-216), la table `events` normalisée
et les projections `messages` / `nodes` / `links` / `message_hops` (US-217)
arrivent en sprint 2. Réf : docs/synthese/09-dashboard-et-donnees.md §3 et §11.2.
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

# (version, nom, instructions) — ordre = ordre d'application.
MIGRATIONS: list[tuple[int, str, list[str]]] = [
    (1, "initial", _0001_INITIAL),
]
