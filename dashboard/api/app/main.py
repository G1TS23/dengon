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
* ``GET  /api/stream``    — diffusion en **SSE** des événements ingérés
  (US-218) : rattrapage depuis `Last-Event-ID` (ou depuis le début), puis
  diffusion live via `app/stream.py::Broadcaster`.
* ``GET  /api/messages``  — liste des messages suivis (US-219).
* ``GET  /api/messages/{msg_log_id}`` — détail + parcours (`hops`, dérivés
  de `events` à la lecture, voir `app/messages_api.py`) d'un message
  (US-219), consommés par `dashboard/web`.
* ``GET  /api/integrity`` — verdict d'intégrité par nœud (US-310), calculé
  via `dengon-verify` en sous-processus (`app/integrity.py`).

Les projections (`messages`) sont l'objet de l'US-217 ; `links` reste hors
périmètre — voir `03-ecarts-conception.md`. `message_hops` (§11.2) n'est pas
une table à part : dérivée à la lecture depuis `events`, voir
`app/messages_api.py`.
"""

from __future__ import annotations

import asyncio
import json
import sqlite3
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from typing import Any

from fastapi import FastAPI, Request, status
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse, StreamingResponse
from starlette.concurrency import run_in_threadpool

from . import ingest, integrity
from .auth import InvalidToken, create_token, node_id_from_authorization_header
from .config import jwt_secret, max_batch_bytes
from .db import LockedConnection, connect, run_migrations
from .messages_api import get_message, get_message_hops, list_messages
from .stream import Broadcaster, StreamEvent


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
        # `get_running_loop()` DANS `lifespan` (une coroutine, donc déjà sur
        # la boucle qui servira toutes les requêtes) : le `Broadcaster` doit
        # programmer ses `put_nowait` sur CETTE boucle précise depuis le
        # threadpool (thread différent, pas de boucle asyncio à lui), pas sur
        # une boucle récupérée au hasard d'un autre contexte.
        app.state.broadcaster = Broadcaster(asyncio.get_running_loop())
        yield
    finally:
        db.close()


app = FastAPI(
    title="dengon — dashboard api",
    version="0.1.0",
    summary="Ingestion validée des batchs d'événements (schéma + JWT + signature Ed25519).",
    lifespan=lifespan,
)

# CORS **en lecture seule** (`GET`), toute origine : `dashboard/web` (US-219,
# US-310) est une page statique, potentiellement servie depuis un autre
# domaine/port que l'API (voire ouverte en `file://`, origine `null`) — sans
# ça, le navigateur bloque `fetch`/`EventSource` avant même que la requête ne
# parte. Sans risque ici : ces routes ne renvoient que des données déjà
# redigées (voir `app/messages_api.py`), sans cookie ni session, et
# `allow_methods` exclut `POST` — un site tiers ne peut pas s'en servir pour
# faire écrire un visiteur dans `/ingest/batch`/`/api/nodes` (qui exigent de
# toute façon un jeton JWT qu'aucune origine ne peut deviner).
# `allow_headers` inclut `Last-Event-ID` : c'est l'en-tête que le navigateur
# renvoie automatiquement à la reconnexion d'un `EventSource` (`GET
# /api/stream`, US-218) pour le rattrapage sans perte — sans lui dans la
# liste, le preflight cross-origine de la reconnexion est rejeté par
# Starlette et le flux ne rattrape plus après une coupure.
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["GET"],
    allow_headers=["Last-Event-ID"],
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
            ingest.ingest_batch,
            request.app.state.db,
            jwt_node_id,
            raw,
            request.app.state.broadcaster,
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
        existing = conn.execute(
            "SELECT 1 FROM nodes WHERE node_id = ?", (node_id,)
        ).fetchone()
        if existing is not None:
            # Pas d'upsert : un ON CONFLICT DO UPDATE remplaçait la clé
            # publique d'un nœud déjà enregistré et le re-whitelistait, y
            # compris un nœud qu'un opérateur aurait retiré — sans auth
            # opérateur sur cette route, n'importe qui pouvait ainsi prendre
            # le contrôle d'un node_id existant (revue PR #91, point
            # important). Correctif minimal en attendant l'auth opérateur
            # (écart consigné dans 03-ecarts-conception.md) : un node_id
            # déjà pris se réenregistre pas.
            return JSONResponse(
                status_code=status.HTTP_409_CONFLICT,
                content={"error": f"node_id {node_id!r} déjà enregistré"},
            )
        conn.execute(
            "INSERT INTO nodes (node_id, kind, pub_sign, whitelisted) VALUES (?, ?, ?, 1)",
            (node_id, kind, pub_sign),
        )

    return JSONResponse(
        status_code=status.HTTP_201_CREATED,
        content={"node_id": node_id, "token": create_token(node_id)},
    )


# Entre deux événements réels, un commentaire SSE (`:` en tête de ligne,
# ignoré par tout client conforme) maintient la connexion active — un
# reverse-proxy (US-224) coupe couramment une connexion HTTP sans octet
# échangé au bout de 30-60 s, bien avant qu'un vrai événement n'arrive dans
# une démo calme.
_HEARTBEAT_INTERVAL_S = 15.0
_HEARTBEAT_SSE = ": keep-alive\n\n"


def _events_since(db: LockedConnection, after_rowid: int) -> list[StreamEvent]:
    """Les événements de `rowid` strictement supérieur à `after_rowid`, dans
    l'ordre d'insertion. `after_rowid = 0` (par défaut, aucun `Last-Event-ID`
    reçu) renvoie tout l'historique — un client qui se connecte pour la
    première fois rattrape l'état actuel plutôt que de partir à vide.
    """
    with db.locked() as conn:
        rows = conn.execute(
            "SELECT rowid, event_id, ts_ms, node_id, name, payload FROM events "
            "WHERE rowid > ? ORDER BY rowid",
            (after_rowid,),
        ).fetchall()
    return [
        StreamEvent(
            rowid=row["rowid"],
            event_id=row["event_id"],
            ts_ms=row["ts_ms"],
            node_id=row["node_id"],
            name=row["name"],
            payload=json.loads(row["payload"]),
        )
        for row in rows
    ]


def _parse_last_event_id(header: str | None) -> int:
    # Un `Last-Event-ID` absent ou mal formé équivaut à « depuis le début »
    # (0) plutôt qu'une erreur 4xx : un client SSE ne sait pas fabriquer cet
    # en-tête lui-même à la connexion initiale (c'est le navigateur qui le
    # renvoie automatiquement, sur RECONNEXION, avec la valeur du dernier
    # `id:` reçu) — le rejeter casserait la toute première connexion.
    if header is None:
        return 0
    try:
        return int(header)
    except ValueError:
        return 0


async def _event_stream(request: Request) -> AsyncIterator[str]:
    db: LockedConnection = request.app.state.db
    broadcaster: Broadcaster = request.app.state.broadcaster
    after_rowid = _parse_last_event_id(request.headers.get("last-event-id"))

    # Abonné AVANT le rattrapage : un événement publié pendant la lecture de
    # rattrapage ne doit être ni perdu (abonnement après lecture : fenêtre où
    # un événement publié entre les deux échapperait aux deux mécanismes) ni
    # dupliqué (filtré ci-dessous via `last_rowid`).
    queue = broadcaster.subscribe()
    try:
        last_rowid = after_rowid
        for event in await run_in_threadpool(_events_since, db, after_rowid):
            yield event.to_sse()
            last_rowid = event.rowid

        while True:
            if await request.is_disconnected():
                return
            try:
                event = await asyncio.wait_for(queue.get(), timeout=_HEARTBEAT_INTERVAL_S)
            except TimeoutError:
                yield _HEARTBEAT_SSE
                continue
            if event.rowid <= last_rowid:
                continue  # déjà servi pendant le rattrapage ci-dessus
            yield event.to_sse()
            last_rowid = event.rowid
    finally:
        broadcaster.unsubscribe(queue)


@app.get("/api/stream")
async def stream_events(request: Request) -> StreamingResponse:
    """SSE (US-218) : rattrapage depuis `Last-Event-ID` (0 = tout
    l'historique), puis diffusion live. `Last-Event-ID` est l'en-tête que
    tout navigateur renvoie automatiquement à la reconnexion avec le dernier
    `id:` reçu — la reprise « sans perdre le fil » demandée par le critère
    d'acceptation ne nécessite donc aucune logique côté client au-delà de
    l'API `EventSource` standard.
    """
    return StreamingResponse(
        _event_stream(request),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )


@app.get("/api/messages")
def list_messages_route(request: Request) -> JSONResponse:
    """Liste des messages suivis (projection `messages`, US-217), pour
    l'écran liste du dashboard web (US-219). Route **sync** (comme
    `/healthz`) : une lecture SQLite seule, pas de traitement CPU notable —
    pas besoin de `run_in_threadpool`.
    """
    return JSONResponse(content=list_messages(request.app.state.db))


@app.get("/api/messages/{msg_log_id}")
def get_message_route(msg_log_id: str, request: Request) -> JSONResponse:
    """Détail d'un message + son parcours (`hops`, dérivé de `events` à la
    lecture — voir `app/messages_api.py`), pour l'écran détail (US-219).
    """
    db = request.app.state.db
    message = get_message(db, msg_log_id)
    if message is None:
        return JSONResponse(
            status_code=status.HTTP_404_NOT_FOUND,
            content={"error": f"message inconnu : {msg_log_id}"},
        )
    message["hops"] = get_message_hops(db, msg_log_id)
    return JSONResponse(content=message)


@app.get("/api/integrity")
async def integrity_route(request: Request) -> JSONResponse:
    """Verdict d'intégrité par nœud (US-310) : `dengon-verify` (sous-
    processus, `app/integrity.py`) rejoue le journal chaîné reconstruit
    depuis `events` et rend `ok`/`broken`/`fork`/`gap`, ou `unverified` pour
    un nœud sans aucun événement portant `entry_hash`/`prev_hash`.
    """
    try:
        verdicts = await run_in_threadpool(integrity.check_all_nodes, request.app.state.db)
    except integrity.IntegrityCheckError as exc:
        # dengon-verify introuvable/en échec/hors délai : signalé à
        # l'opérateur (503, comme le stockage momentanément occupé
        # ci-dessus), pas un 500 générique — la cause est extérieure au
        # traitement de la requête elle-même.
        return JSONResponse(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            content={"error": str(exc)},
        )
    return JSONResponse(
        status_code=status.HTTP_200_OK,
        content=[
            {
                "node_id": v.node_id,
                "verdict": v.verdict,
                "entries": v.entries,
                "first_seq": v.first_seq,
                "last_seq": v.last_seq,
                "signatures": v.signatures,
            }
            for v in verdicts
        ],
    )
