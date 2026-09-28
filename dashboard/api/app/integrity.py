"""Vérification d'intégrité du journal chaîné, par nœud — US-310.

`GET /api/integrity` (voir `app/main.py`) reconstruit, pour chaque nœud, la
suite binaire d'entrées `ledger::Entry::to_bytes()` que `dengon-verify`
(US-305, `crates/dengon-verify`) sait lire, et l'appelle **en sous-processus**
— la règle de vérification reste celle de `dengon_core::ledger`, jamais
réécrite ici (A-5/B-5).

Reconstruction, pas transmission brute : `POST /ingest/batch` ne transporte
que du JSON (`contracts/events/envelope.schema.json`), jamais l'export
binaire directement. Chaque événement porte, **optionnellement**,
`entry_hash`/`prev_hash` (`ledger::Entry.entry_hash`/`.prev_hash`, formule
binaire — pas le JSON canonique) et `sig` (signature Ed25519 de cette entrée
seule). Un événement sans les deux (`entry_hash` ET `prev_hash`) n'entre pas
dans l'export de son nœud — écart consigné dans `03-ecarts-conception.md`
(US-305) : reconstruire depuis les colonnes plutôt que transmettre l'export
brut est le choix retenu pour US-310.

`payload_json` (le texte réellement haché par `ledger::Entry::compute_hash`
côté nœud) n'est **jamais stocké tel quel** : il est re-dérivé à la demande
via `canonical_json(json.loads(events.payload))`. Ça marche seulement parce
que tout appelant de `ledger.append()` dans `dengon-core` lui passe déjà la
forme canonique du payload (`Value::to_canonical_bytes()`,
`contracts/events/CANONICAL.md` fait foi pour les deux implémentations) — si
un producteur envoyait un jour un payload dont le JSON haché sur l'appareil
n'était pas déjà canonique, la reconstruction ne retomberait pas sur les
mêmes octets et casserait le hash à tort (`Broken` pour une entrée en fait
intacte). Aucun producteur réel n'existe encore pour ce contrat (écran QR/
firmware sont des bouchons) : à vérifier quand l'un l'atteindra pour de vrai.
"""

from __future__ import annotations

import json
import struct
import subprocess
from dataclasses import dataclass
from sqlite3 import Row

from .canonical import canonical_json
from .config import dengon_verify_bin
from .db import LockedConnection

_HASH_LEN = 32
_SIG_LEN = 64
_TIMEOUT_S = 10

VERDICT_UNVERIFIED = "unverified"


class IntegrityCheckError(Exception):
    """`dengon-verify` introuvable, ou a échoué à s'exécuter/lire l'export."""


@dataclass(frozen=True)
class NodeIntegrity:
    """Un verdict par nœud — la forme rendue par `GET /api/integrity`."""

    node_id: str
    verdict: str  # "ok" | "broken" | "fork" | "gap" | "unverified"
    entries: int
    first_seq: int | None
    last_seq: int | None
    signatures: str  # "verified" | "invalid" | "unchecked"


def _entry_bytes(row: Row, payload_json: bytes, sig: bytes) -> bytes:
    """`ledger::Entry::to_bytes()` reconstruit — mêmes longueurs, même
    ordre, même endianness (`crates/dengon-core/src/ledger.rs`) :
    ``seq(8) ‖ ts_ms(8) ‖ len(name)(4) ‖ name ‖ len(payload)(4) ‖ payload
    ‖ prev_hash(32) ‖ entry_hash(32) ‖ sig(64)``, entiers **big-endian**.
    """
    name = row["name"].encode("utf-8")
    return (
        struct.pack(">Q", row["seq"])
        + struct.pack(">Q", row["ts_ms"])
        + struct.pack(">I", len(name))
        + name
        + struct.pack(">I", len(payload_json))
        + payload_json
        + row["prev_hash"]
        + row["entry_hash"]
        + sig
    )


def _build_export(rows: list[Row]) -> tuple[bytes, bool]:
    """Concatène les entrées (déjà triées par `seq`, déjà filtrées sur
    `entry_hash`/`prev_hash` non NULL). Rend l'export et si **toutes** les
    entrées portent une signature — condition pour demander à
    `dengon-verify` de les vérifier (`--pubkey`) : une seule entrée sans
    `sig` rendrait sinon un faux `Broken` sur son octet de bourrage.
    """
    export = bytearray()
    toutes_signees = True
    for row in rows:
        payload_json = canonical_json(json.loads(row["payload"]))
        sig = row["sig"]
        if sig is None:
            toutes_signees = False
            sig = bytes(_SIG_LEN)
        export += _entry_bytes(row, payload_json, sig)
    return bytes(export), toutes_signees


def _run_dengon_verify(export: bytes, pubkey_hex: str | None) -> dict:
    args = [dengon_verify_bin()]
    if pubkey_hex is not None:
        args += ["--pubkey", pubkey_hex]
    args.append("-")
    try:
        proc = subprocess.run(
            args, input=export, capture_output=True, timeout=_TIMEOUT_S, check=False
        )
    except FileNotFoundError as exc:
        raise IntegrityCheckError(
            f"binaire dengon-verify introuvable ({dengon_verify_bin()!r}) : {exc}"
        ) from exc
    except subprocess.TimeoutExpired as exc:
        raise IntegrityCheckError(
            f"dengon-verify n'a pas répondu en {_TIMEOUT_S} s"
        ) from exc
    # Contrat de dengon-verify (doc de module) : 0-3 = verdict connu, une
    # ligne JSON sur stdout ; 64/65/66 = erreur d'appel/lecture, stdout VIDE,
    # message sur stderr — jamais de JSON à parser dans ce cas.
    if proc.returncode >= 64:
        detail = proc.stderr.decode("utf-8", "replace").strip()
        raise IntegrityCheckError(f"dengon-verify (code {proc.returncode}) : {detail}")
    try:
        return json.loads(proc.stdout.decode("utf-8"))
    except (UnicodeDecodeError, ValueError) as exc:
        raise IntegrityCheckError(f"sortie dengon-verify illisible : {exc}") from exc


def check_node(db: LockedConnection, node_id: str, pub_sign: bytes | None) -> NodeIntegrity:
    """Verdict d'intégrité pour un nœud. Aucune entrée exportable (aucun
    événement avec `entry_hash`+`prev_hash`) → `unverified` sans appeler
    `dengon-verify` : un export vide s'y lirait à tort comme `ok` (chaîne
    vide, rien à contredire) — `unverified` dit honnêtement qu'il n'y a rien
    eu à vérifier, distinct d'une vraie chaîne intacte.
    """
    with db.locked() as conn:
        rows = conn.execute(
            "SELECT seq, ts_ms, name, payload, prev_hash, entry_hash, sig FROM events "
            "WHERE node_id = ? AND entry_hash IS NOT NULL AND prev_hash IS NOT NULL "
            "ORDER BY seq",
            (node_id,),
        ).fetchall()
    if not rows:
        return NodeIntegrity(node_id, VERDICT_UNVERIFIED, 0, None, None, "unchecked")

    export, toutes_signees = _build_export(rows)
    pubkey_hex = pub_sign.hex() if (toutes_signees and pub_sign is not None) else None
    report = _run_dengon_verify(export, pubkey_hex)
    return NodeIntegrity(
        node_id=node_id,
        verdict=report["verdict"],
        entries=report["entries"],
        first_seq=report["first_seq"],
        last_seq=report["last_seq"],
        signatures=report["signatures"],
    )


def check_all_nodes(db: LockedConnection) -> list[NodeIntegrity]:
    """Un verdict par nœud connu (`nodes`), dans l'ordre de `node_id`."""
    with db.locked() as conn:
        noeuds = conn.execute("SELECT node_id, pub_sign FROM nodes ORDER BY node_id").fetchall()
    return [check_node(db, row["node_id"], row["pub_sign"]) for row in noeuds]
