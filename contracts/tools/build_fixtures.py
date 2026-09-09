"""Génère les 20 fixtures golden signées + `events/payloads.schema.json`.

Les fichiers produits sont **committés** : la CI (`validate.py`) les vérifie,
elle ne les régénère pas. Rejouer ce script après un changement de catalogue,
puis committer le diff.

    uv run python tools/build_fixtures.py
"""

from __future__ import annotations

import base64
import hashlib
import json
from pathlib import Path

from catalogue import CATALOGUE, canonical_json, event_id, payloads_json_schema
from nacl.signing import SigningKey

ROOT = Path(__file__).resolve().parent.parent
EVENTS = ROOT / "events"
FIXTURES = EVENTS / "fixtures"

# Nœuds de test (pseudonymes cohérents avec le pattern node_id).
RELAY, RELAY2 = "relay-3f2a9c", "relay-7b41d0"
CLIENT, CLIENT2 = "client-9c1d84", "client-a2f60b"

# Empreintes de test : 16 hex (msg_log_id, peerID tronqué), 32 hex (recipient_tag).
M1, M2, M3 = "4d5e6f7a8b9c0d1e", "1122334455667788", "aabbccdd00112233"
CONV1 = "0011223344556677"
TAG1 = "00112233445566778899aabbccddeeff"
PEER1, PEER2 = "a1b2c3d4e5f60718", "0f1e2d3c4b5a6978"
ROOT_A, ROOT_B = "9f" * 32, "1a" * 32

T = 1_725_800_000_000  # base d'horodatage (ms)
R, C = "relay", "client"


def _signing_key() -> SigningKey:
    seed = json.loads((EVENTS / "test-signing-key.json").read_text())["seed_hex"]
    return SigningKey(bytes.fromhex(seed))


def ev(node: str, kind: str, seq: int, ts_ms: int, name: str, payload: dict) -> dict:
    assert name in CATALOGUE, f"événement hors catalogue : {name}"
    return {"event_id": event_id(node, seq), "node_id": node, "node_kind": kind,
            "seq": seq, "ts_ms": ts_ms, "name": name, "payload": payload}


def batch(node: str, events: list[dict], sk: SigningKey, schema_version: int = 1) -> dict:
    body = {
        "batch_id": hashlib.sha256(canonical_json(events)).hexdigest(),
        "node_id": node,
        "schema_version": schema_version,
        "events": events,
    }
    body["sig"] = base64.b64encode(sk.sign(canonical_json(body)).signature).decode("ascii")
    return body


def build(sk: SigningKey) -> list[tuple[str, dict]]:
    b = lambda node, events: batch(node, events, sk)  # noqa: E731
    return [
        ("01-pkt-seen", b(RELAY, [
            ev(RELAY, R, 1001, T + 10, "pkt.seen",
               {"msg_log_id": M1, "type": 3, "ttl_in": 7, "size_bucket": 512,
                "from_peer": PEER1, "rssi": -63})])),
        ("02-pkt-duplicate-relayed", b(RELAY, [
            ev(RELAY, R, 1002, T + 20, "pkt.duplicate", {"msg_log_id": M1, "from_peer": PEER2}),
            ev(RELAY, R, 1003, T + 25, "pkt.relayed",
               {"msg_log_id": M1, "type": 3, "ttl_in": 7, "ttl_out": 6, "fanout": 2, "from_peer": PEER1})])),
        ("03-pkt-delivered-local", b(CLIENT, [
            ev(CLIENT, C, 5001, T + 40, "pkt.delivered_local", {"msg_log_id": M2, "type": 3})])),
        ("04-pkt-dropped", b(RELAY, [
            ev(RELAY, R, 1004, T + 50, "pkt.dropped", {"msg_log_id": M3, "reason": "ttl_zero"}),
            ev(RELAY, R, 1005, T + 55, "pkt.dropped", {"reason": "quota"})])),
        ("05-pkt-rejected", b(RELAY, [
            ev(RELAY, R, 1006, T + 60, "pkt.rejected", {"from_peer": PEER2, "reason": "bad_sig"})])),
        ("06-envelope-stored", b(RELAY, [
            ev(RELAY, R, 1007, T + 70, "envelope.stored",
               {"msg_log_id": M1, "recipient_tag": TAG1, "epoch_day": 20340, "copy_budget": 4})])),
        ("07-envelope-offered-handoff", b(RELAY, [
            ev(RELAY, R, 1008, T + 80, "envelope.offered", {"count": 3, "to_peer": PEER1}),
            ev(RELAY, R, 1009, T + 85, "envelope.handoff",
               {"msg_log_id": M1, "recipient_tag": TAG1, "to_peer": PEER1, "budget_after": 3})])),
        ("08-envelope-delivered", b(RELAY2, [
            ev(RELAY2, R, 2001, T + 90, "envelope.delivered",
               {"msg_log_id": M1, "recipient_tag": TAG1, "to_peer": PEER2})])),
        ("09-envelope-expired", b(RELAY, [
            ev(RELAY, R, 1010, T + 100, "envelope.expired",
               {"msg_log_id": M3, "recipient_tag": TAG1, "reason": "budget_zero"})])),
        ("10-msg-queued", b(CLIENT, [
            ev(CLIENT, C, 5002, T + 110, "msg.queued", {"msg_log_id": M2, "conv_hash": CONV1})])),
        ("11-msg-handed-off", b(CLIENT, [
            ev(CLIENT, C, 5003, T + 120, "msg.handed_off", {"msg_log_id": M2, "to_peer": PEER1, "via": "session"}),
            ev(CLIENT, C, 5004, T + 900, "msg.handed_off", {"msg_log_id": M2, "to_peer": PEER2, "via": "envelope"})])),
        ("12-msg-received", b(CLIENT2, [
            ev(CLIENT2, C, 7001, T + 950, "msg.received", {"msg_log_id": M2, "via": "envelope"})])),
        ("13-msg-delivered", b(CLIENT, [
            ev(CLIENT, C, 5005, T + 1000, "msg.delivered", {"msg_log_id": M2, "latency_ms": 880})])),
        ("14-msg-expired", b(CLIENT, [
            ev(CLIENT, C, 5006, T + 90_000, "msg.expired", {"msg_log_id": M3})])),
        ("15-ack-observed", b(RELAY, [
            ev(RELAY, R, 1011, T + 1010, "ack.observed", {"msg_log_id": M2})])),
        ("16-peer-connected-disconnected", b(RELAY, [
            ev(RELAY, R, 1012, T + 1100, "peer.connected", {"peer": PEER1, "rssi": -58, "role": "peripheral"}),
            ev(RELAY, R, 1013, T + 5100, "peer.disconnected", {"peer": PEER1, "duration_s": 4, "pkt_exchanged": 11})])),
        ("17-peer-announce-seen", b(RELAY, [
            ev(RELAY, R, 1014, T + 1200, "peer.announce_seen",
               {"peer": PEER2, "pseudo": "kiosque-hall-B", "ledger_height": 88123, "caps": 3})])),
        ("18-attest", b(RELAY, [
            ev(RELAY, R, 1015, T + 1300, "attest.emitted", {"root": ROOT_A, "height": 1015}),
            ev(RELAY, R, 1016, T + 1305, "attest.observed",
               {"subject_node": CLIENT, "root": ROOT_B, "height": 5000})])),
        ("19-relay-health", b(RELAY, [
            ev(RELAY, R, 1017, T + 1400, "relay.boot",
               {"fw_version": "0.1.0", "reset_reason": "power_on", "secure_boot": False, "flash_enc": False}),
            ev(RELAY, R, 1018, T + 61_400, "relay.health",
               {"uptime_s": 60, "rssi_avg": -61, "peers": 2, "cache_size": 128, "envelope_store": 3,
                "log_buffer_pct": 12, "logs_dropped": 0, "heap_free": 190_000}),
            ev(RELAY, R, 1019, T + 62_000, "relay.wifi_up", {"ssid": "campus-ap", "duration_s": 0}),
            ev(RELAY, R, 1020, T + 92_000, "relay.wifi_down", {"duration_s": 30}),
            ev(RELAY, R, 1021, T + 92_100, "relay.overloaded",
               {"subsystem": "envelope_store", "action": "refuse_envelope"})])),
        ("20-client-summary", b(CLIENT, [
            ev(CLIENT, C, 5007, T + 120_000, "client.summary",
               {"msgs_sent": 4, "msgs_recv": 2, "peers_seen": 5, "app_version": "0.1.0"})])),
    ]


def main() -> None:
    sk = _signing_key()
    FIXTURES.mkdir(parents=True, exist_ok=True)

    fixtures = build(sk)
    assert len(fixtures) == 20, f"{len(fixtures)} fixtures au lieu de 20"

    covered = {e["name"] for _, body in fixtures for e in body["events"]}
    missing = set(CATALOGUE) - covered
    assert not missing, f"événements non couverts : {sorted(missing)}"

    for name, body in fixtures:
        (FIXTURES / f"{name}.json").write_text(
            json.dumps(body, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
    (EVENTS / "payloads.schema.json").write_text(
        json.dumps(payloads_json_schema(), indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(f"{len(fixtures)} fixtures écrites, {len(covered)} noms d'événements couverts.")


if __name__ == "__main__":
    main()
