"""Point d'entrée FastAPI du dashboard dengon (squelette — US-110).

Deux routes seulement :

* ``GET  /healthz``       — liveness, répond 200.
* ``POST /ingest/batch``  — **ingestion permissive** : accepte n'importe quel
  JSON bien formé et le stocke tel quel dans ``raw_batches``.

La validation de schéma, la vérification de signature Ed25519, la déduplication
sur ``event_id`` et les projections sont l'objet d'US-216 / US-217 (sprint 2).
C'est délibéré : le dashboard doit pouvoir démarrer avant que le format
d'événement soit figé (cf. docs/olivier/proposition-organisation-github.md §3.3).
"""

from __future__ import annotations

import json
import time
import uuid
from contextlib import asynccontextmanager
from typing import Any

from fastapi import FastAPI, Request, status
from fastapi.responses import JSONResponse

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


@app.post("/ingest/batch", status_code=status.HTTP_202_ACCEPTED)
async def ingest_batch(request: Request) -> JSONResponse:
    raw = await request.body()
    try:
        parsed = json.loads(raw)
    except (json.JSONDecodeError, UnicodeDecodeError):
        return JSONResponse(
            status_code=status.HTTP_400_BAD_REQUEST,
            content={"error": "corps invalide : un document JSON est attendu"},
        )

    batch_id = str(uuid.uuid4())
    event_count = _guess_event_count(parsed)

    conn = connect()
    try:
        conn.execute(
            "INSERT INTO raw_batches "
            "(batch_id, received_ms, remote_addr, content_type, event_count, body) "
            "VALUES (?, ?, ?, ?, ?, ?)",
            (
                batch_id,
                int(time.time() * 1000),
                request.client.host if request.client else None,
                request.headers.get("content-type"),
                event_count,
                raw.decode("utf-8", errors="replace"),
            ),
        )
        conn.commit()
    finally:
        conn.close()

    return JSONResponse(
        status_code=status.HTTP_202_ACCEPTED,
        content={"stored": True, "batch_id": batch_id, "event_count": event_count},
    )
