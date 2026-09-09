"""Migrations de la base du dashboard.

Chaque migration = ``(version, nom, sql)``. Le SQL est un littéral de ce module
(pas un fichier lu à l'exécution) : versionné dans git, embarqué dans le paquet,
et sans dépendance au système de fichiers au déploiement.

Ajouter une migration = ajouter un tuple à la fin de ``MIGRATIONS``, avec la
version suivante. On ne modifie jamais une migration déjà livrée.
"""

from __future__ import annotations

_0001_INITIAL = """
-- Squelette volontairement minimal (US-110) : une seule table d'atterrissage
-- pour les batchs bruts. Aucune validation, aucune normalisation.
--
-- La validation de schéma + signature Ed25519 (US-216), la table `events`
-- normalisée et les projections `messages` / `nodes` / `links` / `message_hops`
-- (US-217) arrivent en sprint 2.
-- Réf : docs/synthese/09-dashboard-et-donnees.md §3 et §11.2.

CREATE TABLE raw_batches (
    batch_id     TEXT    PRIMARY KEY,     -- uuid4 généré à la réception
    received_ms  INTEGER NOT NULL,        -- horodatage serveur, ms depuis epoch
    remote_addr  TEXT,                    -- IP source (debug uniquement)
    content_type TEXT,                    -- en-tête Content-Type reçu
    event_count  INTEGER,                 -- nb d'événements deviné, NULL si indéterminé
    body         TEXT    NOT NULL         -- corps JSON, tel que reçu
);

CREATE INDEX idx_raw_batches_received ON raw_batches (received_ms DESC);
"""

# (version, nom, sql) — ordre = ordre d'application.
MIGRATIONS: list[tuple[int, str, str]] = [
    (1, "initial", _0001_INITIAL),
]
