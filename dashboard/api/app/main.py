"""Point d'entrée FastAPI du dashboard dengon (squelette — US-110).

Deux routes seulement :

* ``GET  /healthz``       — liveness, répond 200.
* ``POST /ingest/batch``  — **ingestion permissive** : accepte n'importe quel
  JSON UTF-8 bien formé et le stocke tel quel dans ``raw_batches``.

La validation de schéma, la vérification de signature Ed25519, la déduplication
sur ``event_id`` et les projections sont l'objet d'US-216 / US-217 (sprint 2).
C'est délibéré : le dashboard doit pouvoir démarrer avant que le format
d'événement soit figé (cf. docs/olivier/proposition-organisation-github.md §3.3).
"""

from __future__ import annotations

import json
import sqlite3
import time
import uuid
from contextlib import asynccontextmanager
from typing import Any

from fastapi import FastAPI, Request, status
from fastapi.responses import JSONResponse
from starlette.concurrency import run_in_threadpool

from .config import max_batch_bytes
from .db import LockedConnection, connect, run_migrations


class _BodyTooLarge(Exception):
    """Le corps dépasse la limite configurée. Levée pendant la lecture en flux."""


@asynccontextmanager
async def lifespan(app: FastAPI):
    # Appelée ici uniquement pour valider la config tôt : une variable
    # d'environnement malformée fait échouer le démarrage plutôt que chaque
    # POST /ingest/batch un par un (retour de revue #59, round 2). La valeur
    # elle-même n'est pas mise en cache — max_batch_bytes() est relue à
    # chaque requête, comme documenté dans config.py.
    max_batch_bytes()
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
    title="dengon — dashboard api (squelette)",
    version="0.1.0",
    summary="Ingestion permissive des batchs d'événements. Validation : US-216.",
    lifespan=lifespan,
)


@app.get("/healthz")
def healthz() -> dict[str, str]:
    return {"status": "ok"}


def _guess_event_count(parsed: Any) -> int | None:
    """Devine le nombre d'événements sans imposer de schéma.

    Accepte un tableau nu ``[...]`` ou un objet ``{"events": [...]}``.
    Tout le reste → ``None`` (indéterminé), sans rejet.
    """
    if isinstance(parsed, list):
        return len(parsed)
    if isinstance(parsed, dict) and isinstance(parsed.get("events"), list):
        return len(parsed["events"])
    return None


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
            # Vider le flux avant de lever : sans ça, le corps reste non lu
            # sur la connexion au moment du 413, ce qui peut forcer
            # uvicorn/h11 à couper la connexion keep-alive plutôt que de
            # livrer la réponse proprement (retour de revue #59, round 2).
            async for _ in request.stream():
                pass
            raise _BodyTooLarge

    morceaux: list[bytes] = []
    total = 0
    async for morceau in request.stream():
        total += len(morceau)
        if total > limit:
            # Même raison que la branche Content-Length ci-dessus : vider le
            # reste du flux avant de lever, sinon la connexion garde du corps
            # non lu au moment du 413 (retour de revue #59, round 4, point
            # d'OswinFreyr — cette branche-ci avait été oubliée par le fix du
            # round 2, qui ne couvrait que la branche Content-Length).
            # Starlette marque le flux consommé (`_stream_consumed`) dès que
            # le dernier message ASGI (`more_body=False`) est reçu, AVANT même
            # de le céder à ce `async for` — si tout le corps est arrivé en un
            # seul message (courant avec un petit corps, y compris dans les
            # tests), le flux est donc déjà entièrement consommé ici et
            # rappeler `request.stream()` lève `RuntimeError("Stream
            # consumed")`. C'est le signal qu'il n'y a justement plus rien à
            # vider, pas une vraie erreur.
            try:
                async for _ in request.stream():
                    pass
            except RuntimeError:
                pass
            raise _BodyTooLarge
        morceaux.append(morceau)
    return b"".join(morceaux)


def _store_raw_batch(
    db: LockedConnection,
    batch_id: str,
    body_utf8: str,
    event_count: int | None,
    remote_addr: str | None,
    content_type: str | None,
) -> None:
    """Insère un batch brut sur la connexion partagée de l'app.

    `sqlite3.Connection` n'est pas sûre en accès concurrent depuis plusieurs
    threads : `LockedConnection.execute()` sérialise les écritures issues du
    threadpool. Ce n'est pas une limite de SQLite lui-même (WAL, un seul
    écrivain de toute façon), juste une protection de l'objet Python.
    """
    db.execute(
        "INSERT INTO raw_batches "
        "(batch_id, received_ms, remote_addr, content_type, event_count, body) "
        "VALUES (?, ?, ?, ?, ?, ?)",
        (batch_id, int(time.time() * 1000), remote_addr, content_type, event_count, body_utf8),
    )


def _decode_parse_and_store(
    db: LockedConnection,
    raw: bytes,
    remote_addr: str | None,
    content_type: str | None,
) -> tuple[str, int | None]:
    """Décode, parse puis stocke un batch — le tout dans un seul thread.

    Regroupés en un seul aller-retour threadpool (retour de revue #59, point
    2) : décoder l'UTF-8 et parser le JSON sont le travail CPU-bound dominant
    d'un gros batch, plus coûteux que l'INSERT. Les laisser sur la boucle
    d'événements aurait annulé l'intérêt du passage en thread — y compris pour
    /healthz, qui doit rester rapide pendant ce parsing.

    Décodage AVANT `json.loads` : `json.loads` accepte l'UTF-16/32, mais on
    veut stocker du texte qui round-trip (US-216 vérifiera une signature
    Ed25519 sur ces octets). `CANONICAL.md` impose l'UTF-8 : un corps qui n'en
    est pas est rejeté ici (`UnicodeDecodeError`, remonte à l'appelant).
    """
    text = raw.decode("utf-8")
    parsed = json.loads(text)
    event_count = _guess_event_count(parsed)
    batch_id = str(uuid.uuid4())
    _store_raw_batch(db, batch_id, text, event_count, remote_addr, content_type)
    return batch_id, event_count


@app.post("/ingest/batch", status_code=status.HTTP_202_ACCEPTED)
async def ingest_batch(request: Request) -> JSONResponse:
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
        batch_id, event_count = await run_in_threadpool(
            _decode_parse_and_store,
            request.app.state.db,
            raw,
            request.client.host if request.client else None,
            request.headers.get("content-type"),
        )
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError):
        # RecursionError : un JSON très imbriqué (ex. `"["*2000 + "]"*2000`,
        # quelques Ko) fait planter json.loads sans lever JSONDecodeError —
        # c'est un corps invalide du point de vue de l'API, pas une panne
        # serveur (retour de revue #59, round 2).
        return JSONResponse(
            status_code=status.HTTP_400_BAD_REQUEST,
            content={"error": "corps invalide : un document JSON UTF-8 est attendu"},
        )
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
        content={"stored": True, "batch_id": batch_id, "event_count": event_count},
    )
