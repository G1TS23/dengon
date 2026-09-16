# Fiches modules

Une fiche par crate / composant réellement présent dans le dépôt. Modèle :
[`../templates/module.md`](../templates/module.md).

Chaque fiche explique le **code réel** : à quoi sert le module, comment il est
structuré, les types et fonctions importants (avec `chemin:ligne`), les flux de
données, ses dépendances, ses tests, ses limites connues.

Objectif : qu'une personne puisse comprendre le module **sans lire le code**, et
retrouver vite le bon fichier si elle veut y plonger.

## Index

Les six crates Rust sont à l'état **esquisse** : elles compilent et sont
testées, mais aucune n'implémente encore de logique métier (US-104, socle du
sprint 2). Chaque fiche sert aussi de note d'onboarding de son area (proposition
d'organisation §10.3 point 3).

| Module | Fiche | État | Dernière mise à jour |
|---|---|---|---|
| `dengon-core` | [dengon-core.md](dengon-core.md) | esquisse | 2026-09-09 |
| `dengon-ble` | [dengon-ble.md](dengon-ble.md) | esquisse | 2026-09-09 |
| `dengon-node` | [dengon-node.md](dengon-node.md) | esquisse | 2026-09-09 |
| `dengon-sim` | [dengon-sim.md](dengon-sim.md) | esquisse | 2026-09-09 |
| `dengon-verify` | [dengon-verify.md](dengon-verify.md) | esquisse | 2026-09-09 |
| `dengon-ffi` | [dengon-ffi.md](dengon-ffi.md) | esquisse | 2026-09-09 |
| `contracts/events` (schémas + fixtures) | [contracts-events.md](contracts-events.md) | fonctionnel | 2026-09-10 |
| `process` (`.github/`) | [processus-github.md](processus-github.md) | — | 2026-09-09 |
| `android-app` (`android/`) | [android-app.md](android-app.md) | esquisse | 2026-09-09 |

Pas encore de fiche (le composant n'existe pas) : application Android, firmware
`dengon-relay`, dashboard `api` (fiche sur la branche PR #59) et `web`.
Pas encore de fiche (le composant n'existe pas) : firmware `dengon-relay`,
dashboard `api` et `web`.
