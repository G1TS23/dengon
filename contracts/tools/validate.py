"""Vérifie les fixtures golden. Exécuté par la CI ; échoue au premier problème.

    uv run python tools/validate.py

Contrôles :
  1. chaque fixture valide contre batch.schema.json (+ envelope.schema.json) ;
  2. chaque `payload` respecte le catalogue (champs requis + types) ;
  3. `event_id` = hex(SHA-256(node_id ‖ seq)) ; `node_id` du batch = celui des events ;
  4. la signature Ed25519 du batch est valide pour la clé de test ;
  5. redaction : aucune clé interdite, tout `msg_log_id` est bien une empreinte 16 hex ;
  6. `events/payloads.schema.json` est à jour vis-à-vis du catalogue ;
  7. les 20 fixtures couvrent tout le catalogue.
"""

from __future__ import annotations

import base64
import json
import sys
from pathlib import Path

from catalogue import CATALOGUE, canonical_json, event_id, payloads_json_schema
from jsonschema import Draft202012Validator
from nacl.exceptions import BadSignatureError
from nacl.signing import VerifyKey
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parent.parent
EVENTS = ROOT / "events"
FIXTURES = EVENTS / "fixtures"

FORBIDDEN_KEYS = {"msg_uuid", "recipient", "text", "body", "plaintext", "message", "content"}

errors: list[str] = []


def fail(fixture: str, msg: str) -> None:
    errors.append(f"{fixture}: {msg}")


def _registry() -> Registry:
    reg = Registry()
    for schema_file in ("envelope.schema.json", "batch.schema.json"):
        schema = json.loads((EVENTS / schema_file).read_text())
        reg = reg.with_resource(schema_file, Resource.from_contents(schema))
        reg = reg.with_resource(schema["$id"], Resource.from_contents(schema))
    return reg


def _walk_keys(obj: object):
    if isinstance(obj, dict):
        for k, v in obj.items():
            yield k
            yield from _walk_keys(v)
    elif isinstance(obj, list):
        for v in obj:
            yield from _walk_keys(v)


def check_payload(fixture: str, name: str, payload: dict) -> None:
    spec = CATALOGUE.get(name)
    if spec is None:
        fail(fixture, f"événement hors catalogue : {name}")
        return
    for field in spec["required"]:
        if field not in payload:
            fail(fixture, f"{name}: champ requis manquant « {field} »")
    validator = Draft202012Validator({"type": "object", "properties": spec["props"]})
    for err in validator.iter_errors(payload):
        fail(fixture, f"{name}: payload invalide — {err.message}")


def check_fixture(path: Path, batch_validator: Draft202012Validator) -> None:
    fx = path.name
    body = json.loads(path.read_text())

    for err in sorted(batch_validator.iter_errors(body), key=str):
        fail(fx, f"schéma batch — {err.message}")

    # redaction
    for key in _walk_keys(body):
        if key in FORBIDDEN_KEYS:
            fail(fx, f"clé interdite (redaction) : « {key} »")

    # signature
    vk = VerifyKey(
        bytes.fromhex(json.loads((EVENTS / "test-signing-key.json").read_text())["public_hex"])
    )
    unsigned = {k: v for k, v in body.items() if k != "sig"}
    try:
        vk.verify(canonical_json(unsigned), base64.b64decode(body["sig"]))
    except (BadSignatureError, KeyError, ValueError) as exc:
        fail(fx, f"signature invalide : {exc}")

    for e in body.get("events", []):
        if e.get("node_id") != body.get("node_id"):
            fail(fx, f"node_id de l'événement {e.get('seq')} ≠ node_id du batch")
        if e.get("event_id") != event_id(e.get("node_id", ""), e.get("seq", -1)):
            fail(fx, f"event_id incohérent pour seq {e.get('seq')}")
        pl = e.get("payload", {})
        for k, v in pl.items():
            if k.endswith("msg_log_id") or k == "msg_log_id":
                if not (
                    isinstance(v, str) and len(v) == 16 and all(c in "0123456789abcdef" for c in v)
                ):
                    fail(fx, f"msg_log_id « {v} » n'est pas une empreinte 16 hex")
        check_payload(fx, e.get("name", ""), pl)


def main() -> int:
    if not (EVENTS / "test-signing-key.json").exists():
        print("test-signing-key.json manquant", file=sys.stderr)
        return 2

    registry = _registry()
    batch_schema = json.loads((EVENTS / "batch.schema.json").read_text())
    batch_validator = Draft202012Validator(batch_schema, registry=registry)

    fixtures = sorted(FIXTURES.glob("*.json"))
    if len(fixtures) != 20:
        fail("(global)", f"{len(fixtures)} fixtures au lieu de 20")

    for path in fixtures:
        check_fixture(path, batch_validator)

    # payloads.schema.json à jour ?
    on_disk = json.loads((EVENTS / "payloads.schema.json").read_text())
    if on_disk != payloads_json_schema():
        fail("(global)", "events/payloads.schema.json est périmé — relancer build_fixtures.py")

    # couverture du catalogue
    covered = {
        e["name"] for path in fixtures for e in json.loads(path.read_text()).get("events", [])
    }
    missing = set(CATALOGUE) - covered
    if missing:
        fail("(global)", f"catalogue non couvert : {sorted(missing)}")

    if errors:
        print(f"✗ {len(errors)} problème(s) :", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1

    print(f"✓ {len(fixtures)} fixtures valides — {len(covered)} noms d'événements couverts.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
