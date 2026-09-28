"""Diffusion des événements ingérés aux clients connectés en SSE (US-218).

Un `Broadcaster` par processus, posé sur `app.state` au démarrage (comme
`LockedConnection`) : un ensemble d'abonnés (`asyncio.Queue`, un par
connexion `GET /api/stream` ouverte), chacun recevant une copie de chaque
`StreamEvent` publié.

Le pont thread synchrone → boucle asyncio est le point délicat : l'ingestion
(`app/ingest.py::ingest_batch`) tourne dans le threadpool FastAPI
(`run_in_threadpool`), donc dans un thread qui n'est PAS celui de la boucle
asyncio qui sert les connexions SSE. `asyncio.Queue.put_nowait` n'est pas
thread-safe — l'appeler directement depuis le threadpool corromprait l'état
interne de la queue sous contention. `loop.call_soon_threadsafe` est le seul
mécanisme prévu par asyncio pour programmer un appel sur la boucle depuis un
autre thread.
"""

from __future__ import annotations

import asyncio
import json
from dataclasses import dataclass


@dataclass(frozen=True)
class StreamEvent:
    """Un événement tel que diffusé en SSE — reflète une ligne de `events`.

    `rowid` sert d'identifiant SSE (`id:`) : c'est le `rowid` SQLite de la
    ligne dans `events`, un entier strictement croissant à l'insertion, donc
    un curseur de reprise naturel pour `Last-Event-ID` (voir
    `app/main.py::stream_events`) — pas besoin d'une colonne dédiée.
    """

    rowid: int
    event_id: str
    ts_ms: int
    node_id: str
    name: str
    payload: dict

    def to_sse(self) -> str:
        data = json.dumps(
            {
                "event_id": self.event_id,
                "ts_ms": self.ts_ms,
                "node_id": self.node_id,
                "name": self.name,
                "payload": self.payload,
            },
            separators=(",", ":"),
        )
        # `event:` = le nom de domaine.action (ex. "msg.queued") : un client
        # JS peut s'abonner par type d'événement (`addEventListener(name,
        # ...)`) sans reparser `data` pour filtrer côté client.
        return f"id: {self.rowid}\nevent: {self.name}\ndata: {data}\n\n"


class Broadcaster:
    def __init__(self, loop: asyncio.AbstractEventLoop) -> None:
        self._loop = loop
        self._subscribers: set[asyncio.Queue[StreamEvent]] = set()

    def subscribe(self) -> asyncio.Queue[StreamEvent]:
        queue: asyncio.Queue[StreamEvent] = asyncio.Queue()
        self._subscribers.add(queue)
        return queue

    def unsubscribe(self, queue: asyncio.Queue[StreamEvent]) -> None:
        self._subscribers.discard(queue)

    def subscriber_count(self) -> int:
        return len(self._subscribers)

    def publish(self, events: list[StreamEvent]) -> None:
        """Appelable depuis N'IMPORTE QUEL thread — voir la docstring du
        module. Copie `self._subscribers` avant d'itérer : un abonné peut se
        désinscrire (fin de connexion) pendant l'itération, depuis la boucle
        asyncio, concurremment à cet appel venu du threadpool.
        """
        if not events:
            return
        for queue in list(self._subscribers):
            for event in events:
                self._loop.call_soon_threadsafe(queue.put_nowait, event)
