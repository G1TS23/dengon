"""Connexion SQLite et exécution des migrations.

Système de migration volontairement minimal : un fichier ``NNNN_nom.sql`` par
migration dans ``migrations/``, appliqué dans l'ordre du numéro, tracé dans la
table ``schema_migrations``. Pas d'Alembic — surdimensionné pour un squelette.
"""

from __future__ import annotations

import sqlite3
from pathlib import Path

from .config import db_path

MIGRATIONS_DIR = Path(__file__).resolve().parent.parent / "migrations"


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

    for sql_file in sorted(MIGRATIONS_DIR.glob("*.sql")):
        version = int(sql_file.name.split("_", 1)[0])
        if version in applied:
            continue
        conn.executescript(sql_file.read_text(encoding="utf-8"))
        conn.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?, ?)",
            (version, sql_file.name),
        )
        conn.commit()
        newly_applied.append(version)

    return newly_applied
