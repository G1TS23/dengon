"""Vérifie les fixtures golden. Exécuté par la CI ; échoue au premier problème.

    uv run python tools/validate.py

Contrôles, par fixture :
  1. valide contre batch.schema.json (+ envelope.schema.json) ;
  2. redaction : aucune clé interdite ;
  3. signature Ed25519 du batch valide pour la clé de test ;
  4. `batch_id` == hex(SHA-256(canonical_json(events))) ;
  5. par événement : `node_id` cohérent, `event_id` recalculé, `msg_log_id`
     empreinte 16 hex, `payload` conforme au catalogue **et** au
     `payloads.schema.json` réellement livré (celui que consomme US-217).
Contrôles globaux :
  6. `payloads.schema.json` à jour vis-à-vis du catalogue ;
  7. les 20 fixtures couvrent tout le catalogue.
"""

from __future__ import annotations

import base64
import hashlib
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

GLOBAL = "(global)"
FORBIDDEN_KEYS = {"msg_uuid", "recipient", "text", "body", "plaintext", "message", "content"}
_HEX = set("0123456789abcdef")

errors: list[str] = []


def fail(where: str, msg: str) -> None:
    errors.append(f"{where}: {msg}")


def _is_hex16(value: object) -> bool:
    return isinstance(value, str) and len(value) == 16 and set(value) <= _HEX


def _batch_registry() -> Registry:
    reg = Registry()
    for schema_file in ("envelope.schema.json", "batch.schema.json"):
        schema = json.loads((EVENTS / schema_file).read_text())
        reg = reg.with_resource(schema_file, Resource.from_contents(schema))
        reg = reg.with_resource(schema["$id"], Resource.from_contents(schema))
    return reg


def _verify_key() -> VerifyKey:
    pub = json.loads((EVENTS / "test-signing-key.json").read_text())["public_hex"]
    return VerifyKey(bytes.fromhex(pub))


def _walk_keys(obj: object):
    if isinstance(obj, dict):
        for key, value in obj.items():
            yield key
            yield from _walk_keys(value)
    elif isinstance(obj, list):
        for value in obj:
            yield from _walk_keys(value)


def _check_schema(fx: str, body: dict, batch_validator: Draft202012Validator) -> None:
    for err in sorted(batch_validator.iter_errors(body), key=str):
        fail(fx, f"schéma batch — {err.message}")


def _check_redaction(fx: str, body: dict) -> None:
    for key in _walk_keys(body):
        if key in FORBIDDEN_KEYS:
            fail(fx, f"clé interdite (redaction) : « {key} »")


def _check_signature(fx: str, body: dict, verify_key: VerifyKey) -> None:
    unsigned = {k: v for k, v in body.items() if k != "sig"}
    try:
        verify_key.verify(canonical_json(unsigned), base64.b64decode(body["sig"]))
    except (BadSignatureError, KeyError, ValueError) as exc:
        fail(fx, f"signature invalide : {exc}")


def _check_batch_id(fx: str, body: dict) -> None:
    expected = hashlib.sha256(canonical_json(body.get("events", []))).hexdigest()
    if body.get("batch_id") != expected:
        fail(fx, "batch_id ≠ hex(SHA-256(canonical_json(events)))")


# Un seul Draft202012Validator par nom d'événement : le schéma est identique
# pour tous les events qui partagent ce nom (~28 events sur 20 fixtures) — le
# reconstruire à chaque appel recompile le même schéma en boucle (retour de
# revue #60, point perf).
_payload_validators: dict[str, Draft202012Validator] = {}


def _payload_validator(name: str, spec: dict) -> Draft202012Validator:
    validator = _payload_validators.get(name)
    if validator is None:
        validator = Draft202012Validator({"type": "object", "properties": spec["props"]})
        _payload_validators[name] = validator
    return validator


def _check_payload_vs_catalogue(fx: str, name: str, payload: dict) -> None:
    spec = CATALOGUE.get(name)
    if spec is None:
        fail(fx, f"événement hors catalogue : {name}")
        return
    for field in spec["required"]:
        if field not in payload:
            fail(fx, f"{name}: champ requis manquant « {field} »")
    for err in _payload_validator(name, spec).iter_errors(payload):
        fail(fx, f"{name}: payload invalide (catalogue) — {err.message}")


def _valid_seq(seq: object) -> bool:
    # bool est une sous-classe d'int en Python : True/False passeraient
    # isinstance(seq, int) alors que ce n'est pas un seq valide.
    return isinstance(seq, int) and not isinstance(seq, bool) and seq >= 0


def _check_event(
    fx: str, body: dict, event: dict, payloads_validator: Draft202012Validator
) -> None:
    if event.get("node_id") != body.get("node_id"):
        fail(fx, f"node_id de l'événement {event.get('seq')} ≠ node_id du batch")

    seq = event.get("seq")
    if not _valid_seq(seq):
        # Déjà signalé par _check_schema (envelope.schema.json exige seq
        # entier ≥ 0) : on n'essaie pas de recalculer event_id sur une valeur
        # qui ferait planter seq.to_bytes() (OverflowError si négatif,
        # AttributeError si ce n'est pas un entier) — un rapport de
        # validation propre, pas une traceback brute (retour de revue #60).
        fail(fx, f"seq invalide ({seq!r}) : event_id non vérifiable")
    elif event.get("event_id") != event_id(event.get("node_id", ""), seq):
        fail(fx, f"event_id incohérent pour seq {seq}")

    name = event.get("name", "")
    payload = event.get("payload", {})
    for key, value in payload.items():
        if key.endswith("msg_log_id") and not _is_hex16(value):
            fail(fx, f"msg_log_id « {value} » n'est pas une empreinte 16 hex")

    _check_payload_vs_catalogue(fx, name, payload)
    # Confronte l'événement au schéma JSON RÉELLEMENT livré (US-217 le consomme) :
    # un bug dans payloads_json_schema() est ainsi attrapé par les fixtures.
    for err in payloads_validator.iter_errors({"name": name, "payload": payload}):
        fail(fx, f"{name}: payloads.schema.json — {err.message}")


def check_fixture(
    path: Path,
    batch_validator: Draft202012Validator,
    payloads_validator: Draft202012Validator,
    verify_key: VerifyKey,
) -> None:
    fx = path.name
    body = json.loads(path.read_text())

    _check_schema(fx, body, batch_validator)
    _check_redaction(fx, body)
    _check_signature(fx, body, verify_key)
    _check_batch_id(fx, body)
    for event in body.get("events", []):
        _check_event(fx, body, event, payloads_validator)


def _check_schema_freshness() -> None:
    on_disk = json.loads((EVENTS / "payloads.schema.json").read_text())
    if on_disk != payloads_json_schema():
        fail(GLOBAL, "events/payloads.schema.json est périmé — relancer build_fixtures.py")


def _check_catalogue_coverage(fixtures: list[Path]) -> None:
    covered = {
        e["name"] for path in fixtures for e in json.loads(path.read_text()).get("events", [])
    }
    missing = set(CATALOGUE) - covered
    if missing:
        fail(GLOBAL, f"catalogue non couvert : {sorted(missing)}")


def main() -> int:
    if not (EVENTS / "test-signing-key.json").exists():
        print("test-signing-key.json manquant", file=sys.stderr)
        return 2

    batch_validator = Draft202012Validator(
        json.loads((EVENTS / "batch.schema.json").read_text()), registry=_batch_registry()
    )
    payloads_validator = Draft202012Validator(
        json.loads((EVENTS / "payloads.schema.json").read_text())
    )
    verify_key = _verify_key()

    fixtures = sorted(FIXTURES.glob("*.json"))
    if len(fixtures) != 20:
        fail(GLOBAL, f"{len(fixtures)} fixtures au lieu de 20")

    for path in fixtures:
        check_fixture(path, batch_validator, payloads_validator, verify_key)

    _check_schema_freshness()
    _check_catalogue_coverage(fixtures)

    if errors:
        print(f"✗ {len(errors)} problème(s) :", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        return 1

    covered = len({e["name"] for p in fixtures for e in json.loads(p.read_text())["events"]})
    print(f"✓ {len(fixtures)} fixtures valides — {covered} noms d'événements couverts.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
