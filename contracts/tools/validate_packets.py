"""Vérifie les vecteurs de trame L3 (`packet/vectors_v0.json`) — patte Python
du job CI `cross-vectors` (US-222).

    uv run python tools/validate_packets.py

Le décodeur ci-dessous est écrit **indépendamment** du Rust : il ne fait que
suivre `header_layout_be` et `flag_bits`, les deux clés que le fichier de
vecteurs expose justement pour qu'un lecteur n'ait pas à recopier la spec.
C'est ce qui rend le contrôle utile — si les deux implémentations étaient
dérivées l'une de l'autre, les comparer ne prouverait rien.

Côté Rust, les mêmes octets sont relus deux fois :
`crates/dengon-core/tests/protocol_vectors.rs` (avec `std`) et
`crates/dengon-conformance/tests/packet_vectors_nostd.rs` (sans `std`, la
configuration du firmware). Fait foi :
`docs/powl/03-network-protocol.md`, résumé dans
`docs/synthese/05-protocole-et-trame.md`.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

PACKET = Path(__file__).resolve().parent.parent / "packet"
VECTORS = PACKET / "vectors_v0.json"

PROTO_VERSION = 1

# Décalages fixes de l'en-tête, communs à tous les paquets — `header_layout_be`.
VERSION_OFF = 0
TYPE_OFF = 1
TTL_OFF = 2
FLAGS_OFF = 3
TIMESTAMP_OFF = 4
SENDER_OFF = 12
RECIPIENT_OFF = 20

PEER_ID_LEN = 8
SIGNATURE_LEN = 64
HEADER_LEN_BROADCAST = RECIPIENT_OFF + 2  # … + payload_len sur 2 octets
HEADER_LEN_ADDRESSED = RECIPIENT_OFF + PEER_ID_LEN + 2

errors: list[str] = []


def fail(where: str, msg: str) -> None:
    errors.append(f"{where} : {msg}")


class Rejet(Exception):
    """Le paquet viole une règle du format."""


def decode(raw: bytes, flag_bits: dict[str, int], type_names: dict[int, str]) -> dict:
    """Décodage de référence. Lève `Rejet` sur tout paquet non conforme.

    Ne connaît que `header_layout_be`, `flag_bits` et `type_names` du fichier
    de vecteurs — jamais une table recopiée du Rust, qui dériverait en
    silence.
    """
    if len(raw) < HEADER_LEN_BROADCAST:
        raise Rejet(f"en-tête tronqué : {len(raw)} octets < {HEADER_LEN_BROADCAST}")

    version = raw[VERSION_OFF]
    if version != PROTO_VERSION:
        raise Rejet(f"version {version:#04x} != PROTO_VERSION ({PROTO_VERSION})")

    type_id = raw[TYPE_OFF]
    if type_id not in type_names:
        raise Rejet(f"type {type_id:#04x} inconnu")

    # Les bits réservés sont IGNORÉS à la réception, pas motif de rejet
    # (`docs/synthese/05` §… « compatibilité ascendante ») : on les masque,
    # exactement comme `Flags::from_bits_truncate` côté Rust.
    flags = raw[FLAGS_OFF] & ~flag_bits["RESERVED_MASK"] & 0xFF
    addressed = bool(flags & flag_bits["ADDRESSED"])
    signed = bool(flags & flag_bits["SIGNED"])
    fragment = bool(flags & flag_bits["FRAGMENT"])

    header_len = HEADER_LEN_ADDRESSED if addressed else HEADER_LEN_BROADCAST
    if len(raw) < header_len:
        raise Rejet(f"en-tête tronqué : {len(raw)} octets < {header_len} (paquet adressé)")

    payload_len = int.from_bytes(raw[header_len - 2 : header_len], "big")
    attendu = header_len + payload_len + (SIGNATURE_LEN if signed else 0)
    if len(raw) != attendu:
        raise Rejet(f"longueur {len(raw)} != {attendu} attendus (payload_len={payload_len}, signed={signed})")

    return {
        "version": version,
        "type": type_id,
        "type_name": type_names[type_id],
        "ttl": raw[TTL_OFF],
        "flags": flags,
        "addressed": addressed,
        "signed": signed,
        "fragment": fragment,
        "timestamp_ms": int.from_bytes(raw[TIMESTAMP_OFF:SENDER_OFF], "big"),
        "sender_id": raw[SENDER_OFF : SENDER_OFF + PEER_ID_LEN].hex(),
        "recipient_id": (raw[RECIPIENT_OFF : RECIPIENT_OFF + PEER_ID_LEN].hex() if addressed else None),
        "payload_len": payload_len,
        "payload": raw[header_len : header_len + payload_len].hex(),
    }


def check_accept(vec: dict, flag_bits: dict[str, int], type_names: dict[int, str]) -> None:
    name = vec["name"]
    try:
        got = decode(bytes.fromhex(vec["hex"]), flag_bits, type_names)
    except Rejet as exc:
        fail(name, f"vecteur accept refusé par le décodeur Python : {exc}")
        return
    for champ, attendu in vec["expect"].items():
        if got[champ] != attendu:
            fail(name, f"{champ} : Python lit {got[champ]!r}, la fixture annonce {attendu!r}")


def check_reject(vec: dict, flag_bits: dict[str, int], type_names: dict[int, str]) -> None:
    name = vec["name"]
    try:
        decode(bytes.fromhex(vec["hex"]), flag_bits, type_names)
    except Rejet:
        return
    fail(name, f"accepté par le décodeur Python alors qu'il viole « {vec.get('reject', '?')} »")


def check_layout(doc: dict) -> None:
    """Les décalages codés ci-dessus doivent correspondre à `header_layout_be`.

    Sans ce contrôle, modifier la disposition dans le fichier de vecteurs sans
    toucher ce script laisserait les deux silencieusement désynchronisés — le
    décodeur continuerait de lire les anciennes positions et tous les
    `expect` seraient recalculés faux, mais cohérents entre eux.
    """
    layout = doc["header_layout_be"]
    for champ, debut in (
        ("version", VERSION_OFF),
        ("type", TYPE_OFF),
        ("ttl", TTL_OFF),
        ("flags", FLAGS_OFF),
        ("timestamp_ms", TIMESTAMP_OFF),
        ("sender_id", SENDER_OFF),
        ("recipient_id", RECIPIENT_OFF),
    ):
        motif = f"{champ}[{debut}:"
        if motif not in layout:
            fail("header_layout_be", f"« {motif}… » absent — validate_packets.py lit {champ} au mauvais décalage")


def main() -> int:
    if not VECTORS.exists():
        print(f"✗ {VECTORS} introuvable", file=sys.stderr)
        return 1
    doc = json.loads(VECTORS.read_text())
    flag_bits = doc["flag_bits"]
    # Les clés JSON sont des chaînes ; le décodeur indexe par octet.
    type_names = {int(k): v for k, v in doc["type_names"].items()}

    check_layout(doc)

    accept = doc["accept"]
    reject = doc["reject"]
    if len(accept) < 8:
        fail("accept", f"au moins 8 vecteurs attendus, {len(accept)} présents")
    if len(reject) < 5:
        fail("reject", f"au moins 5 vecteurs attendus, {len(reject)} présents")

    for vec in accept:
        check_accept(vec, flag_bits, type_names)
    for vec in reject:
        check_reject(vec, flag_bits, type_names)

    if errors:
        print(f"✗ {len(errors)} problème(s) :", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        return 1

    print(f"✓ {len(accept)} vecteurs accept + {len(reject)} reject lus par le décodeur Python de référence.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
