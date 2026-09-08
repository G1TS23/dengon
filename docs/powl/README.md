# Conception `dengon` — dossier `powl`

Messagerie **Bluetooth maillée** hors-ligne + **relais ESP32** + **dashboard
d'observabilité** sur VPS.

> 伝言 (*dengon*) : un message confié au réseau pour qu'il le transmette.

## Sommaire

| # | Document | Contenu |
| --- | --- | --- |
| 00 | [Vue d'ensemble](00-overview.md) | problème, acteurs, cas d'usage, périmètre, schéma global, matrice exigences → conception |
| 01 | [Benchmarks & décisions](01-benchmarks.md) | « réseau = blockchain ? » (analyse), plateforme app, crypto, firmware ESP32, stack dashboard |
| 02 | [Architecture](02-architecture.md) | `dengon-core`, trait `Transport`, découpage du dépôt, déploiement |
| 03 | [Protocole réseau](03-network-protocol.md) | format de paquet, types, fragmentation, dedup, TTL/flood, gossip, GATT |
| 04 | [Sécurité](04-security.md) | modèle de menace, identité, Noise XX/X, enveloppes scellées, QR + code, journal chaîné signé |
| 05 | [Cycle de vie d'un message](05-message-lifecycle.md) | statuts (en attente → parti → distribué → lu), accusés, outbox, reconnexion |
| 06 | [Relais ESP32](06-relay-esp32.md) | rôle, matériel, stack firmware, mémoire, MQTT, pannes |
| 07 | [Dashboard](07-dashboard.md) | ingestion, modèle de données, écrans, API, déploiement VPS, confidentialité |
| 08 | [Événements d'observabilité](08-observability-events.md) | catalogue normalisé des logs, règles de redaction |
| 09 | [Modèles de données](09-data-model.md) | schémas SQLite (client/relais) & PostgreSQL/Timescale (dashboard), formats de fils |
| 10 | [Périmètre MVP & roadmap](10-mvp-scope-roadmap.md) | Definition of Done, lots de livraison, hors périmètre, risques |
| 11 | [Stratégie de test & CI](11-testing-strategy.md) | pyramide de tests, simulateur, banc ESP32, recette terrain, CI |

## Décisions clés (résumé)

- **Réseau** : DTN à routage gossip + journal local chaîné signé — pas de consensus,
  pas de minage (cf. 01 §1). C'est l'interprétation retenue de « sécurité type
  blockchain ».
- **App** : cœur Rust partagé `dengon-core` ; app **Android/Kotlin** (MVP) via
  UniFFI ; `dengon-node` CLI pour tests & nœud fixe ; iOS post-MVP.
- **Crypto** : Noise `XX` (session live) + Noise `X` (enveloppes offline) + Ed25519 ;
  contact par **QR + code de vérification à 60 chiffres** ; TOFU.
- **Relais** : **ESP32-WROVER** + ESP-IDF + NimBLE ; réutilise `dengon-core` ; ne
  déchiffre rien ; remonte des logs signés en MQTT/TLS.
- **Dashboard** : **observabilité seule** — MQTT → Axum (Rust) → PostgreSQL/Timescale
  → React ; aucun contenu, `msgID` haché ; ne peut ni router ni injecter.
- **Statuts** : en attente → parti → distribué → lu (+ échec/expiré), pilotés par
  accusés signés, rattrapés à la reconnexion.

## Statut

Conception — **pas encore de code**. Prochaine étape : Lot 0 (spikes de dérisquage),
cf. [10-mvp-scope-roadmap.md](10-mvp-scope-roadmap.md).
