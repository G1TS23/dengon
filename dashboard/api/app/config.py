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

VERIFY_BIN_ENV_VAR = "DENGON_VERIFY_BIN"
# Nom de commande, pas un chemin absolu : résolu via PATH par défaut (comme
# tout binaire du workspace `cargo build` installé/copié à côté de `uv run`,
# CI ou VPS). Ajustable pour pointer un binaire précis en test (retour de
# revue — l'image Docker actuelle de dashboard/api, US-224, ne construit PAS
# encore ce binaire : écart consigné dans 03-ecarts-conception.md).
DEFAULT_VERIFY_BIN = "dengon-verify"

JWT_SECRET_ENV_VAR = "DENGON_DASHBOARD_JWT_SECRET"
# PyJWT lève InsecureKeyLengthWarning en dessous de 32 octets pour HS256 (la
# taille de sortie de HMAC-SHA256) — vérifié au démarrage plutôt que laissé
# comme un avertissement ignorable à chaque requête (revue PR #91, point
# mineur), même logique « échouer tôt » que le reste de ce module.
MIN_JWT_SECRET_BYTES = 32


def db_path() -> Path:
    """Chemin du fichier SQLite du dashboard."""
    return Path(os.environ.get(DB_ENV_VAR, DEFAULT_DB_PATH))


def max_batch_bytes() -> int:
    """Taille maximale acceptée pour le corps d'un ``POST /ingest/batch``."""
    raw = os.environ.get(MAX_BATCH_BYTES_ENV_VAR)
    if raw is None:
        return DEFAULT_MAX_BATCH_BYTES
    try:
        return int(raw)
    except ValueError as exc:
        # Sans ce garde-fou, une valeur malformée (ex. "2MB") ne casse rien au
        # démarrage : elle fait planter chaque POST /ingest/batch avec un 500,
        # puisque max_batch_bytes() est relue à chaque requête (retour de
        # revue #59, round 2). Échouer tout de suite et fort est plus lisible
        # qu'un 500 par requête sans rapport apparent avec la config.
        raise RuntimeError(
            f"{MAX_BATCH_BYTES_ENV_VAR}={raw!r} n'est pas un entier valide"
        ) from exc


def dengon_verify_bin() -> str:
    """Commande (ou chemin) du binaire `dengon-verify` (US-310), appelé en
    sous-processus par `app/integrity.py`.
    """
    return os.environ.get(VERIFY_BIN_ENV_VAR, DEFAULT_VERIFY_BIN)


def jwt_secret() -> str:
    """Secret HMAC pour signer/vérifier les JWT courts des nœuds (B-2/C-5,
    `docs/synthese/06-securite.md`).

    Pas de valeur par défaut, volontairement — contrairement à
    `max_batch_bytes()` : un secret par défaut connu de tous les
    déploiements serait une clé maîtresse publique, exactement ce qu'un
    « jeton court » est censé empêcher. Doit être positionné avant le
    démarrage (validé tôt par `lifespan`, même logique que
    `max_batch_bytes()` — échouer au démarrage plutôt qu'à la première
    requête).
    """
    raw = os.environ.get(JWT_SECRET_ENV_VAR)
    if not raw:
        raise RuntimeError(
            f"{JWT_SECRET_ENV_VAR} doit être défini (secret JWT, aucune valeur par défaut)"
        )
    if len(raw.encode("utf-8")) < MIN_JWT_SECRET_BYTES:
        raise RuntimeError(
            f"{JWT_SECRET_ENV_VAR} doit faire au moins {MIN_JWT_SECRET_BYTES} octets"
        )
    return raw


RELAY_SILENT_MINUTES_ENV_VAR = "DENGON_DASHBOARD_RELAY_SILENT_MINUTES"
# docs/synthese/09-dashboard-et-donnees.md §5 : alerte `relay_silent` = pas de
# `relay.health` depuis 5 min (le relais en émet un toutes les 60 s).
DEFAULT_RELAY_SILENT_MINUTES = 5


def relay_silent_ms() -> int:
    """Durée sans `relay.health` au-delà de laquelle un relais est « muet »
    (alerte `relay_silent`, US-311), en millisecondes."""
    raw = os.environ.get(RELAY_SILENT_MINUTES_ENV_VAR)
    if raw is None:
        return DEFAULT_RELAY_SILENT_MINUTES * 60_000
    try:
        minutes = float(raw)
    except ValueError as exc:
        raise RuntimeError(
            f"{RELAY_SILENT_MINUTES_ENV_VAR}={raw!r} n'est pas un nombre valide"
        ) from exc
    if minutes <= 0:
        raise RuntimeError(f"{RELAY_SILENT_MINUTES_ENV_VAR} doit être > 0")
    return int(minutes * 60_000)
