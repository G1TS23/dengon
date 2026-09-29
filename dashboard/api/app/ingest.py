"""Pipeline de validation + ingestion d'un batch (US-216) + projections
(US-217) + diffusion SSE (US-218).

`POST /ingest/batch` → décoder/parser JSON → valider contre le schéma
(`contracts/events/batch.schema.json`) → `node_id` du batch == `node_id` du
jeton JWT → nœud whitelisté, `pub_sign` connu → signature Ed25519 du batch
→ `event_id` recalculé par événement → insertion idempotente dans
`events` (déduplication sur `event_id`, clé primaire) → statut par message
(`messages`) recalculé pour chaque `msg_log_id` touché par ce batch → les
événements RÉELLEMENT nouveaux sont publiés sur `Broadcaster` (après le
`COMMIT`, jamais avant : un abonné ne doit jamais voir un événement que la
base elle-même ne contient pas encore).

Réf : `docs/synthese/09-dashboard-et-donnees.md` §3 (pipeline complet).
`entry_hash`/`prev_hash`/`sig` (US-310, si présents dans l'événement) sont
stockés tels quels, mais la vérification de journal chaîné via
`dengon-verify` ne se fait PAS ici, à l'ingestion — elle est recalculée à la
demande par `GET /api/integrity` (`app/integrity.py`), pas par événement à
chaque batch : les événements restent insérés avec `integrity = 'unverified'`
(colonne encore inutilisée, valeur par défaut) — écart consigné dans
`03-ecarts-conception.md`.
"""

from __future__ import annotations

import base64
import json
import sqlite3
from dataclasses import dataclass

from nacl.exceptions import BadSignatureError
from nacl.signing import VerifyKey

from .canonical import canonical_json, event_id
from .db import LockedConnection
from .projections import project_message
from .schemas import batch_validator
from .stream import Broadcaster, StreamEvent


class IngestError(Exception):
    """Batch rejeté. `status_code` = code HTTP à renvoyer, `detail` le message."""

    def __init__(self, status_code: int, detail: str) -> None:
        super().__init__(detail)
        self.status_code = status_code
        self.detail = detail


@dataclass(frozen=True)
class IngestResult:
    batch_id: str
    node_id: str
    event_count: int
    # Nombre de lignes RÉELLEMENT insérées dans `events` — un rejeu du même
    # batch (mêmes `event_id`) renvoie 0 ici, pas `event_count` : c'est la
    # preuve d'idempotence demandée par le critère d'acceptation.
    new_event_count: int


def _parse_json(raw: bytes) -> object:
    try:
        return json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, ValueError, RecursionError) as exc:
        # ValueError couvre json.JSONDecodeError (qui en hérite) et le cas
        # d'un littéral entier de plus de 4300 chiffres depuis Python 3.11
        # (`sys.int_max_str_digits`, ValueError générique, PAS
        # JSONDecodeError — retour de revue #59, round 7, même piège
        # transposé ici). RecursionError : JSON très imbriqué.
        raise IngestError(
            400, "corps invalide : un document JSON UTF-8 est attendu"
        ) from exc


def _validate_schema(parsed: object) -> dict:
    if not isinstance(parsed, dict):
        raise IngestError(400, "corps invalide : un objet JSON est attendu")
    errors = sorted(batch_validator().iter_errors(parsed), key=str)
    if errors:
        raise IngestError(400, f"schéma invalide : {errors[0].message}")
    return parsed


def _lookup_node(db: LockedConnection, node_id_: str) -> tuple[bytes, str]:
    with db.locked() as conn:
        row = conn.execute(
            "SELECT pub_sign, kind, whitelisted FROM nodes WHERE node_id = ?", (node_id_,)
        ).fetchone()
    if row is None or not row["whitelisted"]:
        # Même message pour "n'existe pas" et "existe mais pas whitelisté" :
        # distinguer les deux ne renseignerait qu'un attaquant qui essaie
        # de deviner des node_id valides (docs/synthese/09 §3 : « node
        # inconnu → quarantaine, alerte », traité ici comme un rejet direct
        # plutôt qu'une quarantaine — pas d'écran opérateur pour lever une
        # quarantaine dans le périmètre de l'US-216).
        raise IngestError(401, "nœud inconnu ou non whitelisté")
    return row["pub_sign"], row["kind"]


def _verify_signature(body: dict, pub_sign: bytes) -> None:
    sig_b64 = body.get("sig")
    unsigned = {k: v for k, v in body.items() if k != "sig"}
    try:
        VerifyKey(pub_sign).verify(canonical_json(unsigned), base64.b64decode(sig_b64))
    except (BadSignatureError, ValueError, TypeError) as exc:
        # ValueError/TypeError : `sig` mal encodé en base64, ou du mauvais
        # type — le schéma le garantit déjà normalement (pattern base64
        # figé), mais on ne fait pas confiance à l'ordre d'évaluation d'un
        # futur refactor pour ne jamais atteindre ce point avec un `sig`
        # inattendu.
        raise IngestError(401, f"signature invalide : {exc}") from exc


def _verify_event_ids(body: dict, expected_node_kind: str) -> None:
    """Recalcule `event_id` pour chaque événement plutôt que de faire
    confiance à la valeur envoyée : le schéma vérifie seulement le
    *format* (64 hex), pas la cohérence avec `node_id`/`seq`. Un `event_id`
    forgé pourrait sinon empoisonner la déduplication (masquer un vrai
    événement derrière un faux `event_id` qui collisionne).

    Vérifie aussi que `node_id`/`node_kind` de CHAQUE événement correspondent
    au `node_id` du batch (== celui du JWT, déjà vérifié) et au `kind`
    enregistré pour ce nœud dans `nodes` — le schéma JSON documente cette
    contrainte (`batch.schema.json` : « doit correspondre au node_id de
    chaque événement ») mais ne peut pas l'imposer, et rien d'autre ne le
    faisait avant : un nœud whitelisté pouvait signer un batch valide tout
    en attribuant ses événements à un AUTRE node_id (`_insert_events` stocke
    `event["node_id"]`, pas celui du batch), ce qui annule la garantie
    « signé par le nœud émetteur » (revue PR #91, point bloquant).
    """
    node_id_ = body["node_id"]
    for event in body["events"]:
        seq = event["seq"]
        expected = event_id(node_id_, seq)
        if event.get("event_id") != expected:
            raise IngestError(400, f"event_id incohérent pour seq {seq}")
        if event.get("node_id") != node_id_:
            raise IngestError(
                400, f"node_id de l'événement (seq {seq}) différent du node_id du batch"
            )
        if event.get("node_kind") != expected_node_kind:
            raise IngestError(
                400, f"node_kind de l'événement (seq {seq}) différent du kind enregistré du nœud"
            )


def _hex_or_none(value: object) -> bytes | None:
    """`entry_hash`/`prev_hash` (US-310) : hex → octets, `None` si absent.

    Le schéma (`envelope.schema.json`) garantit déjà 64 hex si le champ est
    présent — pas de re-validation ici, juste le décodage.
    """
    return bytes.fromhex(value) if isinstance(value, str) else None


def _b64_or_none(value: object) -> bytes | None:
    """`sig` par entrée (US-310) : base64 → octets, `None` si absent."""
    return base64.b64decode(value) if isinstance(value, str) else None


def _touched_msg_log_ids(body: dict) -> set[str]:
    return {
        event["payload"]["msg_log_id"]
        for event in body["events"]
        if event.get("payload", {}).get("msg_log_id")
    }


def _refresh_message_projection(conn: sqlite3.Connection, msg_log_id: str) -> None:
    """Recalcule la projection de `msg_log_id` depuis **tous** les
    événements connus en base (pas seulement ceux du batch courant) : un
    batch qui arrive en retard ou dans le désordre par rapport à un autre
    doit produire le même statut final, peu importe l'ordre d'ingestion —
    c'est plus simple à garantir en relisant l'état complet qu'en fusionnant
    incrémentalement (voir `app/projections.py`).
    """
    rows = conn.execute(
        "SELECT name, ts_ms, payload FROM events WHERE json_extract(payload, '$.msg_log_id') = ?",
        (msg_log_id,),
    ).fetchall()
    events = [
        {"name": row["name"], "ts_ms": row["ts_ms"], "payload": json.loads(row["payload"])}
        for row in rows
    ]
    projection = project_message(msg_log_id, events)
    conn.execute(
        "INSERT INTO messages "
        "(msg_log_id, conv_hash, first_seen_ms, last_event_ms, status, status_ms, "
        " hop_count, delivery_latency_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?) "
        "ON CONFLICT(msg_log_id) DO UPDATE SET "
        "conv_hash = excluded.conv_hash, first_seen_ms = excluded.first_seen_ms, "
        "last_event_ms = excluded.last_event_ms, status = excluded.status, "
        "status_ms = excluded.status_ms, hop_count = excluded.hop_count, "
        "delivery_latency_ms = excluded.delivery_latency_ms",
        (
            projection.msg_log_id,
            projection.conv_hash,
            projection.first_seen_ms,
            projection.last_event_ms,
            projection.status,
            projection.status_ms,
            projection.hop_count,
            projection.delivery_latency_ms,
        ),
    )


def _insert_events(db: LockedConnection, body: dict) -> list[StreamEvent]:
    """Insère les événements du batch, idempotent (`INSERT OR IGNORE` sur
    `event_id`, clé primaire de `events`), puis recalcule la projection
    `messages` de chaque `msg_log_id` touché — dans la **même transaction**
    (une projection ne doit jamais refléter un batch partiellement inséré).
    Renvoie les lignes RÉELLEMENT insérées dans `events` (nouvelles, avec
    leur `rowid`) — un rejeu du même batch renvoie une liste vide, c'est la
    preuve d'idempotence demandée par le critère d'acceptation de l'US-216.
    """
    inserted: list[StreamEvent] = []
    with db.locked() as conn:
        conn.execute("BEGIN IMMEDIATE")
        try:
            for event in body["events"]:
                cur = conn.execute(
                    "INSERT OR IGNORE INTO events "
                    "(event_id, ts_ms, node_id, name, seq, payload, "
                    " entry_hash, prev_hash, sig) "
                    "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    (
                        event["event_id"],
                        event["ts_ms"],
                        event["node_id"],
                        event["name"],
                        event["seq"],
                        json.dumps(event["payload"], sort_keys=True, separators=(",", ":")),
                        _hex_or_none(event.get("entry_hash")),
                        _hex_or_none(event.get("prev_hash")),
                        _b64_or_none(event.get("sig")),
                    ),
                )
                # `cur.lastrowid` n'est fiable que si CETTE instruction a
                # inséré une ligne (`rowcount == 1`) : sur un `INSERT OR
                # IGNORE` ignoré (conflit sur `event_id`), sqlite3 laisse
                # `lastrowid` à sa valeur précédente plutôt que de le mettre
                # à jour — le lire sans garder `rowcount` republierait un
                # événement déjà vu sous l'identité SSE d'un autre.
                if cur.rowcount:
                    inserted.append(
                        StreamEvent(
                            rowid=cur.lastrowid,
                            event_id=event["event_id"],
                            ts_ms=event["ts_ms"],
                            node_id=event["node_id"],
                            name=event["name"],
                            payload=event["payload"],
                        )
                    )
            for msg_log_id in _touched_msg_log_ids(body):
                _refresh_message_projection(conn, msg_log_id)
            conn.execute("COMMIT")
        except Exception:
            if conn.in_transaction:
                conn.execute("ROLLBACK")
            raise
    return inserted


def ingest_batch(
    db: LockedConnection,
    jwt_node_id: str,
    raw_body: bytes,
    broadcaster: Broadcaster | None = None,
) -> IngestResult:
    """Le pipeline complet — voir la docstring de module pour l'ordre des
    étapes. Lève [`IngestError`] à la première étape qui échoue.

    `broadcaster` est optionnel (`None` dans les tests qui appellent
    `ingest_batch` sans passer par l'app FastAPI — voir `test_projections.py`
    de l'US-217, écrit avant que ce paramètre n'existe) : sans lui, les
    événements sont insérés normalement, simplement pas diffusés en SSE.
    """
    parsed = _parse_json(raw_body)
    body = _validate_schema(parsed)

    if body["node_id"] != jwt_node_id:
        raise IngestError(401, "node_id du batch différent du node_id du jeton")

    pub_sign, node_kind = _lookup_node(db, jwt_node_id)
    _verify_signature(body, pub_sign)
    _verify_event_ids(body, node_kind)

    inserted = _insert_events(db, body)
    if broadcaster is not None:
        broadcaster.publish(inserted)

    return IngestResult(
        batch_id=body["batch_id"],
        node_id=body["node_id"],
        event_count=len(body["events"]),
        new_event_count=len(inserted),
    )
