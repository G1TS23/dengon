"""Tests de `GET /api/stream` (US-218).

`starlette.testclient.TestClient` ne convient PAS pour ces tests : sa
`handle_request()` fait tourner la coroutine ASGI complète jusqu'à sa fin
avant de rendre la main (`portal.call(self.app, scope, receive, send)`,
`starlette/testclient.py`) — elle bufferise toute la réponse plutôt que de
la streamer progressivement. `/api/stream` ne se termine jamais tant que le
client ne se déconnecte pas : avec `TestClient`, `client.stream(...)`
resterait bloqué indéfiniment à l'entrée du `with`, avant même de lire un
octet (confirmé en pratique : un test écrit ainsi ne se termine jamais).

Les tests de streaming ici lancent donc un **vrai serveur `uvicorn`** sur
`127.0.0.1` (port choisi par l'OS) dans un thread, et s'y connectent avec un
`httpx.Client` réel — un vrai socket TCP lit les octets au fur et à mesure
qu'ils arrivent, sans attendre la fin de la réponse. Toujours dans le même
processus que le test : `app.state.broadcaster` reste directement
inspectable (voir `_wait_until_subscribed`).
"""

from __future__ import annotations

import base64
import hashlib
import json
import queue
import threading
import time

import httpx
import pytest
import uvicorn
from nacl.signing import SigningKey

from app.stream import Broadcaster, StreamEvent


@pytest.fixture
def live_server(tmp_path, monkeypatch):
    monkeypatch.setenv("DENGON_DASHBOARD_DB", str(tmp_path / "sse.db"))
    monkeypatch.setenv("DENGON_DASHBOARD_JWT_SECRET", "test-secret-ne-jamais-utiliser-en-prod")

    from app.main import app

    config = uvicorn.Config(app, host="127.0.0.1", port=0, log_level="warning")
    server = uvicorn.Server(config)
    thread = threading.Thread(target=server.run, daemon=True)
    thread.start()

    deadline = time.monotonic() + 5.0
    while not server.started and time.monotonic() < deadline:
        time.sleep(0.01)
    if not server.started:
        raise RuntimeError("uvicorn n'a pas démarré à temps")

    port = server.servers[0].sockets[0].getsockname()[1]
    try:
        yield f"http://127.0.0.1:{port}", app
    finally:
        server.should_exit = True
        thread.join(timeout=5.0)


def _register_node(http: httpx.Client, node_id: str = "relay-3f2a9c", kind: str = "relay"):
    signing_key = SigningKey.generate()
    response = http.post(
        "/api/nodes",
        json={"node_id": node_id, "kind": kind, "pub_sign": signing_key.verify_key.encode().hex()},
    )
    assert response.status_code == 201, response.text
    return signing_key, response.json()["token"]


def _canonical_json(obj: object) -> bytes:
    return json.dumps(
        obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False
    ).encode("utf-8")


def _event_id(node_id: str, seq: int) -> str:
    return hashlib.sha256(node_id.encode("ascii") + seq.to_bytes(8, "big")).hexdigest()


def _build_signed_batch(
    node_id: str, signing_key: SigningKey, *, seq: int, name: str, payload: dict
):
    node_kind = "relay" if node_id.startswith("relay-") else "client"
    event = {
        "event_id": _event_id(node_id, seq),
        "node_id": node_id,
        "node_kind": node_kind,
        "seq": seq,
        "ts_ms": 1_725_800_000_000 + seq,
        "name": name,
        "payload": payload,
    }
    batch_id = hashlib.sha256(_canonical_json([event])).hexdigest()
    unsigned = {"batch_id": batch_id, "node_id": node_id, "schema_version": 1, "events": [event]}
    signature = signing_key.sign(_canonical_json(unsigned)).signature
    return {**unsigned, "sig": base64.b64encode(signature).decode("ascii")}


def _ingest_one(http, signing_key, token, node_id, *, seq, name="pkt.seen", payload=None):
    batch = _build_signed_batch(
        node_id,
        signing_key,
        seq=seq,
        name=name,
        payload=payload
        or {
            "msg_log_id": "1122334455667788",
            "type": 1,
            "ttl_in": 5,
            "size_bucket": 256,
            "from_peer": "a1b2c3d4",
        },
    )
    response = http.post("/ingest/batch", json=batch, headers={"Authorization": f"Bearer {token}"})
    assert response.status_code == 202, response.text
    return response.json()


def _parse_sse_events(text: str) -> list[dict]:
    events = []
    current_id = None
    for line in text.split("\n"):
        if line.startswith("id: "):
            current_id = int(line[len("id: ") :])
        elif line.startswith("data: "):
            data = json.loads(line[len("data: ") :])
            events.append({"id": current_id, **data})
    return events


def _read_first_event(response: httpx.Response) -> dict:
    """Lit le flux SSE jusqu'au premier bloc `data:` complet (délimité par
    une ligne vide) et le renvoie. Un `: keep-alive` isolé (commentaire SSE,
    voir `_HEARTBEAT_SSE` dans `app/main.py`) est ignoré plutôt que retourné
    comme un événement.
    """
    buffer = ""
    for chunk in response.iter_text():
        buffer += chunk
        while "\n\n" in buffer:
            block, buffer = buffer.split("\n\n", 1)
            events = _parse_sse_events(block + "\n\n")
            if events:
                return events[0]
    raise AssertionError("flux terminé sans qu'aucun événement ne soit reçu")


def _wait_until(predicate, *, timeout: float = 5.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.01)
    raise AssertionError("condition jamais vraie avant le délai")


# --- StreamEvent.to_sse() (pure, pas de serveur) ----------------------------


def test_stream_event_formats_as_valid_sse():
    event = StreamEvent(
        rowid=42,
        event_id="abc",
        ts_ms=100,
        node_id="relay-3f2a9c",
        name="pkt.seen",
        payload={"msg_log_id": "1122"},
    )
    text = event.to_sse()
    assert text.startswith("id: 42\n")
    assert "event: pkt.seen\n" in text
    assert text.endswith("\n\n")
    assert json.loads(text.split("data: ", 1)[1].strip()) == {
        "event_id": "abc",
        "ts_ms": 100,
        "node_id": "relay-3f2a9c",
        "name": "pkt.seen",
        "payload": {"msg_log_id": "1122"},
    }


# --- Broadcaster (pure asyncio, pas de serveur) -----------------------------


def test_broadcaster_publish_delivers_to_all_subscribers():
    import asyncio

    async def scenario():
        broadcaster = Broadcaster(asyncio.get_running_loop())
        q1 = broadcaster.subscribe()
        q2 = broadcaster.subscribe()
        event = StreamEvent(
            rowid=1, event_id="a", ts_ms=1, node_id="relay-x", name="pkt.seen", payload={}
        )
        broadcaster.publish([event])
        assert await asyncio.wait_for(q1.get(), timeout=1) == event
        assert await asyncio.wait_for(q2.get(), timeout=1) == event
        broadcaster.unsubscribe(q1)
        broadcaster.unsubscribe(q2)
        assert broadcaster.subscriber_count() == 0

    asyncio.run(scenario())


def test_broadcaster_publish_with_no_subscribers_does_not_raise():
    import asyncio

    async def scenario():
        Broadcaster(asyncio.get_running_loop()).publish(
            [StreamEvent(rowid=1, event_id="a", ts_ms=1, node_id="n", name="x", payload={})]
        )

    asyncio.run(scenario())


# --- GET /api/stream, serveur réel ------------------------------------------


def test_stream_catches_up_on_already_ingested_events(live_server):
    base_url, _app = live_server
    with httpx.Client(base_url=base_url) as http:
        signing_key, token = _register_node(http)
        _ingest_one(http, signing_key, token, "relay-3f2a9c", seq=1)

        with http.stream("GET", "/api/stream") as response:
            assert response.status_code == 200
            assert response.headers["content-type"].startswith("text/event-stream")
            event = _read_first_event(response)

    assert event["name"] == "pkt.seen"
    assert event["id"] == 1


def test_stream_reconnection_with_last_event_id_only_replays_newer_events(live_server):
    # "reconnexion client gérée (le client reprend sans perdre le fil)" : un
    # client qui se reconnecte avec Last-Event-ID = id du premier événement
    # ne doit PAS le revoir, seulement les événements suivants.
    base_url, _app = live_server
    with httpx.Client(base_url=base_url) as http:
        signing_key, token = _register_node(http)
        _ingest_one(http, signing_key, token, "relay-3f2a9c", seq=1)

        with http.stream("GET", "/api/stream") as first:
            first_event = _read_first_event(first)
        last_id = first_event["id"]

        _ingest_one(http, signing_key, token, "relay-3f2a9c", seq=2)

        with http.stream("GET", "/api/stream", headers={"Last-Event-ID": str(last_id)}) as second:
            second_event = _read_first_event(second)

    assert second_event["id"] == last_id + 1


def test_stream_with_unparseable_last_event_id_replays_from_the_start(live_server):
    base_url, _app = live_server
    with httpx.Client(base_url=base_url) as http:
        signing_key, token = _register_node(http)
        _ingest_one(http, signing_key, token, "relay-3f2a9c", seq=1)

        with http.stream(
            "GET", "/api/stream", headers={"Last-Event-ID": "pas-un-entier"}
        ) as response:
            event = _read_first_event(response)

    assert event["id"] == 1


def test_stream_delivers_a_live_event_to_a_connected_client(live_server):
    """Critère d'acceptation : « un batch ingéré produit un message SSE reçu
    par un client de test ». Exerce le VRAI chemin `Broadcaster.publish`
    (pas seulement le rattrapage) : le client se connecte avant l'ingestion ;
    on attend son inscription (poll borné sur `subscriber_count()`, pas un
    `sleep` fixe) avant de publier l'événement qu'il doit recevoir en direct.
    """
    base_url, app = live_server
    received: queue.Queue = queue.Queue()

    def consume():
        with httpx.Client(base_url=base_url) as http, http.stream("GET", "/api/stream") as response:
            received.put(_read_first_event(response))

    thread = threading.Thread(target=consume, daemon=True)
    thread.start()
    try:
        _wait_until(lambda: app.state.broadcaster.subscriber_count() >= 1)
        with httpx.Client(base_url=base_url) as http:
            signing_key, token = _register_node(http)
            _ingest_one(http, signing_key, token, "relay-3f2a9c", seq=1, name="pkt.duplicate")
        event = received.get(timeout=5)
    finally:
        thread.join(timeout=5)

    assert event["name"] == "pkt.duplicate"
    assert event["id"] == 1
