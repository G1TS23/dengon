# Catalogue des événements d'observabilité

Source de vérité des noms et champs d'événements. Utilisé par `dengon-core::observability`,
le firmware relais et l'ingest du dashboard.

---

## 1. Règles générales

1. **Nommage** : `<domaine>.<action>` en `snake_case` (`pkt.relayed`, `msg.delivered`).
2. **Enveloppe commune** (tout événement) :

```json
{
  "event_id": "hex(SHA-256(node_id ‖ seq))",
  "node_id": "relay-3f2a… | client-9c1d…",
  "node_kind": "relay | client",
  "seq": 10432,
  "ts_ms": 1725800000123,
  "name": "pkt.relayed",
  "payload": { }
}
```

1. **Signature** : le nœud signe `canonical_json(enveloppe_sans_sig)` avec sa clé
   `sign` ; `sig` (base64) est ajouté hors du JSON canonique, dans le batch MQTT.
2. **Chaînage** : les événements sont **aussi** des entrées de journal (`03`/`04`) ;
   `payload` inclut `prev_hash` implicitement via le mécanisme `ledger` (le dashboard
   recalcule).
3. **Redaction obligatoire avant émission** :
   - `msgID` → `msg_log_id = hex(SHA-256(msgID)[0..16])` ;
   - jamais de `msg_uuid`, jamais de texte, jamais de `recipient` en clair ;
   - `peerID` de pairs : gardés (ce sont des pseudonymes) mais tronqués à 8 o ;
   - `recipient_tag` : autorisé (déjà anonyme et tournant).
4. **Batching** : N événements par message MQTT ; `event_id` déduplique côté VPS.

---

## 2. Domaine `pkt` — trafic de paquets (surtout relais)

| `name` | Producteur | `payload` | Sens |
| --- | --- | --- | --- |
| `pkt.seen` | relais, client | `{ msg_log_id, type, ttl_in, size_bucket, from_peer, rssi }` | paquet reçu (avant dédup) |
| `pkt.duplicate` | relais, client | `{ msg_log_id, from_peer }` | déjà dans le seen-set |
| `pkt.relayed` | relais, client | `{ msg_log_id, type, ttl_in, ttl_out, fanout, from_peer }` | rediffusé |
| `pkt.delivered_local` | client | `{ msg_log_id, type }` | paquet pour moi, traité |
| `pkt.dropped` | relais, client | `{ msg_log_id?, reason }` | `reason ∈ {ttl_zero, relay_not_ok, queue_full, quota, too_old}` |
| `pkt.rejected` | relais, client | `{ from_peer, reason }` | `reason ∈ {bad_sig, bad_version, malformed, peerid_mismatch}` |

`size_bucket` ∈ `{256,512,1024,2048}` (jamais la taille exacte).

---

## 3. Domaine `envelope` — enveloppes scellées (store-and-forward)

| `name` | Producteur | `payload` | Sens |
| --- | --- | --- | --- |
| `envelope.stored` | relais, client | `{ msg_log_id, recipient_tag, epoch_day, copy_budget }` | enveloppe acceptée en dépôt |
| `envelope.offered` | relais, client | `{ count, to_peer }` | `ENVELOPE_OFFER` envoyé |
| `envelope.handoff` | relais, client | `{ msg_log_id, recipient_tag, to_peer, budget_after }` | copie transmise à un autre porteur |
| `envelope.delivered` | relais, client | `{ msg_log_id, recipient_tag, to_peer }` | remise à un pair dont le tag matche (probable destinataire) |
| `envelope.expired` | relais, client | `{ msg_log_id, recipient_tag, reason }` | `reason ∈ {ttl, budget_zero, store_full}` |

---

## 4. Domaine `msg` — cycle de vie (clients opt-in surtout)

| `name` | Producteur | `payload` | Statut résultant |
| --- | --- | --- | --- |
| `msg.queued` | client émetteur | `{ msg_log_id, conv_hash }` | `queued` |
| `msg.handed_off` | client émetteur | `{ msg_log_id, to_peer, via }` (`via ∈ {session, envelope}`) | `in_flight` |
| `msg.received` | client destinataire | `{ msg_log_id, via }` | (contribue à `delivered`) |
| `msg.delivered` | client émetteur | `{ msg_log_id, latency_ms }` | `delivered` |
| `msg.read` | client émetteur | `{ msg_log_id, latency_ms }` | `read` |
| `msg.expired` | client émetteur | `{ msg_log_id }` | `expired` |
| `ack.observed` | relais | `{ msg_log_id }` | indice de `delivered` |
| `read.observed` | relais | `{ msg_log_id }` | indice de `read` |

`conv_hash` = `SHA-256(min(peerA,peerB) ‖ max(peerA,peerB))[0..8]` : permet de
grouper une conversation **sans** savoir qui elle implique.

> Reconstruction du statut côté dashboard : priorité aux événements de l'émetteur
> (`msg.*`) ; à défaut, inférence best-effort depuis les événements relais
> (`pkt.relayed`, `envelope.delivered`, `ack.observed`, `read.observed`).
> Statut `unknown` si aucune donnée.

---

## 5. Domaine `peer` / `link` — topologie

| `name` | Producteur | `payload` |
| --- | --- | --- |
| `peer.connected` | relais, client | `{ peer, rssi, role }` (`role ∈ {central, peripheral}`) |
| `peer.disconnected` | relais, client | `{ peer, duration_s, pkt_exchanged }` |
| `peer.announce_seen` | relais, client | `{ peer, pseudo?, ledger_height, caps }` |

Le dashboard dérive `LINKS` de la corrélation `peer.connected`/`disconnected` entre
deux `node_id` connus.

---

## 6. Domaine `attest` / `integrity` — journal chaîné

| `name` | Producteur | `payload` | Sens |
| --- | --- | --- | --- |
| `attest.emitted` | tout nœud | `{ root, height }` | ma racine de journal courante |
| `attest.observed` | relais, client | `{ subject_node, root, height }` | racine d'un **autre** nœud, vue via `LOG_ATTEST` |
| `integrity.chain_broken` | **dashboard** (dérivé) | `{ node, seq, expected_prev, got_prev }` | `prev_hash` incohérent |
| `integrity.fork_detected` | **dashboard** (dérivé) | `{ node, height, roots: [..], reporters: [..] }` | 2 racines pour une hauteur |
| `integrity.gap` | **dashboard** (dérivé) | `{ node, from_seq, to_seq }` | trou dans `seq` |

`integrity.*` ne viennent pas du terrain : ce sont des **conclusions** de l'ingest.

---

## 7. Domaine `relay` / `node` — santé

| `name` | Producteur | `payload` |
| --- | --- | --- |
| `relay.boot` | relais | `{ fw_version, reset_reason, secure_boot, flash_enc }` |
| `relay.health` | relais (60 s) | `{ uptime_s, rssi_avg, peers, gossip_cache, envelope_store, log_buffer_pct, logs_dropped, heap_free }` |
| `relay.wifi_up` / `relay.wifi_down` | relais | `{ ssid?, duration_s? }` |
| `relay.overloaded` | relais | `{ subsystem, action }` (`action ∈ {drop_pkt, refuse_envelope, evict_cache}`) |
| `node.clock_skew` | **dashboard** (dérivé) | `{ node, skew_ms }` |
| `client.summary` | client opt-in (périodique) | `{ msgs_sent, msgs_recv, peers_seen, app_version }` (agrégats anonymes) |

---

## 8. Exemple de batch MQTT (relais → `dengon/logs/relay-3f2a`)

```json
{
  "batch_id": "b1f0…",
  "node_id": "relay-3f2a9c",
  "events": [
    {
      "event_id": "9a1c…",
      "node_id": "relay-3f2a9c", "node_kind": "relay",
      "seq": 10432, "ts_ms": 1725800000123,
      "name": "pkt.relayed",
      "payload": {
        "msg_log_id": "4d5e6f7a8b9c0d1e",
        "type": 4, "ttl_in": 6, "ttl_out": 5, "fanout": 2,
        "from_peer": "a1b2c3d4e5f60718"
      }
    },
    {
      "event_id": "9a1d…",
      "node_id": "relay-3f2a9c", "node_kind": "relay",
      "seq": 10433, "ts_ms": 1725800000455,
      "name": "envelope.stored",
      "payload": {
        "msg_log_id": "4d5e6f7a8b9c0d1e",
        "recipient_tag": "00112233445566778899aabbccddeeff",
        "epoch_day": 20340, "copy_budget": 4
      }
    }
  ],
  "sig": "base64(ed25519(canonical_json(batch_without_sig)))"
}
```

---

## 9. Versionnement

Champ `schema_version` (entier) dans l'enveloppe du batch. Ajout de champ =
rétrocompatible ; suppression / changement de sens = incrément + adaptation ingest.
Le catalogue vit dans `crates/dengon-core/src/observability/catalog.rs` (enum +
tests de round-trip JSON) — ce document doit rester synchronisé avec lui.
