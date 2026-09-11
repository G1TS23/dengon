"""Catalogue des événements d'observabilité — source lisible du contrat.

Miroir de `docs/powl/08-observability-events.md` et `docs/synthese/09` §9,
restreint au **périmètre MVP** (le statut « Lu » et les `integrity.*` /
`node.clock_skew` dérivés côté dashboard sont hors périmètre — ils n'entrent
pas par `/ingest/batch`).

`build_fixtures.py` s'en sert pour fabriquer les fixtures et générer
`events/payloads.schema.json` ; `validate.py` s'en sert pour vérifier chaque
`payload`. Si ce dict et le `.md` divergent, c'est un bug de contrat.
"""

from __future__ import annotations

import hashlib
import json

# --- fragments de schéma réutilisés -----------------------------------------

# minLength/maxLength en plus du pattern, pas pour la forme : le moteur
# regex de `jsonschema` (Python `re`) fait correspondre `$` juste AVANT un
# `\n` final, donc `"<16 hex>\n"` (17 caractères) passerait le seul pattern
# (retour de revue #60, relecture approfondie). `\Z` corrigerait ça mais est
# une extension Python — l'éviter ici puisque ces fragments finissent dans
# payloads.schema.json, censé rester neutre en langage (JSON Schema/ECMA 262
# n'a pas de `\Z`). minLength/maxLength ferme le même trou sans dépendre du
# moteur regex.
# msg_log_id, peerID tronqué 8 o
HEX16 = {"type": "string", "pattern": "^[0-9a-f]{16}$", "minLength": 16, "maxLength": 16}
# recipient_tag (16 o)
HEX32 = {"type": "string", "pattern": "^[0-9a-f]{32}$", "minLength": 32, "maxLength": 32}
# racine de journal (SHA-256)
HEX64 = {"type": "string", "pattern": "^[0-9a-f]{64}$", "minLength": 64, "maxLength": 64}
UINT = {"type": "integer", "minimum": 0}
# Même motif que $defs/node_id de events/envelope.schema.json (référencé aussi
# par batch.schema.json via $ref) — dupliqué ici parce que ce module est
# Python, pas JSON Schema, donc ne peut pas faire de $ref vers ce fichier.
NODE_ID_PATTERN = r"^(relay|client)-[0-9a-f]{6,}$"
PKT_TYPE = {"type": "integer", "minimum": 1, "maximum": 13}  # types de paquets, synthese/05 §4
SIZE_BUCKET = {"enum": [256, 512, 1024, 2048]}
RSSI = {"type": "integer", "minimum": -120, "maximum": 0}

# --- le catalogue ----------------------------------------------------------
# name -> { "required": [...], "props": { champ: fragment } }
# Les payloads ne sont PAS fermés (additionalProperties non interdit) : un ajout
# de champ est rétrocompatible (schema_version). L'enveloppe et le batch, eux,
# sont stricts.

CATALOGUE: dict[str, dict] = {
    # -- pkt ---------------------------------------------------------------
    "pkt.seen": {
        "required": ["msg_log_id", "type", "ttl_in", "size_bucket", "from_peer"],
        "props": {
            "msg_log_id": HEX16,
            "type": PKT_TYPE,
            "ttl_in": UINT,
            "size_bucket": SIZE_BUCKET,
            "from_peer": HEX16,
            "rssi": RSSI,
        },
    },
    "pkt.duplicate": {
        "required": ["msg_log_id", "from_peer"],
        "props": {"msg_log_id": HEX16, "from_peer": HEX16},
    },
    "pkt.relayed": {
        "required": ["msg_log_id", "type", "ttl_in", "ttl_out", "fanout", "from_peer"],
        "props": {
            "msg_log_id": HEX16,
            "type": PKT_TYPE,
            "ttl_in": UINT,
            "ttl_out": UINT,
            "fanout": UINT,
            "from_peer": HEX16,
        },
    },
    "pkt.delivered_local": {
        "required": ["msg_log_id", "type"],
        "props": {"msg_log_id": HEX16, "type": PKT_TYPE},
    },
    "pkt.dropped": {
        "required": ["reason"],
        "props": {
            "msg_log_id": HEX16,
            "reason": {"enum": ["ttl_zero", "relay_not_ok", "queue_full", "quota", "too_old"]},
        },
    },
    "pkt.rejected": {
        "required": ["from_peer", "reason"],
        "props": {
            "from_peer": HEX16,
            "reason": {"enum": ["bad_sig", "bad_version", "malformed", "peerid_mismatch"]},
        },
    },
    # -- envelope --------------------------------------------------------
    "envelope.stored": {
        "required": ["msg_log_id", "recipient_tag", "epoch_day", "copy_budget"],
        "props": {
            "msg_log_id": HEX16,
            "recipient_tag": HEX32,
            "epoch_day": UINT,
            "copy_budget": UINT,
        },
    },
    "envelope.offered": {
        "required": ["count", "to_peer"],
        "props": {"count": UINT, "to_peer": HEX16},
    },
    "envelope.handoff": {
        "required": ["msg_log_id", "recipient_tag", "to_peer", "budget_after"],
        "props": {
            "msg_log_id": HEX16,
            "recipient_tag": HEX32,
            "to_peer": HEX16,
            "budget_after": UINT,
        },
    },
    "envelope.delivered": {
        "required": ["msg_log_id", "recipient_tag", "to_peer"],
        "props": {"msg_log_id": HEX16, "recipient_tag": HEX32, "to_peer": HEX16},
    },
    "envelope.expired": {
        "required": ["msg_log_id", "recipient_tag", "reason"],
        "props": {
            "msg_log_id": HEX16,
            "recipient_tag": HEX32,
            "reason": {"enum": ["ttl", "budget_zero", "store_full"]},
        },
    },
    # -- msg (cycle de vie) --------------------------------------------
    "msg.queued": {
        "required": ["msg_log_id", "conv_hash"],
        "props": {"msg_log_id": HEX16, "conv_hash": HEX16},
    },
    "msg.handed_off": {
        "required": ["msg_log_id", "to_peer", "via"],
        "props": {"msg_log_id": HEX16, "to_peer": HEX16, "via": {"enum": ["session", "envelope"]}},
    },
    "msg.received": {
        "required": ["msg_log_id", "via"],
        "props": {"msg_log_id": HEX16, "via": {"enum": ["session", "envelope"]}},
    },
    "msg.delivered": {
        "required": ["msg_log_id", "latency_ms"],
        "props": {"msg_log_id": HEX16, "latency_ms": UINT},
    },
    "msg.expired": {
        "required": ["msg_log_id"],
        "props": {"msg_log_id": HEX16},
    },
    "ack.observed": {
        "required": ["msg_log_id"],
        "props": {"msg_log_id": HEX16},
    },
    # -- peer / link --------------------------------------------------
    "peer.connected": {
        "required": ["peer", "rssi", "role"],
        "props": {"peer": HEX16, "rssi": RSSI, "role": {"enum": ["central", "peripheral"]}},
    },
    "peer.disconnected": {
        "required": ["peer", "duration_s", "pkt_exchanged"],
        "props": {"peer": HEX16, "duration_s": UINT, "pkt_exchanged": UINT},
    },
    "peer.announce_seen": {
        "required": ["peer", "ledger_height", "caps"],
        "props": {
            "peer": HEX16,
            "pseudo": {"type": "string", "maxLength": 32},
            "ledger_height": UINT,
            "caps": UINT,
        },
    },
    # -- attest -----------------------------------------------------
    "attest.emitted": {
        "required": ["root", "height"],
        "props": {"root": HEX64, "height": UINT},
    },
    "attest.observed": {
        "required": ["subject_node", "root", "height"],
        "props": {
            "subject_node": {"type": "string", "pattern": NODE_ID_PATTERN},
            "root": HEX64,
            "height": UINT,
        },
    },
    # -- relay / node (santé) --------------------------------------
    "relay.boot": {
        "required": ["fw_version", "reset_reason", "secure_boot", "flash_enc"],
        "props": {
            "fw_version": {"type": "string"},
            "reset_reason": {"type": "string"},
            "secure_boot": {"type": "boolean"},
            "flash_enc": {"type": "boolean"},
        },
    },
    "relay.health": {
        "required": [
            "uptime_s",
            "rssi_avg",
            "peers",
            "cache_size",
            "envelope_store",
            "log_buffer_pct",
            "logs_dropped",
            "heap_free",
        ],
        "props": {
            "uptime_s": UINT,
            "rssi_avg": RSSI,
            "peers": UINT,
            "cache_size": UINT,
            "envelope_store": UINT,
            "log_buffer_pct": {"type": "integer", "minimum": 0, "maximum": 100},
            "logs_dropped": UINT,
            "heap_free": UINT,
        },
    },
    "relay.wifi_up": {
        "required": [],
        "props": {"ssid": {"type": "string"}, "duration_s": UINT},
    },
    "relay.wifi_down": {
        "required": [],
        "props": {"ssid": {"type": "string"}, "duration_s": UINT},
    },
    "relay.overloaded": {
        "required": ["subsystem", "action"],
        "props": {
            "subsystem": {"type": "string"},
            "action": {"enum": ["drop_pkt", "refuse_envelope", "evict_cache"]},
        },
    },
    "client.summary": {
        "required": ["msgs_sent", "msgs_recv", "peers_seen", "app_version"],
        "props": {
            "msgs_sent": UINT,
            "msgs_recv": UINT,
            "peers_seen": UINT,
            "app_version": {"type": "string"},
        },
    },
}

# Événements explicitement HORS de ce contrat (documentés pour lever le doute) :
#   msg.read, read.observed            -> statut « Lu », v2 (A-10)
#   integrity.chain_broken/fork/gap    -> conclusions de l'ingest, pas du terrain
#   node.clock_skew                    -> idem
OUT_OF_SCOPE = {
    "msg.read",
    "read.observed",
    "integrity.chain_broken",
    "integrity.fork_detected",
    "integrity.gap",
    "node.clock_skew",
}


# --- helpers de contrat ---------------------------------------------------


def canonical_json(obj: object) -> bytes:
    """Forme canonique signée : clés triées, séparateurs compacts, UTF-8, pas de NaN.

    C'EST le contrat de signature. L'implémentation Rust (`serde_json` +
    tri des clés, entiers jamais en `f64`) doit produire des octets identiques.
    Voir events/CANONICAL.md.
    """
    return json.dumps(
        obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False
    ).encode("utf-8")


def event_id(node_id: str, seq: int) -> str:
    """hex(SHA-256(node_id ‖ seq_big_endian_64))."""
    return hashlib.sha256(node_id.encode("ascii") + seq.to_bytes(8, "big")).hexdigest()


def payloads_json_schema() -> dict:
    """Dérive un JSON Schema (draft 2020-12) `payload` à partir du catalogue.

    Un `allOf` de `if (name == X) then (payload requiert …)`. Généré pour les
    consommateurs qui veulent du JSON Schema pur (US-217) ; la source reste ce
    module.
    """
    clauses = []
    for name, spec in sorted(CATALOGUE.items()):
        then_payload: dict = {"type": "object", "properties": dict(spec["props"])}
        if spec["required"]:
            then_payload["required"] = list(spec["required"])
        clauses.append(
            {
                "if": {"properties": {"name": {"const": name}}, "required": ["name"]},
                "then": {"properties": {"payload": then_payload}},
            }
        )
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://dengon.local/contracts/events/payloads.schema.json",
        "title": "Contraintes de payload par nom d'événement (périmètre MVP)",
        "description": "Généré depuis contracts/tools/catalogue.py — ne pas éditer à la main.",
        "type": "object",
        "allOf": clauses,
    }
