"""Lecture des projections `messages` pour le web (US-219).

`docs/synthese/09-dashboard-et-donnees.md` §11.2 prévoit une table
`message_hops` séparée, alimentée à l'ingestion. L'US-217 ne l'a pas créée
(écart consigné, `03-ecarts-conception.md`) : au lieu de la maintenir en
écriture à chaque batch, le « parcours » d'un message est reconstruit **à la
lecture**, directement depuis `events` — la même requête que
`ingest.py::_refresh_message_projection` utilise déjà pour recalculer le
statut. Moins de code à maintenir en écriture, coût négligeable en lecture au
volume visé (démo 5-8 appareils, B-4).
"""

from __future__ import annotations

import json
from dataclasses import dataclass

from .db import LockedConnection

# Les 4 valeurs de `message_hops.kind` prévues par §11.2. Un événement dont
# le nom n'y correspond pas garde son nom d'origine comme `kind` (ex.
# "msg.queued") — le web affiche alors ce nom brut plutôt qu'un libellé
# traduit (`HOP_KIND_LABEL` côté `app.js` retombe sur le texte tel quel),
# ce qui reste informatif plutôt que de masquer l'événement.
_HOP_KIND_BY_EVENT_NAME = {
    "pkt.relayed": "relay",
    "envelope.stored": "envelope_store",
    "envelope.handoff": "envelope_handoff",
    "msg.delivered": "delivered",
    "ack.observed": "delivered",
}


@dataclass(frozen=True)
class MessageHop:
    node_id: str
    ts_ms: int
    kind: str
    ttl_in: int | None
    ttl_out: int | None
    fanout: int | None
    rssi: int | None

    def to_dict(self) -> dict:
        return {
            "node_id": self.node_id,
            "ts_ms": self.ts_ms,
            "kind": self.kind,
            "ttl_in": self.ttl_in,
            "ttl_out": self.ttl_out,
            "fanout": self.fanout,
            "rssi": self.rssi,
        }


def list_messages(db: LockedConnection) -> list[dict]:
    with db.locked() as conn:
        rows = conn.execute(
            "SELECT msg_log_id, conv_hash, first_seen_ms, last_event_ms, status, "
            "status_ms, hop_count, delivery_latency_ms FROM messages "
            "ORDER BY last_event_ms DESC"
        ).fetchall()
    return [dict(row) for row in rows]


def get_message(db: LockedConnection, msg_log_id: str) -> dict | None:
    with db.locked() as conn:
        row = conn.execute(
            "SELECT msg_log_id, conv_hash, first_seen_ms, last_event_ms, status, "
            "status_ms, hop_count, delivery_latency_ms FROM messages WHERE msg_log_id = ?",
            (msg_log_id,),
        ).fetchone()
    return dict(row) if row is not None else None


def get_message_hops(db: LockedConnection, msg_log_id: str) -> list[dict]:
    """Le parcours complet d'un message, dans l'ordre chronologique — pas
    seulement les `pkt.relayed` : tout événement portant ce `msg_log_id`
    (`msg.queued`, `envelope.*`, `ack.observed`, …), pour que « tient avec
    des données partielles » reste visible comme un trou dans le fil plutôt
    que comme un silence total.

    Tri sur `(ts_ms, rowid)` : `ts_ms` seul ne départage pas deux événements
    arrivés à la même milliseconde (plusieurs relais rapprochés, horloges de
    démo synchronisées) — `rowid` (ordre d'insertion) donne au moins un ordre
    stable et reproductible dans ce cas, même si l'ordre chronologique exact
    reste indécidable en cas d'égalité stricte.
    """
    with db.locked() as conn:
        rows = conn.execute(
            "SELECT node_id, ts_ms, name, payload FROM events "
            "WHERE json_extract(payload, '$.msg_log_id') = ? ORDER BY ts_ms, rowid",
            (msg_log_id,),
        ).fetchall()
    hops = []
    for row in rows:
        payload = json.loads(row["payload"])
        hops.append(
            MessageHop(
                node_id=row["node_id"],
                ts_ms=row["ts_ms"],
                kind=_HOP_KIND_BY_EVENT_NAME.get(row["name"], row["name"]),
                ttl_in=payload.get("ttl_in"),
                ttl_out=payload.get("ttl_out"),
                fanout=payload.get("fanout"),
                rssi=payload.get("rssi"),
            ).to_dict()
        )
    return hops
