"""Connexion SQLite et exécution des migrations.

Système volontairement minimal : les migrations sont déclarées dans
``app/migrations.py`` (``MIGRATIONS``), appliquées dans l'ordre de version et
tracées dans la table ``schema_migrations``. Pas d'Alembic — surdimensionné
pour un squelette.

`run_migrations` est **atomique et sûr en concurrence** : chaque migration
s'applique dans une transaction ``BEGIN IMMEDIATE`` (les instructions DDL *et*
l'enregistrement dans ``schema_migrations`` réussissent ou échouent ensemble),
et la présence de la version est revérifiée sous verrou — deux processus qui
démarrent en même temps (``uvicorn --workers N``) n'appliquent pas la migration
deux fois.
"""

from __future__ import annotations

import sqlite3

from .config import db_path
from .migrations import MIGRATIONS


def connect() -> sqlite3.Connection:
    """Ouvre une connexion sur la base configurée.

    ``autocommit`` désactivé (``isolation_level = None``) : les transactions
    sont **explicites** (``BEGIN`` / ``COMMIT``), comportement identique de
    Python 3.11 à 3.13 et seul moyen de rendre le DDL transactionnel.

    ``check_same_thread=False`` : la connexion créée au démarrage de l'app est
    réutilisée pour les écritures depuis le threadpool (``run_in_threadpool``),
    donc depuis un thread différent de celui qui l'a ouverte. Elle n'est
    utilisée que sous ``app.state.db_lock`` (voir ``main.py``), qui sérialise
    l'accès — ``sqlite3.Connection`` n'est pas sûre en usage concurrent non
    protégé, même avec ce réglage.
    """
    conn = sqlite3.connect(db_path(), isolation_level=None, check_same_thread=False)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA journal_mode = WAL")  # lecteurs et écrivain ne se bloquent pas
    conn.execute("PRAGMA foreign_keys = ON")
    conn.execute("PRAGMA busy_timeout = 5000")  # attendre un verrou plutôt qu'échouer aussitôt
    return conn


def _ensure_schema_migrations(conn: sqlite3.Connection) -> None:
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations ("
        "  version    INTEGER PRIMARY KEY,"
        "  name       TEXT    NOT NULL,"
        "  applied_at TEXT    NOT NULL DEFAULT (datetime('now'))"
        ")"
    )


def _applied_versions(conn: sqlite3.Connection) -> set[int]:
    return {row["version"] for row in conn.execute("SELECT version FROM schema_migrations")}


def run_migrations(conn: sqlite3.Connection) -> list[int]:
    """Applique les migrations manquantes. Retourne les versions nouvellement appliquées."""
    _ensure_schema_migrations(conn)
    newly_applied: list[int] = []

    # Calculée une fois : re-sélectionner à chaque itération serait un
    # SELECT par migration, y compris pour celles déjà appliquées qu'on ne
    # fait que sauter (retour de revue #59, point 5) — coût O(n) au
    # démarrage dès que MIGRATIONS grossit. Seule la revérification sous
    # verrou ci-dessous a besoin d'une lecture fraîche.
    deja_appliquees = _applied_versions(conn)

    for version, name, statements in sorted(MIGRATIONS, key=lambda m: m[0]):
        if version in deja_appliquees:
            continue

        # BEGIN IMMEDIATE : prend le verrou d'écriture tout de suite, donc un
        # second processus attend ici puis reverra la version comme appliquée.
        conn.execute("BEGIN IMMEDIATE")
        try:
            if version in _applied_versions(conn):  # revérification sous verrou
                conn.execute("ROLLBACK")
                continue
            for statement in statements:
                conn.execute(statement)
            conn.execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?, ?)",
                (version, name),
            )
            conn.execute("COMMIT")
        except Exception:
            conn.execute("ROLLBACK")
            raise

        newly_applied.append(version)

    return newly_applied
