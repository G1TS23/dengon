"""Connexion SQLite et exécution des migrations.

Système volontairement minimal : les migrations sont des littéraux SQL déclarés
dans ``app/migrations.py`` (``MIGRATIONS``), appliqués dans l'ordre de version
et tracés dans la table ``schema_migrations``. Pas d'Alembic — surdimensionné
pour un squelette.
"""

from __future__ import annotations

import sqlite3

from .config import db_path
from .migrations import MIGRATIONS


def connect() -> sqlite3.Connection:
    """Ouvre une connexion sur la base configurée (WAL, clés étrangères actives)."""
    conn = sqlite3.connect(db_path())
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA journal_mode = WAL")
    conn.execute("PRAGMA foreign_keys = ON")
    return conn


def _applied_versions(conn: sqlite3.Connection) -> set[int]:
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations ("
        "  version    INTEGER PRIMARY KEY,"
        "  name       TEXT    NOT NULL,"
        "  applied_at TEXT    NOT NULL DEFAULT (datetime('now'))"
        ")"
    )
    return {row["version"] for row in conn.execute("SELECT version FROM schema_migrations")}


def run_migrations(conn: sqlite3.Connection) -> list[int]:
    """Applique les migrations manquantes. Retourne les versions nouvellement appliquées."""
    applied = _applied_versions(conn)
    newly_applied: list[int] = []

    for version, name, sql in sorted(MIGRATIONS, key=lambda m: m[0]):
        if version in applied:
            continue
        conn.executescript(sql)
        conn.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?, ?)",
            (version, name),
        )
        conn.commit()
        newly_applied.append(version)

    return newly_applied
