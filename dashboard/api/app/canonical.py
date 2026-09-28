"""JSON canonique — miroir Python de `contracts/tools/catalogue.py`.

`contracts/` n'est pas un paquet Python installable (pas de nom de paquet
distribuable, juste un projet `uv` pour ses propres outils/tests) : plutôt
qu'une dépendance inter-paquets fragile, ce module réimplémente les deux
fonctions dont `dashboard/api` a besoin pour vérifier une signature
Ed25519 côté serveur. `contracts/events/CANONICAL.md` **fait foi** pour les
deux implémentations (Rust `dengon-core::observability`, Python
`contracts/tools/catalogue.py` et ce module) — elles doivent produire les
mêmes octets indépendamment, jamais l'une en copiant l'autre à l'exécution.
"""

from __future__ import annotations

import hashlib
import json


def canonical_json(obj: object) -> bytes:
    """Forme canonique signée : clés triées, séparateurs compacts, UTF-8,
    pas de NaN — voir `contracts/events/CANONICAL.md` §1.
    """
    return json.dumps(
        obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False
    ).encode("utf-8")


def event_id(node_id: str, seq: int) -> str:
    """`hex(SHA-256(node_id ‖ seq_big_endian_64))` — `docs/powl/08` §1."""
    return hashlib.sha256(node_id.encode("ascii") + seq.to_bytes(8, "big")).hexdigest()
