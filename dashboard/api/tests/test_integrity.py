"""Tests de `GET /api/integrity` (US-310).

Construit des batches dont les événements portent `entry_hash`/`prev_hash`/
`sig` (`envelope.schema.json`) formés exactement comme
`dengon_core::ledger::Entry` les calcule (`_MiniLedger` ci-dessous, miroir
Python minimal de `Ledger::append`) — les quatre verdicts sont obtenus en
répliquant les scénarios que `dengon-verify` (US-305) détecte réellement
(`crates/dengon-verify/tests/cli.rs`), pas en les simulant côté Python.

Nécessite le binaire `dengon-verify` du workspace (variable d'environnement
`DENGON_VERIFY_BIN`, voir `app/config.py` — `conftest.py::dengon_verify_bin`
le construit une fois par session si besoin).
"""

from __future__ import annotations

import base64
import hashlib
import json
import struct

import pytest
from nacl.signing import SigningKey
from test_api import _build_signed_batch, _canonical_json, _event_id, _register_node

pytestmark = pytest.mark.usefixtures("dengon_verify_bin")

_GENESIS_HASH = bytes(32)


def _signing_bytes(seq: int, ts_ms: int, name: str, payload_json: bytes, prev_hash: bytes) -> bytes:
    """Même agencement que `ledger::Entry::signing_bytes` (Rust) : entiers
    big-endian, deux champs texte préfixés par leur longueur en `u32`.
    """
    name_b = name.encode("utf-8")
    return (
        struct.pack(">Q", seq)
        + struct.pack(">Q", ts_ms)
        + struct.pack(">I", len(name_b))
        + name_b
        + struct.pack(">I", len(payload_json))
        + payload_json
        + prev_hash
    )


class _MiniLedger:
    """Miroir minimal de `dengon_core::ledger::Ledger::append` — construit
    une chaîne d'entrées valides (`entry_hash`/`prev_hash`/`sig`) pour un
    nœud de test, signées avec sa clé Ed25519 (celle enregistrée via
    `POST /api/nodes`, pour que `--pubkey` les vérifie).
    """

    def __init__(self, signing_key: SigningKey) -> None:
        self._signing_key = signing_key
        self._prev_hash = _GENESIS_HASH

    def append(self, seq: int, ts_ms: int, name: str, payload: dict) -> dict:
        payload_json = _canonical_json(payload)
        entry_hash = hashlib.sha256(
            _signing_bytes(seq, ts_ms, name, payload_json, self._prev_hash)
        ).digest()
        sig = self._signing_key.sign(entry_hash).signature
        entry = {
            "seq": seq,
            "ts_ms": ts_ms,
            "name": name,
            "payload": payload,
            "prev_hash": self._prev_hash.hex(),
            "entry_hash": entry_hash.hex(),
            "sig": base64.b64encode(sig).decode("ascii"),
        }
        self._prev_hash = entry_hash
        return entry


def _payload(n: int) -> dict:
    # Forme valide de `msg.queued` (payloads.schema.json) — le contenu exact
    # n'importe pas pour ces tests, seul compte qu'il varie assez pour ne pas
    # produire deux `entry_hash` identiques par accident.
    return {"msg_log_id": f"{n:016x}", "conv_hash": "0011223344556677"}


def _batch_with_ledger_entries(node_id: str, signing_key: SigningKey, entries: list[dict]) -> dict:
    """Construit un batch valide (US-216) à partir d'entrées `_MiniLedger` —
    pas `_build_signed_batch` : celle-ci impose un `ts_ms` UNIQUE pour tout
    le batch, alors que chaque entrée de chaîne a le sien (déjà figé dans
    `entry_hash` au moment de `_MiniLedger.append`).
    """
    node_kind = "relay" if node_id.startswith("relay-") else "client"
    built_events = [
        {
            "event_id": _event_id(node_id, entry["seq"]),
            "node_id": node_id,
            "node_kind": node_kind,
            "seq": entry["seq"],
            "ts_ms": entry["ts_ms"],
            "name": entry["name"],
            "payload": entry["payload"],
            "entry_hash": entry["entry_hash"],
            "prev_hash": entry["prev_hash"],
            **({"sig": entry["sig"]} if "sig" in entry else {}),
        }
        for entry in entries
    ]
    batch_id = hashlib.sha256(_canonical_json(built_events)).hexdigest()
    unsigned = {
        "batch_id": batch_id,
        "node_id": node_id,
        "schema_version": 1,
        "events": built_events,
    }
    signature = signing_key.sign(_canonical_json(unsigned)).signature
    return {**unsigned, "sig": base64.b64encode(signature).decode("ascii")}


def _post_ledger_batch(client, node_id, signing_key, token, entries):
    batch = _batch_with_ledger_entries(node_id, signing_key, entries)
    response = client.post(
        "/ingest/batch", json=batch, headers={"Authorization": f"Bearer {token}"}
    )
    assert response.status_code == 202, response.text
    return response


def _insert_raw_events(node_id: str, entries: list[dict]) -> None:
    """Insère des lignes `events` directement (contourne `/ingest/batch` et
    la dédup par `event_id`) — pour un scénario que le pipeline normal ne
    peut pas produire, voir `test_verdict_fork_on_two_entries_for_the_same_seq`.
    `event_id` est fabriqué distinct par ligne (suffixé), sans lien avec la
    vraie formule `SHA-256(node_id ‖ seq)` : ce test ne passe pas par la
    dédup, donc peu importe qu'il ne la respecte pas.
    """
    from app.db import connect

    conn = connect()
    try:
        for i, entry in enumerate(entries):
            conn.execute(
                "INSERT INTO events "
                "(event_id, ts_ms, node_id, name, seq, payload, entry_hash, prev_hash, sig) "
                "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (
                    f"{entry['entry_hash']}{i:02x}"[:64],
                    entry["ts_ms"],
                    node_id,
                    entry["name"],
                    entry["seq"],
                    json.dumps(entry["payload"], sort_keys=True, separators=(",", ":")),
                    bytes.fromhex(entry["entry_hash"]),
                    bytes.fromhex(entry["prev_hash"]),
                    base64.b64decode(entry["sig"]) if "sig" in entry else None,
                ),
            )
        conn.commit()
    finally:
        conn.close()


def _verdict_for(client, node_id: str) -> dict:
    body = client.get("/api/integrity").json()
    matches = [v for v in body if v["node_id"] == node_id]
    assert len(matches) == 1, f"{node_id} absent ou dupliqué dans {body}"
    return matches[0]


def test_integrity_is_empty_before_any_node_registered(client):
    response = client.get("/api/integrity")
    assert response.status_code == 200
    assert response.json() == []


def test_unverified_when_no_event_carries_entry_hash(client):
    # Batch normal (US-216), sans les champs de chaîne du tout — le cas
    # réel actuel : aucun producteur ne les envoie encore.
    signing_key, token = _register_node(client, node_id="relay-3f2a9c")
    batch = _build_signed_batch("relay-3f2a9c", signing_key)
    response = client.post(
        "/ingest/batch", json=batch, headers={"Authorization": f"Bearer {token}"}
    )
    assert response.status_code == 202, response.text

    verdict = _verdict_for(client, "relay-3f2a9c")
    assert verdict["verdict"] == "unverified"
    assert verdict["entries"] == 0
    assert verdict["first_seq"] is None
    assert verdict["signatures"] == "unchecked"


def test_verdict_ok_on_an_intact_chain_with_verified_signatures(client):
    node_id = "relay-3f2a9c"
    signing_key, token = _register_node(client, node_id=node_id)
    ledger = _MiniLedger(signing_key)
    entries = [
        ledger.append(0, 1_725_800_000_000, "msg.queued", _payload(0)),
        ledger.append(1, 1_725_800_000_100, "msg.queued", _payload(1)),
        ledger.append(2, 1_725_800_000_200, "msg.queued", _payload(2)),
    ]
    _post_ledger_batch(client, node_id, signing_key, token, entries)

    verdict = _verdict_for(client, node_id)
    assert verdict["verdict"] == "ok"
    assert verdict["entries"] == 3
    assert verdict["first_seq"] == 0
    assert verdict["last_seq"] == 2
    assert verdict["signatures"] == "verified"


def test_verdict_broken_when_a_payload_is_tampered_after_hashing(client):
    # Le payload transmis diverge de celui réellement haché à la création
    # de l'entrée (seq 1) : `entry_hash` recalculé par dengon-verify ne
    # correspond plus à celui annoncé — altération classique.
    node_id = "relay-3f2a9c"
    signing_key, token = _register_node(client, node_id=node_id)
    ledger = _MiniLedger(signing_key)
    entries = [
        ledger.append(0, 1_725_800_000_000, "msg.queued", _payload(0)),
        ledger.append(1, 1_725_800_000_100, "msg.queued", _payload(1)),
        ledger.append(2, 1_725_800_000_200, "msg.queued", _payload(2)),
    ]
    entries[1]["payload"] = _payload(999)  # falsifié après le calcul du hash
    _post_ledger_batch(client, node_id, signing_key, token, entries)

    verdict = _verdict_for(client, node_id)
    assert verdict["verdict"] == "broken"


def test_verdict_fork_on_two_entries_for_the_same_seq(client):
    # `event_id = SHA-256(node_id ‖ seq)` (US-216) est déterministe : une
    # resoumission pour la même position (node_id, seq), même avec un
    # contenu différent, produit le MÊME `event_id` et se fait donc
    # dédupliquer par `INSERT OR IGNORE` avant même d'atteindre la
    # vérification de chaîne — un vrai fork ne peut pas naître de
    # `POST /ingest/batch` avec le schéma actuel (écart consigné,
    # `03-ecarts-conception.md`, US-310). On insère donc les deux lignes en
    # conflit directement (contournant `event_id` comme clé), pour tester
    # que `GET /api/integrity` détecte bien un fork déjà présent dans la
    # table — le scénario réaliste étant un export tiers inséré hors de ce
    # pipeline, pas une resoumission normale.
    node_id = "relay-3f2a9c"
    signing_key, _token = _register_node(client, node_id=node_id)
    ledger = _MiniLedger(signing_key)
    e0 = ledger.append(0, 1_725_800_000_000, "msg.queued", _payload(0))
    e1 = ledger.append(1, 1_725_800_000_100, "msg.queued", _payload(1))
    ledger_bis = _MiniLedger(signing_key)
    e0_bis = ledger_bis.append(0, 1_725_800_000_000, "msg.queued", _payload(42))
    _insert_raw_events(node_id, [e0, e1, e0_bis])

    verdict = _verdict_for(client, node_id)
    assert verdict["verdict"] == "fork"


def test_verdict_gap_when_a_position_is_missing(client):
    node_id = "relay-3f2a9c"
    signing_key, token = _register_node(client, node_id=node_id)
    ledger = _MiniLedger(signing_key)
    e0 = ledger.append(0, 1_725_800_000_000, "msg.queued", _payload(0))
    ledger.append(1, 1_725_800_000_100, "msg.queued", _payload(1))  # jamais transmise
    e2 = ledger.append(2, 1_725_800_000_200, "msg.queued", _payload(2))
    _post_ledger_batch(client, node_id, signing_key, token, [e0, e2])

    verdict = _verdict_for(client, node_id)
    assert verdict["verdict"] == "gap"


def test_signatures_unchecked_when_some_entries_lack_a_signature(client):
    node_id = "relay-3f2a9c"
    signing_key, token = _register_node(client, node_id=node_id)
    ledger = _MiniLedger(signing_key)
    entries = [
        ledger.append(0, 1_725_800_000_000, "msg.queued", _payload(0)),
        ledger.append(1, 1_725_800_000_100, "msg.queued", _payload(1)),
    ]
    del entries[1]["sig"]  # entrée sans signature (schéma : optionnelle)
    _post_ledger_batch(client, node_id, signing_key, token, entries)

    verdict = _verdict_for(client, node_id)
    # La chaîne reste intacte (entry_hash/prev_hash présents et cohérents) :
    # seule la vérification de SIGNATURE est désactivée, pas le verdict de
    # chaîne — une entrée non signée ne doit pas se faire passer, à tort,
    # pour une signature invalide.
    assert verdict["verdict"] == "ok"
    assert verdict["signatures"] == "unchecked"


def test_two_nodes_get_independent_verdicts(client):
    node_a = "relay-aaaaaa"
    node_b = "relay-bbbbbb"
    key_a, token_a = _register_node(client, node_id=node_a)
    key_b, token_b = _register_node(client, node_id=node_b)

    ledger_a = _MiniLedger(key_a)
    _post_ledger_batch(
        client, node_a, key_a, token_a,
        [ledger_a.append(0, 1_725_800_000_000, "msg.queued", _payload(0))],
    )

    ledger_b = _MiniLedger(key_b)
    e0_b = ledger_b.append(0, 1_725_800_000_000, "msg.queued", _payload(0))
    e1_b = ledger_b.append(1, 1_725_800_000_100, "msg.queued", _payload(1))
    _post_ledger_batch(client, node_b, key_b, token_b, [e0_b])  # e1_b jamais envoyée : gap latent
    _post_ledger_batch(client, node_b, key_b, token_b, [e1_b])

    assert _verdict_for(client, node_a)["verdict"] == "ok"
    # node_b a bien reçu les deux entrées (deux batches), donc chaîne intacte
    # elle aussi — ce test vérifie surtout l'ISOLATION : le verdict de A ne
    # dépend pas des événements de B, et réciproquement.
    assert _verdict_for(client, node_b)["verdict"] == "ok"
    assert _verdict_for(client, node_b)["entries"] == 2
