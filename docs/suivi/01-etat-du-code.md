# État du code — photo courante

> Réécrit à chaque session. Décrit ce qui **existe et fonctionne aujourd'hui**, pas
> ce qui est prévu. Dernière mise à jour : **2026-09-08**.

## Résumé

Aucun code applicatif écrit. Le dépôt contient : la conception
([`docs/powl/`](../powl/)), ce dossier de suivi, un `.gitignore`, des hooks git
(`.githooks/`).

## Avancement par composant

| Composant | Prévu (conception) | Réel | Avancement | Fiche |
|---|---|---|---|---|
| `dengon-core` | crate Rust : protocol, crypto, store, sync, ledger, observability | — | 0 % | — |
| `dengon-ble` | trait Transport + impl btleplug | — | 0 % | — |
| `dengon-node` | binaire CLI (nœud headless) | — | 0 % | — |
| `dengon-sim` | simulateur multi-nœuds | — | 0 % | — |
| `dengon-ffi` | bindings UniFFI | — | 0 % | — |
| App Android | Kotlin + Compose | — | 0 % | — |
| Firmware `dengon-relay` | ESP-IDF + NimBLE | — | 0 % | — |
| Dashboard `api` | Axum + Postgres/Timescale | — | 0 % | — |
| Dashboard `web` | React + TS | — | 0 % | — |
| Déploiement VPS | docker-compose + Caddy + Mosquitto | — | 0 % | — |

## Ce qui tourne / commandes utiles

_(à remplir quand il y aura du code : comment builder, lancer, tester chaque partie)_

## Prochaines étapes

Voir [`docs/powl/10-mvp-scope-roadmap.md`](../powl/10-mvp-scope-roadmap.md) — **Lot 0** :
1. Spike A — `dengon-core` (protocol + crypto) cross-compile pour xtensa-esp32 ?
2. Spike B — `btleplug` en rôle GATT peripheral sous Linux.
3. Spike C — Android `BluetoothGattServer` + scan en foreground service.
