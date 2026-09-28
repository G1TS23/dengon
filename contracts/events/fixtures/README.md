# Fixtures golden — événements d'observabilité

20 batches `POST /ingest/batch` valides et signés (clé de test
`../test-signing-key.json`). Générés par `../../tools/build_fixtures.py`,
vérifiés par `../../tools/validate.py`. **Ne pas éditer à la main.**

| Fichier | Événements | Ce que ça exerce |
| --- | --- | --- |
| `01-pkt-seen.json` | pkt.seen | cas nominal du domaine |
| `02-pkt-duplicate-relayed.json` | pkt.duplicate, pkt.relayed | cas nominal du domaine |
| `03-pkt-delivered-local.json` | pkt.delivered_local | cas nominal du domaine |
| `04-pkt-dropped.json` | pkt.dropped, pkt.dropped | drop avec et sans msg_log_id |
| `05-pkt-rejected.json` | pkt.rejected | cas nominal du domaine |
| `06-envelope-stored.json` | envelope.stored | cas nominal du domaine |
| `07-envelope-offered-handoff.json` | envelope.offered, envelope.handoff | cas nominal du domaine |
| `08-envelope-delivered.json` | envelope.delivered | cas nominal du domaine |
| `09-envelope-expired.json` | envelope.expired | cas nominal du domaine |
| `10-msg-queued.json` | msg.queued | cas nominal du domaine |
| `11-msg-handed-off.json` | msg.handed_off, msg.handed_off | un même msg_log_id remis via session puis via enveloppe |
| `12-msg-received.json` | msg.received | cas nominal du domaine |
| `13-msg-delivered.json` | msg.delivered | cas nominal du domaine |
| `14-msg-expired.json` | msg.expired | cas nominal du domaine |
| `15-ack-observed.json` | ack.observed | cas nominal du domaine |
| `16-peer-connected-disconnected.json` | peer.connected, peer.disconnected | dérivation d'un lien topologique |
| `17-peer-announce-seen.json` | peer.announce_seen | cas nominal du domaine |
| `18-attest.json` | attest.emitted, attest.observed | attestation propre + attestation d'un autre nœud |
| `19-relay-health.json` | relay.boot, relay.health, relay.wifi_up, relay.wifi_down, relay.overloaded | batch de 5 événements de santé relais |
| `20-client-summary.json` | client.summary | cas nominal du domaine |
