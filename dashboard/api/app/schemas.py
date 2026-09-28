"""Chargement du schéma de validation de `POST /ingest/batch` (US-107/US-216).

`contracts/` n'est pas un paquet Python installable : ce module lit les
fichiers `.schema.json` directement depuis le monorepo, exactement comme
`contracts/tools/validate.py` (même patron de `Registry` — deux alias par
schéma, le nom de fichier ET le `$id` complet, pour que les deux formes de
`$ref` rencontrées dans les schémas résolvent toutes les deux).
"""

from __future__ import annotations

import json
from functools import lru_cache
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

# dashboard/api/app/schemas.py -> dashboard/api/app -> dashboard/api ->
# dashboard -> racine du monorepo -> contracts/events.
_CONTRACTS_EVENTS = Path(__file__).resolve().parents[3] / "contracts" / "events"


def _registry() -> Registry:
    reg = Registry()
    for name in ("envelope.schema.json", "batch.schema.json"):
        schema = json.loads((_CONTRACTS_EVENTS / name).read_text())
        reg = reg.with_resource(name, Resource.from_contents(schema))
        reg = reg.with_resource(schema["$id"], Resource.from_contents(schema))
    return reg


@lru_cache(maxsize=1)
def batch_validator() -> Draft202012Validator:
    """Validateur de `contracts/events/batch.schema.json` (résout son `$ref`
    vers `envelope.schema.json`). Mis en cache : reconstruire le validateur
    à chaque requête recompilerait le même schéma en boucle.
    """
    schema = json.loads((_CONTRACTS_EVENTS / "batch.schema.json").read_text())
    return Draft202012Validator(schema, registry=_registry())
