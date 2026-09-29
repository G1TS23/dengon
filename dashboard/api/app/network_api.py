"""Vue « réseau » et « flotte de relais » (US-311).

Comme `messages_api.py`, tout est **dérivé de `events` à la lecture** : pas de
table `links` ni de projection de santé (hors périmètre, écart consigné dans
`03-ecarts-conception.md`). Au volume visé (démo 5-8 appareils, B-4) une
lecture des événements de santé/topologie est négligeable.

* Flotte : par relais, dernier `relay.boot` (version), dernier `relay.health`
  (uptime, RSSI, tampons), et alertes calculées par rapport à `now_ms`.
* Graphe : nœuds connus (`nodes`) + pairs vus (`peer.connected` /
  `peer.disconnected`). Le `peer` d'un événement est un **peerID** (8 octets
  hex), pas un `node_id` : il ne peut pas être rapproché d'un nœud remontant
  ses propres événements. Les pairs apparaissent donc comme nœuds
  `kind="peer"`, rattachés au nœud qui les a observés.

Une vue partielle est la norme (seuls les nœuds ayant du Wi-Fi remontent) :
les champs de santé sont `None` tant qu'aucun événement n'est reçu.
"""

from __future__ import annotations

import json

from .db import LockedConnection

BUFFER_HIGH_PCT = 90  # docs/synthese/09 §5 : « buffer > 90 % »


def _last_event(conn, node_id: str, name: str):
    return conn.execute(
        "SELECT ts_ms, payload FROM events WHERE node_id = ? AND name = ? "
        "ORDER BY ts_ms DESC, seq DESC LIMIT 1",
        (node_id, name),
    ).fetchone()


def _relay_alerts(
    last_contact_ms: int | None, health: dict | None, now_ms: int, silent_ms: int
) -> list[dict]:
    alerts: list[dict] = []
    if last_contact_ms is None or now_ms - last_contact_ms > silent_ms:
        alerts.append({"code": "relay_silent", "since_ms": last_contact_ms})
    if health is not None and (health.get("log_buffer_pct") or 0) > BUFFER_HIGH_PCT:
        alerts.append({"code": "buffer_high", "value": health["log_buffer_pct"]})
    return alerts


def list_fleet(db: LockedConnection, now_ms: int, silent_ms: int) -> list[dict]:
    """Un dict par nœud enregistré ; les relais portent santé + alertes.

    `last_contact_ms` = dernier `relay.health` pour un relais (règle de
    l'alerte `relay_silent`), à défaut son `relay.boot`, à défaut son dernier
    événement : un relais qui a booté mais n'a jamais émis de santé est
    signalé muet une fois le délai écoulé depuis son boot.
    """
    fleet: list[dict] = []
    with db.locked() as conn:
        nodes = conn.execute(
            "SELECT node_id, kind, label, fw_version FROM nodes ORDER BY node_id"
        ).fetchall()
        for node in nodes:
            node_id = node["node_id"]
            last_any = conn.execute(
                "SELECT MAX(ts_ms) AS ts FROM events WHERE node_id = ?", (node_id,)
            ).fetchone()["ts"]
            entry = {
                "node_id": node_id,
                "kind": node["kind"],
                "label": node["label"],
                "fw_version": node["fw_version"],
                "last_contact_ms": last_any,
                "status": "online",
                "health": None,
                "alerts": [],
            }
            if node["kind"] == "relay":
                boot = _last_event(conn, node_id, "relay.boot")
                if boot is not None:
                    entry["fw_version"] = json.loads(boot["payload"]).get("fw_version")
                health_row = _last_event(conn, node_id, "relay.health")
                if health_row is not None:
                    entry["health"] = json.loads(health_row["payload"])
                    entry["last_contact_ms"] = health_row["ts_ms"]
                elif boot is not None:
                    entry["last_contact_ms"] = boot["ts_ms"]
                entry["alerts"] = _relay_alerts(
                    entry["last_contact_ms"], entry["health"], now_ms, silent_ms
                )
                if any(a["code"] == "relay_silent" for a in entry["alerts"]):
                    entry["status"] = "stale"
            fleet.append(entry)
    return fleet


def network_graph(db: LockedConnection, now_ms: int, silent_ms: int) -> dict:
    """Snapshot `{nodes, links}` pour la carte du réseau."""
    fleet = list_fleet(db, now_ms, silent_ms)
    nodes = [
        {"id": n["node_id"], "kind": n["kind"], "label": n["label"], "status": n["status"]}
        for n in fleet
    ]
    links: dict[tuple[str, str], dict] = {}
    with db.locked() as conn:
        rows = conn.execute(
            "SELECT node_id, ts_ms, name, payload FROM events "
            "WHERE name IN ('peer.connected','peer.disconnected') "
            "ORDER BY ts_ms ASC, seq ASC"
        ).fetchall()
    for row in rows:
        payload = json.loads(row["payload"])
        peer = payload.get("peer")
        if not peer:
            continue
        link = links.setdefault(
            (row["node_id"], peer),
            {
                "source": row["node_id"],
                "target": peer,
                "active": False,
                "sessions": 0,
                "rssi": None,
                "last_ts_ms": None,
            },
        )
        link["last_ts_ms"] = row["ts_ms"]
        if row["name"] == "peer.connected":
            link["active"] = True
            link["sessions"] += 1
            link["rssi"] = payload.get("rssi")
        else:
            link["active"] = False
    known = {n["id"] for n in nodes}
    for _source, peer in links:
        if peer not in known:
            known.add(peer)
            nodes.append({"id": peer, "kind": "peer", "label": None, "status": "online"})
    return {"now_ms": now_ms, "nodes": nodes, "links": list(links.values())}
