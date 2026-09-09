# Journal de développement

Journal **append-only**. Entrée la plus récente en haut. Une entrée par session de
travail sur le code. Modèle : [`templates/entree-journal.md`](templates/entree-journal.md).

> On ne modifie jamais une entrée passée. Pour corriger une info, on ajoute une
> nouvelle entrée qui rectifie.

---

<!-- NOUVELLES ENTRÉES ICI (juste en dessous de cette ligne) -->

---

## 2026-09-09 — Outillage GitHub : formulaires, CODEOWNERS, labels versionnés (US-113)

**Auteur :** Claude (Opus 5)
**Périmètre :** `.github/ISSUE_TEMPLATE/{user-story,spike,bug,config}.yml`,
`.github/PULL_REQUEST_TEMPLATE.md`, `.github/CODEOWNERS`, `.github/labels.yml`,
`.github/workflows/labels.yml`, `docs/suivi/`.
**Lot :** Lot 0 — Fondations (issue #13, US-113, sprint S1, jalon J0, area `process`).

### Fait

- **Trois formulaires d'issue** (*issue forms* YAML, pas templates Markdown) :
  `user-story` (identifiant, area, sprint, jalon, estimation, MoSCoW,
  contrainte dure, puis contexte / critères / dépendances / référence /
  stratégie de test, et les 8 cases de la DoR §6), `spike` (question fermée,
  timebox, critère d'arrêt, livrable = décision écrite), `bug` (observé,
  attendu, repro, où, gravité).
- **`config.yml`** : `blank_issues_enabled: false` — l'issue vierge
  contournait tout le dispositif — plus trois liens de sortie (board
  *Démarrables maintenant*, `docs/synthese/`, `docs/suivi/`).
- **`PULL_REQUEST_TEMPLATE.md`** : les 8 points de la DoD §7.1 en cases à
  cocher, la DoD §7.2 par type d'US dans un bloc repliable, un champ « ce que
  la revue doit regarder en priorité », un champ « vérification » pour les
  commandes réellement exécutées et un champ « ce qui reste ouvert ».
- **`CODEOWNERS`** : traduction de §10.3 avec les vrais comptes (§13 :
  POWLAIR = Paul, OswinFreyr = Tanguy, G1TS23 = Olivier). Filet `*` en tête,
  puis `/crates/`, `/android/`, `/firmware/`, `/dashboard/`, `/docs/`,
  `/.github/`.
- **`labels.yml`** : les 32 labels versionnés, couleurs et descriptions
  recopiées depuis `gh label list` — le fichier décrit l'existant.
- **`workflows/labels.yml`** : synchro `workflow_dispatch` uniquement, avec
  `dry_run` à `true` et `supprimer` à `false` par défaut.
- **Fiche module** `modules/processus-github.md` : elle porte les **commandes
  admin exactes** de protection de `main`, pour qu'elles soient versionnées et
  rejouables plutôt que perdues dans une conversation.

### Pourquoi / décisions

- **Formulaires YAML plutôt que templates Markdown.** Un template Markdown se
  soumet vide ; un formulaire refuse la soumission tant qu'un champ `required`
  est vide. C'est la différence entre une DoR affichée et une DoR appliquée —
  et c'est tout l'objet de l'US.
- **Mais les 8 cases de la DoR restent non bloquantes.** Une issue doit pouvoir
  naître incomplète : c'est l'entrée en *sprint* qui exige les 8 cochées (§6).
  Les rendre obligatoires à l'ouverture aurait produit des cases cochées sans
  être lues, soit l'inverse du but.
- **Deux comptes par chemin dans `CODEOWNERS`, jamais un.** Un seul nom bloque
  le dépôt dès que la personne est absente ; deux n'affaiblissent rien, parce
  que **GitHub interdit à l'auteur d'une PR de s'auto-approuver au titre de
  CODEOWNERS**. La revue croisée est donc mécanique, pas conventionnelle.
- **La ligne `*` est placée en tête.** Dans `CODEOWNERS`, c'est la **dernière**
  ligne qui correspond qui gagne : en bas, elle aurait annulé toutes les règles
  par chemin. Erreur classique, silencieuse, et qui aurait vidé l'US de son
  contenu.
- **Un seul check requis (`core`) dans les commandes de protection.** La DoD
  §7.1 point 3 en nomme quatre ; trois n'existent pas encore. Les déclarer
  requis aurait laissé toutes les PR sur « Expected — Waiting for status to be
  reported ». Écart consigné.
- **Synchro des labels manuelle, en simulation par défaut.** Les 32 labels sont
  posés sur 55 issues et servent de filtres au board : un
  `delete-other-labels` déclenché par un push les effacerait sans revue
  possible. Trois gestes délibérés sont nécessaires pour détruire quoi que ce
  soit.
- **Action `EndBug/label-sync` épinglée sur un SHA complet** (`5207415…`,
  v2.3.3), pas sur un tag : un tag est mutable. Même règle que `core.yml`.

### Écarts vs conception

- **Checks requis réduits à `core`** au lieu des quatre exigés — reporté dans
  `03-ecarts-conception.md`, avec la condition de levée.
- **`good first issue`** (nom réel GitHub, avec espaces) conservé plutôt que le
  `good-first-issue` de §5.3 — reporté également.

### Appris

- Formulaires d'issue YAML vs templates Markdown ; règle de priorité de
  `CODEOWNERS` et interdiction de l'auto-approbation ; pourquoi un check requis
  inexistant fige un dépôt ; `workflow_dispatch` n'est visible que depuis la
  branche par défaut. Notes ajoutées à `04-apprentissages.md`.

### État après cette session

- Les **quatre livrables versionnés** de l'issue #13 existent. Le cinquième
  critère — **protection de `main` — n'est PAS fait** : `gh api
  repos/G1TS23/dengon/collaborators` confirme que seul `G1TS23` est admin, et
  le poste est authentifié en `POWLAIR`. Les commandes sont écrites et prêtes
  dans `modules/processus-github.md`, à lancer par Olivier **après le merge de
  la PR #57** (c'est elle qui crée le check `core`).
- Par conséquent l'issue #13 **reste ouverte** : la PR référence `Refs #13`, pas
  `Closes #13`.
- Fiche module créée : `modules/processus-github.md` (+ ligne dans `_index.md`).
- `01-etat-du-code.md` mis à jour : oui.
- **Conflit attendu à la fusion** avec la PR #57 (US-104) sur `00-journal.md`,
  `01-etat-du-code.md` et `modules/_index.md` : les deux branches partent de
  `main` et écrivent aux mêmes endroits. Résolution : garder les deux entrées
  de journal (la plus récente en haut) et fusionner les deux tableaux.

### Vérification (commandes réellement exécutées)

```
$ python3 -c "import yaml,glob; [yaml.safe_load(open(f)) for f in glob.glob('.github/**/*.yml', recursive=True)]"
6 fichiers, tous valides

$ python3 <script jetable de contrôle du schéma des formulaires>
SCHÉMA : ok  (id uniques, options présentes, `required` sous `validations`)

$ gh label list --limit 100 --json name,color,description  |  diff avec .github/labels.yml
labels.yml : 32   GitHub : 41
DIFF SUR LES LABELS DÉCLARÉS : aucun
9 labels par défaut hors fichier (enhancement, wontfix, …) — conservés car `supprimer` vaut false

$ gh api repos/G1TS23/dengon/branches/main/protection
404 Not Found          → aucune protection aujourd'hui
$ gh api repos/G1TS23/dengon/rulesets
[]                     → aucun ruleset non plus
```

**Ce qui n'a PAS pu être vérifié, et pourquoi :**

- `gh api repos/G1TS23/dengon/codeowners/errors?ref=…` — demande que la branche
  soit poussée ; elle ne l'était pas au moment d'écrire cette entrée.
- Le rendu réel des formulaires sur `/issues/new/choose` — GitHub ne lit
  `ISSUE_TEMPLATE/` que depuis la branche par défaut.
- Le dispatch du workflow `labels` en `dry_run` — `workflow_dispatch`
  n'apparaît que si le fichier est déjà sur `main`.
- La **preuve par l'usage** demandée par la DoR n°7 de l'issue (« une PR de
  test est bloquée tant que les checks ne sont pas verts ») — impossible sans
  la protection, donc sans droit admin.

---

## 2026-09-08 — Mise en place du dossier de suivi

**Auteur :** Claude (Sonnet 5)
**Périmètre :** documentation seule, aucun code applicatif.

### Fait

- Création de [`docs/suivi/`](.) : journal, état du code, fiches modules, écarts,
  apprentissages, glossaire, support oral, templates.
- Ajout des règles de mise à jour (voir [`README.md`](README.md)).

### Décisions

- Le suivi est séparé de la conception (`docs/powl/`) : ici = ce qui est **codé**,
  là-bas = ce qui est **visé**.
- Journal anti-chronologique, append-only, pour garder l'historique d'apprentissage
  intact jusqu'à l'oral.

### État

- Aucune crate créée. Prochaine étape réelle : **Lot 0** de
  [`docs/powl/10-mvp-scope-roadmap.md`](../powl/10-mvp-scope-roadmap.md) (spikes).

### Vérification

- `ls docs/suivi/` → structure en place. Rien à compiler.
