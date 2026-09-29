"""US-311 : `GET /api/nodes` (flotte + alertes) et `GET /api/network/graph`.

Les fixtures golden de `contracts/events/fixtures/` datent de
`ts_ms = 1725800000000` : on fixe « maintenant » par rapport à cette date pour
déclencher (ou non) l'alerte `relay_silent` — scénario 4 du DoD : un relais
qui se tait est signalé.
"""

import pytest
from conftest import _ingest_fixture_batches, _load_fixture_batches

RELAY = "relay-3f2a9c"
T0 = 1_725_800_000_000
# Dernier `relay.health` des fixtures : T0 + 61_400 (voir 19-relay-health.json).
HEALTH_MS = T0 + 61_400
# Dernier événement du relais (`relay.overloaded`) : plus récent que la santé.
LAST_MS = T0 + 92_100


@pytest.fixture
def ingested(client):
    _ingest_fixture_batches(client, _load_fixture_batches())
    return client


def _at(monkeypatch, now_ms: int) -> None:
    monkeypatch.setattr("app.main.time.time", lambda: now_ms / 1000)


def _relay(body: dict) -> dict:
    return next(n for n in body["nodes"] if n["node_id"] == RELAY)


def test_fleet_is_empty_on_a_fresh_database(client):
    body = client.get("/api/nodes").json()
    assert body["nodes"] == []
    assert body["silent_after_ms"] == 5 * 60_000


def test_fleet_exposes_relay_health_and_version(ingested, monkeypatch):
    _at(monkeypatch, LAST_MS + 60_000)
    relay = _relay(ingested.get("/api/nodes").json())
    assert relay["kind"] == "relay"
    assert relay["fw_version"] == "0.1.0"
    assert relay["last_contact_ms"] == LAST_MS
    assert relay["health"]["uptime_s"] == 60
    assert relay["health"]["rssi_avg"] == -61
    assert relay["status"] == "online"
    assert relay["alerts"] == []


def test_silent_relay_is_flagged_after_the_threshold(ingested, monkeypatch):
    _at(monkeypatch, LAST_MS + 5 * 60_000 + 1)
    relay = _relay(ingested.get("/api/nodes").json())
    assert relay["status"] == "stale"
    assert relay["alerts"] == [{"code": "relay_silent", "since_ms": LAST_MS}]


def test_relay_still_emitting_events_is_not_silent(ingested, monkeypatch):
    # Plus de `relay.health` frais depuis > 5 min, mais un événement récent.
    _at(monkeypatch, HEALTH_MS + 5 * 60_000 + 1)
    relay = _relay(ingested.get("/api/nodes").json())
    assert relay["status"] == "online"
    assert relay["alerts"] == []


def test_relay_exactly_at_the_threshold_is_not_yet_silent(ingested, monkeypatch):
    _at(monkeypatch, LAST_MS + 5 * 60_000)
    assert _relay(ingested.get("/api/nodes").json())["alerts"] == []


def test_silence_threshold_is_configurable(ingested, monkeypatch):
    monkeypatch.setenv("DENGON_DASHBOARD_RELAY_SILENT_MINUTES", "1")
    _at(monkeypatch, LAST_MS + 90_000)
    body = ingested.get("/api/nodes").json()
    assert body["silent_after_ms"] == 60_000
    assert _relay(body)["status"] == "stale"


def test_relay_without_any_health_event_is_partial_not_an_error(client, monkeypatch):
    from conftest import _register_and_authorize

    _register_and_authorize(client, "relay-aaaaaa", "relay")
    _at(monkeypatch, T0)
    relay = client.get("/api/nodes").json()["nodes"][0]
    assert relay["health"] is None
    assert relay["last_contact_ms"] is None
    assert [a["code"] for a in relay["alerts"]] == ["relay_silent"]


def test_graph_links_relay_to_the_peers_it_saw(ingested, monkeypatch):
    _at(monkeypatch, HEALTH_MS)
    graph = ingested.get("/api/network/graph").json()
    ids = {n["id"]: n for n in graph["nodes"]}
    assert ids[RELAY]["kind"] == "relay"
    assert ids["a1b2c3d4e5f60718"]["kind"] == "peer"
    link = next(
        link_
        for link_ in graph["links"]
        if link_["source"] == RELAY and link_["target"] == "a1b2c3d4e5f60718"
    )
    # La fixture 16 connecte puis déconnecte ce pair : lien connu mais inactif.
    assert link["active"] is False
    assert link["sessions"] == 1
    assert link["rssi"] == -58


def test_graph_of_an_empty_network_is_empty(client):
    graph = client.get("/api/network/graph").json()
    assert graph["nodes"] == []
    assert graph["links"] == []


def test_invalid_silence_threshold_env_is_rejected(monkeypatch):
    from app.config import relay_silent_ms

    monkeypatch.setenv("DENGON_DASHBOARD_RELAY_SILENT_MINUTES", "abc")
    with pytest.raises(RuntimeError):
        relay_silent_ms()
    monkeypatch.setenv("DENGON_DASHBOARD_RELAY_SILENT_MINUTES", "0")
    with pytest.raises(RuntimeError):
        relay_silent_ms()
