"""Configuration lue depuis l'environnement.

Une seule variable pour l'instant : le chemin de la base SQLite. Lue à chaque
appel (pas de cache) — le volume est faible et ça garde les tests simples
(un fichier temporaire par test via ``DENGON_DASHBOARD_DB``).
"""

from __future__ import annotations

import os
from pathlib import Path

DB_ENV_VAR = "DENGON_DASHBOARD_DB"
DEFAULT_DB_PATH = "dashboard.db"


def db_path() -> Path:
    """Chemin du fichier SQLite du dashboard."""
    return Path(os.environ.get(DB_ENV_VAR, DEFAULT_DB_PATH))
