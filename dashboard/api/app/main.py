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

from .db import connect, run_migrations


@asynccontextmanager
async def lifespan(_: FastAPI):
    conn = connect()
    try:
        run_migrations(conn)
    finally:
        conn.close()
    yield


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


def _store_raw_batch(
    batch_id: str,
    body_utf8: str,
    event_count: int | None,
    remote_addr: str | None,
    content_type: str | None,
) -> None:
    """Insère un batch brut. Synchrone : appelé dans un thread par la route."""
    conn = connect()
    try:
        conn.execute(
            "INSERT INTO raw_batches "
            "(batch_id, received_ms, remote_addr, content_type, event_count, body) "
            "VALUES (?, ?, ?, ?, ?, ?)",
            (batch_id, int(time.time() * 1000), remote_addr, content_type, event_count, body_utf8),
        )
    finally:
        conn.close()


@app.post("/ingest/batch", status_code=status.HTTP_202_ACCEPTED)
async def ingest_batch(request: Request) -> JSONResponse:
    raw = await request.body()
    try:
        # Décodage AVANT json.loads : json.loads accepte l'UTF-16/32, mais on
        # veut stocker du texte qui round-trip (US-216 vérifiera une signature
        # Ed25519 sur ces octets). CANONICAL.md impose l'UTF-8 : un corps qui
        # n'est pas de l'UTF-8 est rejeté ici.
        text = raw.decode("utf-8")
        parsed = json.loads(text)
    except (UnicodeDecodeError, json.JSONDecodeError):
        return JSONResponse(
            status_code=status.HTTP_400_BAD_REQUEST,
            content={"error": "corps invalide : un document JSON UTF-8 est attendu"},
        )

    batch_id = str(uuid.uuid4())
    event_count = _guess_event_count(parsed)

    try:
        # I/O SQLite bloquante → thread, pour ne pas geler la boucle d'événements
        # (une sonde de liveness sur /healthz doit rester rapide).
        await run_in_threadpool(
            _store_raw_batch,
            batch_id,
            text,
            event_count,
            request.client.host if request.client else None,
            request.headers.get("content-type"),
        )
    except sqlite3.OperationalError:
        # SQLite n'a qu'un écrivain : sous deux flush simultanés, le perdant
        # échoue après le busy_timeout. Le nœud doit retenter.
        return JSONResponse(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            content={"error": "stockage momentanément occupé, réessayer"},
            headers={"Retry-After": "1"},
        )

    return JSONResponse(
        status_code=status.HTTP_202_ACCEPTED,
        content={"stored": True, "batch_id": batch_id, "event_count": event_count},
    )
