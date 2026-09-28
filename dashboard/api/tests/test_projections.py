"""Tests de `app/projections.py` (US-217) — logique pure, sans base ni API.

Les scénarios `msg_1122…`/`msg_aabb…`/`msg_4d5e…` rejouent volontairement les
20 fixtures golden de US-107 (`contracts/events/fixtures/`, mêmes
`msg_log_id`) : ce module teste la logique isolée, `test_api.py` teste le
même résultat obtenu via le vrai pipeline HTTP (`POST /ingest/batch`).
"""

from __future__ import annotations

import json
import random
from pathlib import Path

import pytest

from app.projections import group_by_msg_log_id, project_message, project_messages

FIXTURES_DIR = Path(__file__).resolve().parents[3] / "contracts" / "events" / "fixtures"


def _all_fixture_events() -> list[dict]:
    events = []
    for path in sorted(FIXTURES_DIR.glob("*.json")):
        batch = json.loads(path.read_text())
        events.extend(batch["events"])
    return events


def _event(name: str, ts_ms: int, **payload) -> dict:
    return {"name": name, "ts_ms": ts_ms, "payload": payload}


# --- Règles de base (docs/synthese/09 §10) ---------------------------------


def test_queued_only_gives_queued_status():
    events = [_event("msg.queued", 100, msg_log_id="a", conv_hash="c")]
    projection = project_message("a", events)
    assert projection.status == "queued"
    assert projection.conv_hash == "c"
    assert projection.first_seen_ms == 100
    assert projection.last_event_ms == 100


def test_pkt_relayed_gives_in_flight_status_and_counts_hops():
    events = [
        _event("pkt.relayed", 100, msg_log_id="a"),
        _event("pkt.relayed", 200, msg_log_id="a"),
    ]
    projection = project_message("a", events)
    assert projection.status == "in_flight"
    assert projection.hop_count == 2


def test_msg_delivered_gives_delivered_status_with_latency():
    events = [
        _event("msg.queued", 100, msg_log_id="a"),
        _event("msg.delivered", 300, msg_log_id="a", latency_ms=200),
    ]
    projection = project_message("a", events)
    assert projection.status == "delivered"
    assert projection.delivery_latency_ms == 200
    assert projection.status_ms == 300


def test_ack_observed_also_gives_delivered_status():
    events = [_event("ack.observed", 150, msg_log_id="a")]
    assert project_message("a", events).status == "delivered"


def test_msg_expired_gives_expired_status():
    events = [_event("msg.expired", 400, msg_log_id="a")]
    assert project_message("a", events).status == "expired"


def test_no_events_gives_unknown_status():
    projection = project_message("a", [])
    assert projection.status == "unknown"


# --- Tolérance au désordre et aux données partielles (critères US-217) ----


def test_delivered_wins_over_a_late_out_of_order_expired_event():
    # Le message a bien été délivré ; un `msg.expired` arrivé en retard (ou
    # dans le désordre) ne doit pas écraser ce statut — §10 : "Expiré : ...
    # et PAS de delivered".
    events = [
        _event("msg.queued", 100, msg_log_id="a"),
        _event("msg.expired", 50, msg_log_id="a"),  # ts antérieur, mais reçu après
        _event("msg.delivered", 300, msg_log_id="a"),
    ]
    assert project_message("a", events).status == "delivered"


@pytest.mark.parametrize("seed", range(5))
def test_status_is_independent_of_arrival_order(seed):
    events = [
        _event("msg.queued", 100, msg_log_id="a", conv_hash="c"),
        _event("msg.handed_off", 150, msg_log_id="a"),
        _event("pkt.relayed", 180, msg_log_id="a"),
        _event("msg.delivered", 300, msg_log_id="a", latency_ms=200),
    ]
    shuffled = events[:]
    random.Random(seed).shuffle(shuffled)
    in_order = project_message("a", events)
    out_of_order = project_message("a", shuffled)
    assert in_order == out_of_order


def test_partial_data_one_missing_node_still_yields_a_status():
    # "tient avec des données partielles (un nœud n'a pas remonté) sans
    # planter ni inventer" : ici, seul le relais a remonté (pkt.relayed) —
    # aucun événement de l'émetteur (msg.queued/handed_off/delivered).
    events = [_event("pkt.relayed", 100, msg_log_id="a")]
    projection = project_message("a", events)
    assert projection.status == "in_flight"
    assert projection.delivery_latency_ms is None


def test_duplicate_events_are_idempotent():
    events = [_event("msg.delivered", 100, msg_log_id="a", latency_ms=50)]
    once = project_message("a", events)
    twice = project_message("a", events + events)
    assert once == twice


# --- group_by_msg_log_id ----------------------------------------------------


def test_group_by_msg_log_id_ignores_events_without_one():
    events = [
        _event("msg.queued", 100, msg_log_id="a"),
        _event("peer.connected", 100, peer="deadbeef"),
    ]
    grouped = group_by_msg_log_id(events)
    assert list(grouped) == ["a"]


# --- Fixtures golden de US-107 ----------------------------------------------


def test_all_20_golden_fixtures_project_without_crashing():
    events = _all_fixture_events()
    assert len(events) >= 20  # au moins un événement par fixture
    projections = project_messages(events)
    assert projections  # au moins un msg_log_id projeté


def test_golden_fixtures_reconstruct_the_expected_message_lifecycle():
    # msg_log_id "1122334455667788" : pkt.delivered_local (03),
    # msg.queued (10), msg.handed_off ×2 (11), msg.received (12),
    # msg.delivered (13), ack.observed (15) → delivered.
    projections = project_messages(_all_fixture_events())

    delivered = projections["1122334455667788"]
    assert delivered.status == "delivered"
    assert delivered.conv_hash == "0011223344556677"
    assert delivered.delivery_latency_ms == 880

    # "aabbccdd00112233" : pkt.dropped (04), envelope.expired (09),
    # msg.expired (14) → expired.
    expired = projections["aabbccdd00112233"]
    assert expired.status == "expired"

    # "4d5e6f7a8b9c0d1e" : pkt.seen/duplicate/relayed (01/02),
    # envelope.stored/handoff/delivered (06/07/08), aucun événement
    # msg.*/ack.observed → en circulation (in_flight), pas "delivered" —
    # envelope.delivered n'est pas un signal de statut de message au sens de
    # §10 (seuls msg.delivered/ack.observed le sont).
    in_flight = projections["4d5e6f7a8b9c0d1e"]
    assert in_flight.status == "in_flight"
    assert in_flight.hop_count == 1


@pytest.mark.parametrize("seed", range(5))
def test_golden_fixtures_reconstruction_is_order_independent(seed):
    events = _all_fixture_events()
    shuffled = events[:]
    random.Random(seed).shuffle(shuffled)
    assert project_messages(events) == project_messages(shuffled)


# --- Bout en bout : les 20 fixtures ingérées via le vrai pipeline HTTP ------
#
# Les fixtures sont signées avec `contracts/events/test-signing-key.json` —
# une seule paire de clés pour les 4 node_id qui y apparaissent (vérifié :
# `SigningKey(bytes.fromhex(seed_hex)).verify_key == bytes.fromhex(public_hex)`).

_TEST_SIGNING_PUB_HEX = "c561fa9f643fe5c60113cce9db282fde2b9e5ca5fc6b6fc0d1679bb339c9f72f"


def _register_and_authorize(client, node_id: str, kind: str) -> dict[str, str]:
    response = client.post(
        "/api/nodes",
        json={"node_id": node_id, "kind": kind, "pub_sign": _TEST_SIGNING_PUB_HEX},
    )
    assert response.status_code == 201, response.text
    return {"Authorization": f"Bearer {response.json()['token']}"}


def _load_fixture_batches() -> list[dict]:
    return [json.loads(path.read_text()) for path in sorted(FIXTURES_DIR.glob("*.json"))]


def _ingest_fixture_batches(client, batches: list[dict]) -> None:
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


def test_golden_fixtures_ingested_via_http_reconstruct_message_status(client):
    """Preuve d'intégration bout en bout, en plus des tests unitaires
    ci-dessus : les 20 fixtures **réelles** (mêmes signatures que
    `contracts/tools/validate.py`) traversent `/api/nodes` puis
    `/ingest/batch`, la projection `messages` se lit ensuite en base.
    """
    _ingest_fixture_batches(client, _load_fixture_batches())

    from app.db import connect

    conn = connect()
    try:
        row = conn.execute(
            "SELECT status, delivery_latency_ms, hop_count FROM messages WHERE msg_log_id = ?",
            ("1122334455667788",),
        ).fetchone()
        assert row["status"] == "delivered"
        assert row["delivery_latency_ms"] == 880

        row = conn.execute(
            "SELECT status FROM messages WHERE msg_log_id = ?", ("aabbccdd00112233",)
        ).fetchone()
        assert row["status"] == "expired"

        row = conn.execute(
            "SELECT status, hop_count FROM messages WHERE msg_log_id = ?",
            ("4d5e6f7a8b9c0d1e",),
        ).fetchone()
        assert row["status"] == "in_flight"
        assert row["hop_count"] == 1
    finally:
        conn.close()


def test_golden_fixtures_ingested_in_reverse_order_give_the_same_final_status(client):
    # "delivered" (13) ingéré AVANT "queued" (10) pour le même msg_log_id :
    # le statut final doit être le même qu'à l'ingestion dans l'ordre normal
    # (critère US-217 : "les événements arrivant dans le désordre donnent le
    # même résultat final").
    batches = _load_fixture_batches()
    _ingest_fixture_batches(client, list(reversed(batches)))

    from app.db import connect

    conn = connect()
    try:
        row = conn.execute(
            "SELECT status, delivery_latency_ms FROM messages WHERE msg_log_id = ?",
            ("1122334455667788",),
        ).fetchone()
    finally:
        conn.close()
    assert row["status"] == "delivered"
    assert row["delivery_latency_ms"] == 880
