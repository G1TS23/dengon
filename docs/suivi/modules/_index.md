# Fiches modules

Une fiche par crate / composant réellement présent dans le dépôt. Modèle :
[`../templates/module.md`](../templates/module.md).

Chaque fiche explique le **code réel** : à quoi sert le module, comment il est
structuré, les types et fonctions importants (avec `chemin:ligne`), les flux de
données, ses dépendances, ses tests, ses limites connues.

Objectif : qu'une personne puisse comprendre le module **sans lire le code**, et
retrouver vite le bon fichier si elle veut y plonger.

## Index

Le code applicatif n'a pas commencé. La seule fiche existante décrit
l'**outillage de processus** — elle sert aussi de note d'onboarding de l'area
`process` (§10.3 point 3).

| Module | Fiche | Dernière mise à jour |
|---|---|---|
| `process` (`.github/`) | [processus-github.md](processus-github.md) | 2026-09-09 |
