#!/usr/bin/env python3
"""Génère `vectors_generated.h` à partir de `contracts/packet/vectors_v0.json`
(US-307) : des tableaux d'octets C, pas de bibliothèque JSON côté C.

N'écrit **aucun fichier versionné** : régénéré juste avant la compilation du
programme C hôte (`host_test.c`), comme `contracts/tools/build_fixtures.py`
régénère les fixtures Python. Bibliothèque standard uniquement (`json`),
volontairement sans dépendance à `uv`/`contracts/pyproject.toml` : ce script
n'a besoin de rien d'autre pour rester exécutable par un simple
`python3 generate_vectors.py`.
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
VECTORS = ROOT / "contracts" / "packet" / "vectors_v0.json"
OUT = Path(__file__).resolve().parent / "vectors_generated.h"


def c_bytes(name: str, hex_str: str) -> str:
    raw = bytes.fromhex(hex_str)
    body = ", ".join(f"0x{b:02x}" for b in raw)
    return f"static const uint8_t {name}[] = {{{body}}};"


def main() -> int:
    doc = json.loads(VECTORS.read_text(encoding="utf-8"))
    accept = doc["accept"]
    reject = doc["reject"]

    lines = [
        "/* Généré par generate_vectors.py depuis contracts/packet/vectors_v0.json.",
        " * NE PAS ÉDITER À LA MAIN, NE PAS COMMITTER (voir host_test.c). */",
        "#ifndef DENGON_VECTORS_GENERATED_H",
        "#define DENGON_VECTORS_GENERATED_H",
        "",
        "#include <stddef.h>",
        "#include <stdint.h>",
        "",
    ]

    for i, v in enumerate(accept):
        lines.append(c_bytes(f"ACCEPT_{i}_BYTES", v["hex"]))
    lines.append("")
    for i, v in enumerate(reject):
        lines.append(c_bytes(f"REJECT_{i}_BYTES", v["hex"]))
    lines.append("")

    lines.append("typedef struct { const char *name; const uint8_t *bytes; size_t len; } DengonVector;")
    lines.append("")
    lines.append(f"static const DengonVector ACCEPT_VECTORS[{len(accept)}] = {{")
    for i, v in enumerate(accept):
        lines.append(f'    {{"{v["name"]}", ACCEPT_{i}_BYTES, sizeof(ACCEPT_{i}_BYTES)}},')
    lines.append("};")
    lines.append("")
    lines.append(f"static const DengonVector REJECT_VECTORS[{len(reject)}] = {{")
    for i, v in enumerate(reject):
        lines.append(f'    {{"{v["name"]}", REJECT_{i}_BYTES, sizeof(REJECT_{i}_BYTES)}},')
    lines.append("};")
    lines.append("")
    lines.append("#endif /* DENGON_VECTORS_GENERATED_H */")
    lines.append("")

    OUT.write_text("\n".join(lines), encoding="utf-8")
    print(f"{OUT} : {len(accept)} vecteurs accept, {len(reject)} vecteurs reject")
    return 0


if __name__ == "__main__":
    sys.exit(main())
