# Fiches modules

Une fiche par crate / composant réellement présent dans le dépôt. Modèle :
[`../templates/module.md`](../templates/module.md).

Chaque fiche explique le **code réel** : à quoi sert le module, comment il est
structuré, les types et fonctions importants (avec `chemin:ligne`), les flux de
données, ses dépendances, ses tests, ses limites connues.

Objectif : qu'une personne puisse comprendre le module **sans lire le code**, et
retrouver vite le bon fichier si elle veut y plonger.

## Index

Les crates Rust ont été posées à l'état **esquisse** par US-104 (socle du
sprint 2) ; la colonne « État » indique où chacune en est depuis. La fiche
`process` décrit l'**outillage**. Chaque fiche sert aussi de note d'onboarding
de son area (proposition d'organisation §10.3 point 3).

| Module | Fiche | État | Dernière mise à jour |
|---|---|---|---|
| `dengon-core` | [dengon-core.md](dengon-core.md) | en cours (+ `sync::status` US-211, `sync::routing` US-209, `protocol::fragment` US-202, `observability` US-208) | 2026-09-28 |
| `dengon-ble` | [dengon-ble.md](dengon-ble.md) | **contrat gelé** (US-105) | 2026-09-25 |
| `dengon-node` | [dengon-node.md](dengon-node.md) | esquisse | 2026-09-09 |
| `dengon-sim` | [dengon-sim.md](dengon-sim.md) | partiel (harness + réseau simulé + scénarios, US-221) | 2026-09-28 |
| `dengon-verify` | [dengon-verify.md](dengon-verify.md) | esquisse | 2026-09-09 |
| `dashboard/api` (FastAPI) | [dashboard-api.md](dashboard-api.md) | esquisse | 2026-09-28 |
| `contracts/events` (schémas + fixtures) | [contracts-events.md](contracts-events.md) | fonctionnel | 2026-09-10 |
| `dengon-ffi` | [dengon-ffi.md](dengon-ffi.md) | **contrat v0 gelé** (US-106) | 2026-09-28 |
| `process` (`.github/`) | [processus-github.md](processus-github.md) | 4 workflows requis présents (US-222) ; checks requis de `main` pas encore élargis | 2026-09-28 |
| `android-app` (`android/`) | [android-app.md](android-app.md) | partiel (service de fond + messagerie sur bouchon, US-214) | 2026-09-28 |
| `firmware-relay` (`firmware/dengon-relay/`) | [firmware-relay.md](firmware-relay.md) | partiel (transport NimBLE US-220 ; essai 2 cartes à faire) | 2026-09-28 |
| `android-app` (`android/`) | [android-app.md](android-app.md) | partiel (service de fond + appairage QR, US-215) | 2026-09-28 |
| `firmware-relay` (`firmware/dengon-relay/`) | [firmware-relay.md](firmware-relay.md) | esquisse | 2026-09-16 |
| `dashboard-web` (`dashboard/web/`) | [dashboard-web.md](dashboard-web.md) | fait (US-111) | 2026-09-25 |
