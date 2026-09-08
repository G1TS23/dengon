# docs/synthese — Contexte global du projet

Ce dossier **regroupe en une source unique** l'intégralité de la recherche et de
la conception produites séparément dans [`../powl/`](../powl/),
[`../oswin/`](../oswin/) et [`../olivier/`](../olivier/). But : qu'une nouvelle
conversation (ou un nouvel arrivant) ait **tout le contexte** sans relire les
34 fichiers d'origine.

| Fichier | Contenu |
| --- | --- |
| [`00-contexte-global.md`](00-contexte-global.md) | **Le document unique.** Fusion thématique complète (problème, état de l'art, architecture, protocole, format de trame, sécurité, cycle de vie, relais ESP32, dashboard, modèles de données, benchmarks, feuille de route, tests, glossaire, bibliographie). `powl` sert de colonne vertébrale ; le contenu propre à `oswin` (état de l'art, benchmarks, bibliographie) et à `olivier` (spéc comportementale, format de trame v0.1, analyse de besoins) y est fondu. |
| [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md) | **Tout ce qui n'est pas figé** : divergences entre les 3 dossiers, points ouverts internes à chacun, incohérences internes `powl` à réconcilier, « avis d'Olivier » à valider en équipe. Chaque fiche = sujet · sources · options avec arguments · piste de résolution · statut. |

## Périmètre

- **Recherche & conception uniquement.** L'état réel du code et le méta-projet
  (échéance, équipe, conventions de commit) ne sont **pas** dans ce dossier : ils
  restent dans [`../suivi/`](../suivi/) et `CLAUDE.md`.

## Hiérarchie des sources

- [`../powl/`](../powl/) reste la **source de vérité de conception** (désignée
  par `CLAUDE.md`). Sources de vérité internes : `03` (protocole/trame), `08`
  (événements), `09` (schémas de données).
- [`../oswin/`](../oswin/) et [`../olivier/`](../olivier/) restent la **matière
  première** : recherche sourcée, benchmarks, brouillons v0.1–v0.3. Ce dossier en
  est la synthèse, pas le remplacement.

## Règle de mise à jour

Ce dossier est un **document dérivé**. Quand `powl/`, `oswin/` ou `olivier/`
évoluent :

1. répercuter le changement dans la section thématique concernée de
   `00-contexte-global.md` (l'annexe §19 donne la correspondance section ↔
   fichiers sources) ;
2. si le changement crée, résout ou modifie une divergence / un point ouvert,
   mettre à jour la fiche correspondante de `01-sujets-a-trancher.md` (statut,
   options, piste de résolution) ;
3. si une décision d'équipe tranche un sujet, marquer la fiche `tranché` avec la
   référence, et refléter la décision dans `00-contexte-global.md`.

Une fois le code commencé, le suivi de ce qui est **réellement implémenté** se
fait dans [`../suivi/`](../suivi/) (règles dans `../suivi/README.md`), pas ici.
