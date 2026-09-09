# Fiches modules

Une fiche par crate / composant réellement présent dans le dépôt. Modèle :
[`../templates/module.md`](../templates/module.md).

Chaque fiche explique le **code réel** : à quoi sert le module, comment il est
structuré, les types et fonctions importants (avec `chemin:ligne`), les flux de
données, ses dépendances, ses tests, ses limites connues.

Objectif : qu'une personne puisse comprendre le module **sans lire le code**, et
retrouver vite le bon fichier si elle veut y plonger.

## Index

| Module | Fiche | Dernière mise à jour |
|---|---|---|
| `process` (`.github/`) | [processus-github.md](processus-github.md) | 2026-09-09 |
| `dashboard/api` (FastAPI) | [dashboard-api.md](dashboard-api.md) | 2026-09-09 |

Chaque fiche sert aussi de **note d'onboarding** de son area (proposition
d'organisation §10.3 point 3).
