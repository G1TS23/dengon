"""Point d'entrée FastAPI du dashboard dengon.

Routes :

* ``GET  /healthz``       — liveness, répond 200.
* ``POST /ingest/batch``  — **ingestion validée** (US-216) : schéma
  (`contracts/events/batch.schema.json`), authentification JWT du nœud,
  signature Ed25519 du batch, déduplication sur ``event_id``. Remplace le
  squelette permissif de l'US-110 (qui acceptait n'importe quel JSON et le
  stockait tel quel dans ``raw_batches``) — voir `app/ingest.py`.
* ``POST /api/nodes``     — enregistre un nœud (`pub_sign`) et lui remet un
  jeton JWT. **Sans authentification opérateur** pour l'instant — écart
  consigné dans `03-ecarts-conception.md`.

Les projections (`messages`/`links`/`message_hops`) et le flux SSE sont
l'objet d'US-217/US-218 (sprint 2).
"""

from __future__ import annotations

import json
import sqlite3
from contextlib import asynccontextmanager
from typing import Any

from fastapi import FastAPI, Request, status
from fastapi.responses import JSONResponse
from starlette.concurrency import run_in_threadpool

from . import ingest
from .auth import InvalidToken, create_token, node_id_from_authorization_header
from .config import jwt_secret, max_batch_bytes
from .db import LockedConnection, connect, run_migrations


class _BodyTooLarge(Exception):
    """Le corps dépasse la limite configurée. Levée pendant la lecture en flux."""


@asynccontextmanager
async def lifespan(app: FastAPI):
    # Appelée ici uniquement pour valider la config tôt : une variable
    # d'environnement malformée (ou, pour le secret JWT, absente) fait
    # échouer le démarrage plutôt que chaque requête une par une (retour de
    # revue #59, round 2, étendu au secret JWT avec US-216). Ni l'une ni
    # l'autre n'est mise en cache — relues à chaque appel, comme documenté
    # dans config.py.
    max_batch_bytes()
    jwt_secret()
    conn = connect()
    # LockedConnection couple connexion et verrou dès l'ouverture (retour de
    # revue #59, round 4, point d'OswinFreyr — voir db.py) : `db.close()`
    # ferme la connexion sous-jacente, donc un seul chemin de fermeture,
    # que run_migrations() réussisse ou non.
    db = LockedConnection(conn)
    try:
        # run_migrations() DANS le try : si une migration échoue (ex.
        # OperationalError après busy_timeout avec plusieurs workers), conn
        # doit quand même être fermée, pas fuiter — c'était le cas avant
        # (retour de revue #59, round 4, point d'OswinFreyr : `connect()` et
        # `run_migrations()` étaient hors du try/finally).
        run_migrations(conn)
        # Connexion unique, réutilisée pour toutes les écritures (retour de
        # revue #59, point 3) : ouvrir une connexion par batch (fichier + 3
        # PRAGMA) devient un coût redondant dès qu'un flush réel arrive.
        app.state.db = db
        yield
    finally:
        db.close()


app = FastAPI(
    title="dengon — dashboard api",
    version="0.1.0",
    summary="Ingestion validée des batchs d'événements (schéma + JWT + signature Ed25519).",
    lifespan=lifespan,
)


@app.get("/healthz")
def healthz() -> dict[str, str]:
    return {"status": "ok"}


# Borne du drain supplémentaire une fois la limite dépassée : vider le flux
# SANS borne (comme avant) transformait la limite de taille en simple limite
# mémoire, pas de travail fourni — un client qui envoie en continu occupait
# la coroutine indéfiniment (retour de revue #59, round 5, point 3
# d'OswinFreyr). Passé ce cap, on abandonne : uvicorn ferme la connexion au
# lieu de livrer un 413 propre, ce qui est le compromis le plus sûr pour un
# client hors limite.
_DRAIN_CAP_BYTES = 1 * 1024 * 1024


async def _read_limited_body(request: Request, limit: int) -> bytes:
    """Lit le corps par morceaux, coupe dès que `limit` est dépassé.

    `request.body()` charge tout le corps en mémoire sans borne (retour de
    revue #59, point 1) : un nœud buggé ou malveillant, une fois exposé sur le
    VPS, pourrait envoyer un batch de plusieurs centaines de Mo et épuiser la
    mémoire du process — /healthz avec lui. `Content-Length`, quand présent,
    permet un rejet rapide ; la lecture en flux couvre aussi son absence
    (chunked) ou un en-tête mensonger.
    """
    content_length = request.headers.get("content-length")
    if content_length is not None:
        try:
            trop_gros = int(content_length) > limit
        except ValueError:
            trop_gros = False  # en-tête non numérique : on se fie au comptage réel ci-dessous
        if trop_gros:
            # Vider un peu le flux avant de lever : sans ça, le corps reste
            # non lu sur la connexion au moment du 413, ce qui peut forcer
            # uvicorn/h11 à couper la connexion keep-alive plutôt que de
            # livrer la réponse proprement (retour de revue #59, round 2).
            # Borné à `_DRAIN_CAP_BYTES` (round 5, point 3) : le flux n'a pas
            # encore été touché ici, donc un nouvel appel à `request.stream()`
            # est sûr.
            #
            # Sauf si le client attend `100 Continue` (curl l'envoie par
            # défaut au-delà de 1 MiB) : lire `request.stream()` ici
            # déclenche l'envoi de `100 Continue` par uvicorn au premier
            # `receive()`, ce qui *invite* le client à téléverser un corps
            # qu'on s'apprête à rejeter — bande passante perdue, et le
            # client peut voir une erreur d'envoi au lieu du 413 propre.
            # Sans `Expect: 100-continue`, rien n'a encore été envoyé côté
            # client à ce stade, donc rien à drainer (retour de revue #59,
            # round 8, point 1 d'OswinFreyr).
            if "100-continue" not in request.headers.get("expect", "").lower():
                await _drain_bounded(request.stream())
            raise _BodyTooLarge

    morceaux: list[bytes] = []
    total = 0
    hors_limite = False
    drained = 0
    async for morceau in request.stream():
        if hors_limite:
            # On continue de consommer LA MÊME boucle `async for` plutôt que
            # de rappeler `request.stream()` : Starlette marque le flux
            # consommé (`_stream_consumed`) dès que le dernier message ASGI
            # (`more_body=False`) est reçu, AVANT même de le céder à ce
            # `async for` — si tout le corps est arrivé en un seul message,
            # rappeler `request.stream()` lève `RuntimeError("Stream
            # consumed")`. Rester dans la boucle en cours évite le problème
            # sans avoir à rattraper cette `RuntimeError` (retour de revue
            # #59, round 5, point 3 : l'`except RuntimeError` précédent
            # rattrapait plus large que « flux déjà consommé »).
            drained += len(morceau)
            if drained > _DRAIN_CAP_BYTES:
                break
            continue
        total += len(morceau)
        if total > limit:
            hors_limite = True
            continue
        morceaux.append(morceau)
    if hors_limite:
        raise _BodyTooLarge
    return b"".join(morceaux)


async def _drain_bounded(stream: Any) -> None:
    """Consomme un flux ASGI jusqu'à `_DRAIN_CAP_BYTES`, puis abandonne."""
    drained = 0
    async for morceau in stream:
        drained += len(morceau)
        if drained > _DRAIN_CAP_BYTES:
            return


@app.post("/ingest/batch", status_code=status.HTTP_202_ACCEPTED)
async def ingest_batch_route(request: Request) -> JSONResponse:
    # JWT vérifié AVANT de lire le corps : un jeton absent/invalide n'a pas
    # besoin qu'on lise (et donc qu'on drain, potentiellement 1 MiB) le
    # corps pour être rejeté.
    try:
        jwt_node_id = node_id_from_authorization_header(request.headers.get("authorization"))
    except InvalidToken as exc:
        return JSONResponse(
            status_code=status.HTTP_401_UNAUTHORIZED, content={"error": str(exc)}
        )

    # Lue une seule fois dans une variable locale (retour de revue #59, round
    # 4, point d'OswinFreyr) : appelée deux fois avant, le message d'erreur du
    # 413 pouvait annoncer une limite différente de celle réellement
    # appliquée si la variable d'environnement changeait entre les deux
    # lectures (fenêtre étroite, mais un test qui monkeypatche entre les deux
    # l'aurait révélé).
    limite = max_batch_bytes()
    try:
        raw = await _read_limited_body(request, limite)
    except _BodyTooLarge:
        return JSONResponse(
            status_code=status.HTTP_413_CONTENT_TOO_LARGE,
            content={"error": f"corps trop volumineux (max {limite} octets)"},
        )

    try:
        result = await run_in_threadpool(
            ingest.ingest_batch, request.app.state.db, jwt_node_id, raw
        )
    except ingest.IngestError as exc:
        return JSONResponse(status_code=exc.status_code, content={"error": exc.detail})
    except (sqlite3.OperationalError, sqlite3.ProgrammingError):
        # OperationalError : SQLite n'a qu'un écrivain, le perdant d'un flush
        # concurrent échoue après le busy_timeout. ProgrammingError : la
        # connexion a été fermée sous nos pieds (lifespan en cours d'arrêt
        # pendant qu'une écriture tournait encore sur le threadpool — retour
        # de revue #59, round 2). Dans les deux cas, le nœud doit retenter.
        return JSONResponse(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            content={"error": "stockage momentanément occupé, réessayer"},
            headers={"Retry-After": "1"},
        )

    return JSONResponse(
        status_code=status.HTTP_202_ACCEPTED,
        content={
            "stored": True,
            "batch_id": result.batch_id,
            "event_count": result.event_count,
            "new_event_count": result.new_event_count,
        },
    )


@app.post("/api/nodes", status_code=status.HTTP_201_CREATED)
async def register_node(request: Request) -> JSONResponse:
    """Enregistre un nœud (`pub_sign`) et lui remet un jeton JWT.

    Étape (4) du déploiement décrite par `docs/synthese/09-dashboard-et-
    donnees.md` §7 : « enregistrer chaque relais via `POST /api/nodes` et
    lui remettre un JWT ». **Aucune authentification opérateur** ici —
    l'US-216 ne couvre que l'auth des NŒUDS pour `/ingest/batch`, pas
    l'auth d'un opérateur humain (session/cookie, hors périmètre, écart
    consigné dans `03-ecarts-conception.md`).
    """
    try:
        body = json.loads(await request.body())
    except (UnicodeDecodeError, ValueError) as exc:
        return JSONResponse(
            status_code=status.HTTP_400_BAD_REQUEST,
            content={"error": f"corps JSON invalide : {exc}"},
        )

    if not isinstance(body, dict):
        return JSONResponse(
            status_code=status.HTTP_400_BAD_REQUEST,
            content={"error": "corps invalide : un objet JSON est attendu"},
        )
    node_id = body.get("node_id")
    kind = body.get("kind")
    pub_sign_hex = body.get("pub_sign")
    if (
        not isinstance(node_id, str)
        or not node_id
        or kind not in ("relay", "client")
        or not isinstance(pub_sign_hex, str)
    ):
        return JSONResponse(
            status_code=status.HTTP_400_BAD_REQUEST,
            content={"error": "node_id (str), kind ('relay'|'client') et pub_sign (hex) requis"},
        )
    try:
        pub_sign = bytes.fromhex(pub_sign_hex)
    except ValueError:
        return JSONResponse(
            status_code=status.HTTP_400_BAD_REQUEST,
            content={"error": "pub_sign doit être une chaîne hexadécimale"},
        )
    if len(pub_sign) != 32:
        return JSONResponse(
            status_code=status.HTTP_400_BAD_REQUEST,
            content={"error": "pub_sign doit faire 32 octets (clé publique Ed25519)"},
        )

    db: LockedConnection = request.app.state.db
    with db.locked() as conn:
        conn.execute(
            "INSERT INTO nodes (node_id, kind, pub_sign, whitelisted) VALUES (?, ?, ?, 1) "
            "ON CONFLICT(node_id) DO UPDATE SET "
            "kind = excluded.kind, pub_sign = excluded.pub_sign, whitelisted = 1",
            (node_id, kind, pub_sign),
        )

    return JSONResponse(
        status_code=status.HTTP_201_CREATED,
        content={"node_id": node_id, "token": create_token(node_id)},
    )
