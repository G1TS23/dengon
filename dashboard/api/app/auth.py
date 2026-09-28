"""JWT courts par nœud (B-2/C-5, `docs/synthese/06-securite.md`).

Authentifie un **nœud** (relais ou client) auprès de `POST /ingest/batch` —
pas un opérateur humain sur le dashboard web (session/cookie, hors
périmètre de l'US-216, cf. `docs/synthese/09-dashboard-et-donnees.md` §6).
"""

from __future__ import annotations

import time

import jwt

from .config import jwt_secret

ALGORITHM = "HS256"
# « Jeton court » : assez long pour couvrir une fenêtre Wi-Fi de relais
# typique sans re-provisionnement constant, assez court pour qu'un jeton
# volé n'ouvre pas un accès indéfini.
DEFAULT_TTL_S = 24 * 3600


class InvalidToken(Exception):
    """Jeton absent, mal formé, expiré, ou de signature invalide.

    Un seul type d'erreur pour tous les cas d'échec : côté appelant
    (`POST /ingest/batch`), les deux se traduisent identiquement par un
    401 — distinguer les sous-cas n'aiderait qu'un attaquant à savoir quel
    élément corriger.
    """


def create_token(node_id: str, *, ttl_s: int = DEFAULT_TTL_S, now: float | None = None) -> str:
    """Émet un jeton pour `node_id`, valable `ttl_s` secondes à partir de
    `now` (l'horloge système par défaut ; paramétrable pour les tests).
    """
    now = time.time() if now is None else now
    payload = {"node_id": node_id, "iat": int(now), "exp": int(now + ttl_s)}
    return jwt.encode(payload, jwt_secret(), algorithm=ALGORITHM)


def node_id_from_authorization_header(header: str | None) -> str:
    """Extrait et vérifie le `node_id` porté par `Authorization: Bearer <jwt>`.

    Lève [`InvalidToken`] pour toute défaillance (en-tête absent ou mal
    formé, jeton expiré, signature invalide, `node_id` absent du jeton).
    """
    if not header or not header.startswith("Bearer "):
        raise InvalidToken("en-tête Authorization: Bearer <jwt> manquant")
    token = header[len("Bearer ") :]
    try:
        payload = jwt.decode(token, jwt_secret(), algorithms=[ALGORITHM])
    except jwt.PyJWTError as exc:
        raise InvalidToken(f"jeton invalide : {exc}") from exc
    node_id = payload.get("node_id")
    if not isinstance(node_id, str) or not node_id:
        raise InvalidToken("jeton sans node_id")
    return node_id
