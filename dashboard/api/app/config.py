"""Configuration lue depuis l'environnement.

Lue à chaque appel (pas de cache) — le volume est faible et ça garde les tests
simples (un fichier temporaire par test via ``DENGON_DASHBOARD_DB``).
"""

from __future__ import annotations

import os
from pathlib import Path

DB_ENV_VAR = "DENGON_DASHBOARD_DB"
DEFAULT_DB_PATH = "dashboard.db"

MAX_BATCH_BYTES_ENV_VAR = "DENGON_DASHBOARD_MAX_BATCH_BYTES"
# Garde-fou, pas une valeur tirée du protocole : borne la mémoire consommée
# par un POST /ingest/batch (retour de revue #59, point 1). Ajustable si un
# vrai volume de démo le justifie.
DEFAULT_MAX_BATCH_BYTES = 2 * 1024 * 1024  # 2 MiB


def db_path() -> Path:
    """Chemin du fichier SQLite du dashboard."""
    return Path(os.environ.get(DB_ENV_VAR, DEFAULT_DB_PATH))


def max_batch_bytes() -> int:
    """Taille maximale acceptée pour le corps d'un ``POST /ingest/batch``."""
    return int(os.environ.get(MAX_BATCH_BYTES_ENV_VAR, DEFAULT_MAX_BATCH_BYTES))
