"""Tests de `GET /api/messages` et `GET /api/messages/{id}` (US-219)."""

from __future__ import annotations

import json
from pathlib import Path

from app.projections import group_by_msg_log_id

FIXTURES_DIR = Path(__file__).resolve().parents[3] / "contracts" / "events" / "fixtures"


def _load_fixture_batches() -> list[dict]:
    return [json.loads(path.read_text()) for path in sorted(FIXTURES_DIR.glob("*.json"))]


_TEST_SIGNING_PUB_HEX = "c561fa9f643fe5c60113cce9db282fde2b9e5ca5fc6b6fc0d1679bb339c9f72f"


def _register_and_authorize(client, node_id: str, kind: str) -> dict[str, str]:
    response = client.post(
        "/api/nodes",
        json={"node_id": node_id, "kind": kind, "pub_sign": _TEST_SIGNING_PUB_HEX},
    )
    assert response.status_code == 201, response.text
    return {"Authorization": f"Bearer {response.json()['token']}"}


def _ingest_all_fixtures(client) -> None:
    batches = _load_fixture_batches()
    node_ids = {
        (b["node_id"], "relay" if b["node_id"].startswith("relay-") else "client") for b in batches
    }
    headers_by_node = {
        node_id: _register_and_authorize(client, node_id, kind) for node_id, kind in node_ids
    }
    for batch in batches:
        response = client.post(
            "/ingest/batch", json=batch, headers=headers_by_node[batch["node_id"]]
        )
        assert response.status_code == 202, response.text


def _all_fixture_msg_log_ids() -> set[str]:
    events = []
    for batch in _load_fixture_batches():
        events.extend(batch["events"])
    return set(group_by_msg_log_id(events))


def test_list_messages_is_empty_before_any_ingestion(client):
    response = client.get("/api/messages")
    assert response.status_code == 200
    assert response.json() == []


def test_get_message_returns_404_for_unknown_id(client):
    response = client.get("/api/messages/0000000000000000")
    assert response.status_code == 404


def test_list_messages_after_ingesting_golden_fixtures(client):
    _ingest_all_fixtures(client)

    response = client.get("/api/messages")
    assert response.status_code == 200
    body = response.json()

    returned_ids = {m["msg_log_id"] for m in body}
    assert returned_ids == _all_fixture_msg_log_ids()
    # Triés par activité la plus récente en premier (voir list_messages()).
    last_event_values = [m["last_event_ms"] for m in body]
    assert last_event_values == sorted(last_event_values, reverse=True)


def test_get_message_detail_has_the_expected_shape_and_status(client):
    _ingest_all_fixtures(client)

    response = client.get("/api/messages/1122334455667788")
    assert response.status_code == 200
    body = response.json()

    assert body["msg_log_id"] == "1122334455667788"
    assert body["status"] == "delivered"
    assert body["delivery_latency_ms"] == 880
    assert isinstance(body["hops"], list)
    assert len(body["hops"]) >= 1
    # Chronologique : chaque saut a un ts_ms >= au précédent.
    ts_values = [hop["ts_ms"] for hop in body["hops"]]
    assert ts_values == sorted(ts_values)


def test_get_message_detail_maps_pkt_relayed_to_relay_kind_with_ttl_and_fanout(client):
    _ingest_all_fixtures(client)

    # "4d5e6f7a8b9c0d1e" porte un pkt.relayed (fixture 02) avec
    # ttl_in/ttl_out/fanout renseignés dans le payload.
    response = client.get("/api/messages/4d5e6f7a8b9c0d1e")
    assert response.status_code == 200
    hops = response.json()["hops"]
    relay_hops = [h for h in hops if h["kind"] == "relay"]
    assert len(relay_hops) == 1
    assert relay_hops[0]["ttl_in"] is not None
    assert relay_hops[0]["ttl_out"] is not None
    assert relay_hops[0]["fanout"] is not None


def test_get_message_detail_tolerates_missing_radio_fields(client):
    # "aabbccdd00112233" (fixtures 04/09/14) n'a pas de pkt.relayed — ses
    # événements n'ont donc pas de ttl_in/ttl_out/fanout/rssi : le champ doit
    # être `null`, pas une erreur, pas un champ absent qui ferait planter un
    # client strict.
    _ingest_all_fixtures(client)

    response = client.get("/api/messages/aabbccdd00112233")
    assert response.status_code == 200
    hops = response.json()["hops"]
    assert len(hops) >= 1
    for hop in hops:
        assert "ttl_in" in hop
        assert "ttl_out" in hop
        assert "fanout" in hop
        assert "rssi" in hop


def test_get_message_detail_never_exposes_a_raw_identifier(client):
    # Cohérent avec la redaction (critère d'acceptation US-219) : aucun champ
    # de la réponse ne doit ressembler à un identifiant non tronqué (le
    # contrat impose 8 octets / 16 hex pour tout pseudonyme, voir
    # 03-ecarts-conception.md — un event_id (32 o / 64 hex) ne doit jamais
    # apparaître dans une réponse destinée à l'écran web).
    _ingest_all_fixtures(client)

    response = client.get("/api/messages/1122334455667788")
    body = response.json()
    assert "event_id" not in body
    for hop in body["hops"]:
        assert set(hop.keys()) == {
            "node_id",
            "ts_ms",
            "kind",
            "ttl_in",
            "ttl_out",
            "fanout",
            "rssi",
        }
