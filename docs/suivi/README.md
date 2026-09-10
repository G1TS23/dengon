# Suivi technique & apprentissage — `dengon`

Ce dossier est un **document vivant**. Il n'est **pas** la conception (ça, c'est
[`docs/powl/`](../powl/)). Il décrit **ce qui est réellement codé**, au fur et à
mesure, avec des explications compréhensibles, dans le but de :

1. garder une trace fidèle de l'avancement et des choix d'implémentation ;
2. expliquer **tout le contenu réel du code** (pas l'intention, le fait) ;
3. produire, à la fin, un support complet pour une **présentation orale**.

Il est mis à jour **pendant** le développement — par les développeurs et par les
assistants IA (Claude) après chaque tâche de code (voir [Règles de mise à jour](#règles-de-mise-à-jour)).

---

## Contenu du dossier

| Fichier | Rôle | Rythme de mise à jour |
|---|---|---|
| [`00-journal.md`](00-journal.md) | Journal chronologique **append-only** : une entrée par session de dev (quoi, pourquoi, comment vérifié). | à chaque session |
| [`01-etat-du-code.md`](01-etat-du-code.md) | **Pointeurs** vers où lire l'état réel. Change rarement. | rarement |
| [`02-avancement.md`](02-avancement.md) | Avancement par composant + outillage. **Édité en place** : on ne touche que sa/ses ligne(s). | quand un composant avance |
| [`modules/`](modules/) | Un fichier par crate / composant : explication du **code réel** (structs, fonctions clés, flux, dépendances). | quand le module bouge |
| [`03-ecarts-conception.md`](03-ecarts-conception.md) | Différences entre le code et [`docs/powl/`](../powl/), avec justification. | quand un écart apparaît |
| [`04-apprentissages.md`](04-apprentissages.md) | Volet « apprentissage » : notions comprises, pièges rencontrés, ressources utiles. | quand on apprend qqch |
| [`05-glossaire.md`](05-glossaire.md) | Termes du projet et du domaine, définis simplement. | au fil de l'eau |
| [`06-support-oral.md`](06-support-oral.md) | Synthèse finale pour l'oral : plan, messages clés, script de démo, questions/réponses anticipées. | consolidé en fin de projet, ébauché tôt |
| [`templates/`](templates/) | Modèles pour une entrée de journal et une fiche module. | — |

---

## Règles de mise à jour

### Pour un assistant IA (Claude) — à faire à la fin de toute tâche qui touche au code

1. **Ajouter une entrée** dans [`00-journal.md`](00-journal.md) en haut de la liste
   des entrées (ordre anti-chronologique), en suivant
   [`templates/entree-journal.md`](templates/entree-journal.md). Ne jamais réécrire
   ni supprimer une entrée passée. **Toujours un `---` puis une ligne vide** avant
   la nouvelle entrée.
2. **Mettre à jour sa ligne** dans [`02-avancement.md`](02-avancement.md) :
   modifier **uniquement** la/les ligne(s) du composant touché. Ne jamais
   réécrire le fichier entier. (`01-etat-du-code.md` n'est plus à toucher.)
3. **Créer ou mettre à jour** la fiche du module concerné dans [`modules/`](modules/)
   (modèle : [`templates/module.md`](templates/module.md)). Expliquer le **code réel**
   tel qu'il est, pas tel qu'il devrait être.
4. Si le code **s'écarte** de [`docs/powl/`](../powl/) : consigner l'écart et la
   raison dans [`03-ecarts-conception.md`](03-ecarts-conception.md).
5. Si une **notion** a été nécessaire (algo, protocole, API, piège) : une note dans
   [`04-apprentissages.md`](04-apprentissages.md).
6. Nouveau terme employé → l'ajouter au [`05-glossaire.md`](05-glossaire.md).
7. Rester **honnête et vérifiable** : si un test échoue, si une étape est bâclée ou
   simulée, l'écrire. Donner les commandes de vérification réellement exécutées.
8. Style : français, phrases courtes, orienté « qu'est-ce que ça fait et pourquoi ».
   Citer les fichiers en `chemin:ligne`. Pas de jargon non défini.

### Pour un développeur humain

Mêmes règles. Le journal se remplit aussi à la main quand on bricole sans IA.
En cas de gros refactor, mettre à jour la fiche module correspondante dans la foulée.

### Ce qu'on ne met PAS ici

- Le **pourquoi général / la cible** → [`docs/powl/`](../powl/).
- Le détail des tickets / la gestion de projet → outil de suivi habituel.
- La liste des **PR / branches en vol** → le board GitHub et `gh pr list`. Un
  markdown se périme, pas le board.
- Des secrets, clés, identifiants.

---

## Fusion (pourquoi ces fichiers ne provoquent plus de conflit)

`00-journal.md`, `02-avancement.md`, `03-ecarts-conception.md`,
`04-apprentissages.md`, `05-glossaire.md` et `modules/_index.md` sont déclarés
`merge=union` dans le [`.gitattributes`](../../.gitattributes) racine.

- **Effet :** quand deux branches ajoutent chacune du contenu au même endroit,
  git **garde les deux** au lieu de lever un conflit. Rien à installer : le
  `.gitattributes` est versionné.
- **Ce que ça ne fait pas :** trier. `union` concatène les deux côtés dans un
  ordre non garanti. Après une fusion, **relire le haut du journal** et
  remettre l'ordre anti-chronologique si besoin (les entrées sont datées).
- **Pour que ça reste propre :**
  - une entrée de journal = un bloc précédé de `---` + ligne vide ;
  - on **n'édite jamais** une entrée passée, on ajoute une entrée rectificative ;
  - dans `02-avancement.md`, on ne touche que **sa** ligne ;
  - si on doit modifier l'intro d'un de ces fichiers (rare), le faire dans une
    PR seule pour éviter une duplication de paragraphe par `union`.
- **`01-etat-du-code.md` n'est pas en `union`** : il ne contient que des
  pointeurs, il ne bouge presque pas.

---

## En fin de projet

[`06-support-oral.md`](06-support-oral.md) est consolidé à partir de tout le reste :
il doit permettre de présenter le projet **sans relire le code**, en comprenant
chaque brique réellement livrée. Les fiches [`modules/`](modules/) servent de
réservoir d'explications détaillées pour les questions du jury.
