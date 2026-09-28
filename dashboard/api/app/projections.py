"""Reconstruction du statut d'un message à partir du flux d'événements (US-217).

Repose sur `docs/synthese/09-dashboard-et-donnees.md` §10 (table de
déduction) et le schéma `messages` de son §11.2 — plus granulaire que §10 :
là où §10 range `msg.queued`/`msg.handed_off`/`pkt.relayed` dans un seul
bucket « en circulation », `messages.status` distingue `queued` (aucune
prise en charge encore observée) de `in_flight` (relayé ou remis à un
porteur).

Une projection se recalcule **entièrement à partir de l'ensemble des
événements connus pour un `msg_log_id`**, jamais par mise à jour
incrémentale d'un état précédent : c'est ce qui rend le résultat robuste à
l'ordre d'arrivée (retard, désordre, doublon) par construction, plutôt
qu'une garantie à maintenir manuellement à chaque nouvel événement.
"""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Iterable
from dataclasses import dataclass

# `msg.read`/`ack`/`read.observed` (statut `read`, v2) ne sont pas traités
# ici : §10 ne couvre que queued/in_flight/delivered/expired/unknown — voir
# écart consigné dans 03-ecarts-conception.md.
_DELIVERED_EVENTS = {"msg.delivered", "ack.observed"}
_IN_FLIGHT_EVENTS = {"msg.handed_off", "pkt.relayed"}
_QUEUED_EVENTS = {"msg.queued"}
_EXPIRED_EVENTS = {"msg.expired"}

# Ordre de priorité pour le statut final, du plus définitif au moins
# définitif. "delivered" gagne toujours sur "expired" : un message livré
# puis marqué expiré par un événement en retard reste délivré (§10,
# "Expiré : ... et PAS de delivered").
_STATUS_TRIGGERS: list[tuple[str, set[str]]] = [
    ("delivered", _DELIVERED_EVENTS),
    ("expired", _EXPIRED_EVENTS),
    ("in_flight", _IN_FLIGHT_EVENTS),
    ("queued", _QUEUED_EVENTS),
]


@dataclass(frozen=True)
class MessageProjection:
    msg_log_id: str
    conv_hash: str | None
    first_seen_ms: int
    last_event_ms: int
    status: str
    status_ms: int
    hop_count: int
    delivery_latency_ms: int | None


def _status_and_trigger_names(names: set[str]) -> tuple[str, set[str]]:
    for status, trigger_names in _STATUS_TRIGGERS:
        if names & trigger_names:
            return status, trigger_names
    return "unknown", set()


def project_message(msg_log_id: str, events: list[dict]) -> MessageProjection:
    """Projette les événements d'un seul `msg_log_id` en une projection.

    `events` : enveloppes complètes (`name`, `ts_ms`, `payload`), déjà
    filtrées sur ce `msg_log_id`, dans un ordre arbitraire (peut contenir
    des doublons — idempotent). Ne lève jamais ; `events` vide renvoie un
    statut `unknown` avec des horodatages à 0 (ne devrait pas arriver en
    pratique : [`group_by_msg_log_id`] ne produit que des groupes non vides).
    """
    names = {e["name"] for e in events}
    status, trigger_names = _status_and_trigger_names(names)

    ts_values = [e["ts_ms"] for e in events]
    first_seen_ms = min(ts_values) if ts_values else 0
    last_event_ms = max(ts_values) if ts_values else 0

    # status_ms : horodatage du plus récent événement qui JUSTIFIE le statut
    # retenu — pas `last_event_ms`, qui pourrait être un événement sans
    # rapport avec le statut final (ex. un `pkt.dropped` arrivé après le
    # `msg.delivered` qui a déjà fixé le statut).
    triggering_ts = [e["ts_ms"] for e in events if e["name"] in trigger_names]
    status_ms = max(triggering_ts) if triggering_ts else last_event_ms

    conv_hash = next(
        (e["payload"]["conv_hash"] for e in events if e["payload"].get("conv_hash")), None
    )
    hop_count = sum(1 for e in events if e["name"] == "pkt.relayed")
    delivery_latency_ms = next(
        (
            e["payload"]["latency_ms"]
            for e in events
            if e["name"] == "msg.delivered" and "latency_ms" in e["payload"]
        ),
        None,
    )

    return MessageProjection(
        msg_log_id=msg_log_id,
        conv_hash=conv_hash,
        first_seen_ms=first_seen_ms,
        last_event_ms=last_event_ms,
        status=status,
        status_ms=status_ms,
        hop_count=hop_count,
        delivery_latency_ms=delivery_latency_ms,
    )


def group_by_msg_log_id(events: Iterable[dict]) -> dict[str, list[dict]]:
    """Regroupe des événements par `payload.msg_log_id`, en ignorant ceux
    qui n'en portent pas (topologie, santé relais, attestations — hors
    périmètre de la reconstruction de statut d'un message).
    """
    grouped: dict[str, list[dict]] = defaultdict(list)
    for event in events:
        msg_log_id = event.get("payload", {}).get("msg_log_id")
        if msg_log_id:
            grouped[msg_log_id].append(event)
    return grouped


def project_messages(events: Iterable[dict]) -> dict[str, MessageProjection]:
    """Projette un flux d'événements — arbitrairement ordonné, avec
    doublons ou données partielles (un nœud n'a pas (encore) remonté) — en
    un statut par `msg_log_id`.
    """
    return {
        msg_log_id: project_message(msg_log_id, group)
        for msg_log_id, group in group_by_msg_log_id(events).items()
    }
