# Journal de développement

Journal **append-only**. Entrée la plus récente en haut. Une entrée par session de
travail sur le code. Modèle : [`templates/entree-journal.md`](templates/entree-journal.md).

> On ne modifie jamais une entrée passée. Pour corriger une info, on ajoute une
> nouvelle entrée qui rectifie.

---

<!-- NOUVELLES ENTRÉES ICI (juste en dessous de cette ligne) -->

---

## 2026-09-09 — Contrat des événements d'observabilité + 20 fixtures golden (US-107)

**Auteur :** Claude (Sonnet 5)
**Périmètre :** `contracts/` (nouveau), `.github/workflows/contracts.yml`,
`docs/suivi/modules/contracts-events.md` (créée), `_index.md`,
`01-etat-du-code.md`, `03-ecarts-conception.md`.
**Lot :** Lot 0 — Fondations (issue #7, US-107). Branche
`contract/US-107-enveloppe-evenement`, prise « en attendant les review » de la
PR #59 (US-110).

### Fait
- **`contracts/events/`** :
  - `envelope.schema.json`, `batch.schema.json` — JSON Schema draft 2020-12,
    **stricts** (`additionalProperties: false`) pour l'enveloppe et le batch
    `POST /ingest/batch`.
  - `payloads.schema.json` — contraintes de `payload` par nom d'événement,
    **généré** depuis `tools/catalogue.py` (un `allOf` de `if name==X then …`).
  - `CANONICAL.md` — **fait foi** : forme canonique du JSON signé (clés triées,
    séparateurs compacts, UTF-8) + procédure de signature Ed25519 d'un batch.
  - `test-signing-key.json` — clé Ed25519 de test (graine déterministe,
    publique, jamais de prod).
  - `fixtures/*.json` — **20 batches** valides et signés, couvrant **28 noms
    d'événements** (tout le périmètre MVP : `msg.read` / `read.observed` et les
    `integrity.*` / `node.clock_skew` dérivés sont explicitement exclus).
- **`contracts/tools/`** : `catalogue.py` (source lisible + helpers
  `canonical_json` / `event_id`), `build_fixtures.py` (génère fixtures +
  `payloads.schema.json`), `validate.py` (schéma + payload vs catalogue +
  cohérence `event_id`/`node_id` + **signature Ed25519** + **redaction** +
  fraîcheur du schéma généré + couverture du catalogue).
- **`contracts/pyproject.toml` + `uv.lock`** : outillage `uv` (jsonschema,
  pynacl, referencing ; ruff en dev), cohérent avec `dashboard/api`.
- **`.github/workflows/contracts.yml`** : `ruff` + « fixtures régénérées à
  l'identique » (`git diff --exit-code`) + `validate.py`, filtré
  `paths: contracts/**`, actions épinglées au SHA, `uv --no-build`.

### Pourquoi / décisions
- **`contracts/` en dossier top-level** : artefact neutre en langage, consommé
  par `dashboard/` (Python) **et** `crates/` (Rust) **et** le firmware (C).
  Écart mineur au layout de `docs/synthese/04` §5 — consigné.
- **Un seul `sig` par batch** (et non par événement) : c'est ce que montre
  l'exemple de `docs/synthese/09` §9 ; l'intégrité fine vient du journal chaîné
  (`seq` + `prev_hash`), `event_id` fait la déduplication.
- **`msg_log_id` = 16 hex (8 octets)** : les docs se contredisent (`[0..16]`
  vs « 16 o » vs `[:16]`), le seul exemple concret fait 16 hex. Réconciliation
  consignée dans `03-ecarts-conception.md`.
- **Fixtures générées puis committées** (pas régénérées en CI) : un diff montre
  toute dérive ; la CI vérifie que la régénération ne bouge rien.
- **Catalogue en module Python** (`catalogue.py`) comme source, `.schema.json`
  dérivé : évite de maintenir un gros JSON Schema à la main, garde une source
  unique.

### Écarts vs conception
- `contracts/` ajouté au layout du dépôt — `03-ecarts-conception.md`.
- Longueur de `msg_log_id` tranchée à 8 octets — `03-ecarts-conception.md`.
- Rien d'autre : les schémas transcrivent `docs/powl/08` et `docs/synthese/09`
  §9 sans les contredire.

### Appris
- **JSON Schema `$ref` relatif + `jsonschema` Python** : depuis la 4.18, la
  résolution passe par un `referencing.Registry` qu'il faut peupler à la main
  (`Resource.from_contents`) — l'ancien `RefResolver` est déprécié. Enregistrer
  la ressource **et** sous son `$id` **et** sous son nom de fichier.
- **Ed25519 = 64 octets de signature → 88 caractères base64** terminés par
  `==` (pattern de schéma `^[A-Za-z0-9+/]{86}==$`).

### État après cette session
- `contracts/events/` : contrat complet, `validate.py` vert, 20 fixtures
  prêtes à être consommées par US-208 (core) et US-217 (dashboard).
- Fiche `modules/contracts-events.md` créée ; `_index.md` et
  `01-etat-du-code.md` à jour.
- **Contrat à annoncer « gelé »** au point d'équipe (DoD §7.2, type contrat).

### Vérification (commandes réellement exécutées)
```
$ cd contracts && uv sync
$ uv run python tools/build_fixtures.py
  20 fixtures écrites, 28 noms d'événements couverts.
$ uv run python tools/validate.py
  ✓ 20 fixtures valides — 28 noms d'événements couverts.
$ uv run ruff check .
  All checks passed!
$ uv run python tools/build_fixtures.py && git diff --stat -- events/
  (aucun diff — régénération stable)
```
- La CI `contracts` n'a pas encore tourné : à l'ouverture de la PR.

---

## 2026-09-09 — La protection de `main` est active, et ma vérification était creuse (US-113)

**Auteur :** Claude (Opus 5)
**Périmètre :** `docs/suivi/modules/processus-github.md`, `01-etat-du-code.md`,
`04-apprentissages.md`. Aucun fichier `.github/` modifié.
**Lot :** Lot 0 — Fondations (issue #13, US-113).

> Deuxième entrée **rectificative** du jour. Journal append-only : on rectifie
> en ajoutant, on ne réécrit pas.

### Ce qui s'est passé

`G1TS23` a activé la protection de `main` le 09/09 à **15:18** — « Review
required » (1 approbation) et **`core` en check requis**. Le 5ᵉ critère
d'acceptation de l'issue #13 est donc atteint. Mais elle a été activée **avant**
le merge de la PR #57, exactement le cas que la fiche demandait d'éviter.

Effet immédiat, constaté : **#56 et #58 sont `BLOCKED`** sur un check `core` que
rien ne peut rapporter — `core.yml` n'existe ni sur `main`, ni sur leurs
branches. GitHub ne les fait pas échouer, il les **attend**. #57, elle, est
verte : son *head* porte `core.yml`, donc le workflow tourne sur son *merge
ref*.

Sortie, dans cet ordre : merger #57 → fusionner `main` dans les branches de #56
et #58 pour que leur *merge ref* contienne le workflow → `core` se met enfin à
rapporter. Et il rapportera **même sur ces PR sans une ligne de Rust**, grâce à
l'écart de l'US-104 (filtrage dans le job, pas sur le déclencheur). Sans cet
écart, ces deux PR seraient définitivement bloquées : c'est la démonstration
grandeur nature de l'écart.

### Mon erreur de méthode

J'ai écrit à plusieurs reprises « `branches/main/protection` → 404 → aucune
protection ». **Ce raisonnement est faux.** Cet endpoint renvoie 404 à un compte
**non admin**, que la protection existe ou non — et on est authentifié en
`POWLAIR`. La conclusion se trouvait être vraie à l'instant où je l'écrivais,
mais la preuve ne valait rien : la même commande renvoie toujours 404
aujourd'hui, alors que la protection est bien là.

Vérification fiable pour un non-admin : l'effet observable sur une PR,
`gh pr view <n> --json mergeStateStatus,statusCheckRollup`. Note ajoutée à
`04-apprentissages.md` sous le titre « Un 404 d'API GitHub peut vouloir dire
"tu n'as pas le droit de savoir" ».

### Aussi constaté

- La revue de `G1TS23` (15:18:13) est passée en `DISMISSED` à 15:22:16, au
  moment précis de mon push : `dismiss_stale_reviews` est actif et fonctionne.
  Toute nouvelle poussée invalide l'approbation — il faut donc pousser d'abord,
  demander la revue ensuite.
- `gh api repos/G1TS23/dengon/rulesets` → `[]` et `rules/branches/main` → `[]` :
  la protection est **classique**, pas un ruleset. Ces endpoints n'auraient de
  toute façon rien montré à un non-admin.

### État après cette session

- Issue #13 : les 5 critères sont désormais **couverts**, le dernier par une
  action de `G1TS23` et non par cette PR. Reste la preuve par l'usage (DoR n°7)
  — elle est en train de se faire toute seule : #58 est visiblement bloquée
  faute d'approbation.
- PR #58 : `BLOCKED`, GitGuardian et SonarCloud verts, `core` en attente
  perpétuelle jusqu'au merge de #57.

### Vérification (commandes réellement exécutées)

```
$ gh pr view 57 --json mergeStateStatus,statusCheckRollup
core SUCCESS · GitGuardian SUCCESS · SonarCloud SUCCESS · statut BLOCKED

$ gh pr view 56 --json mergeStateStatus   → BLOCKED
$ gh pr view 58 --json mergeStateStatus   → BLOCKED

$ git ls-tree origin/main -- .github/workflows/core.yml                    → ABSENT
$ git ls-tree HEAD -- .github/workflows/core.yml                           → ABSENT
$ git ls-tree origin/build/US-104-workspace-cargo -- .github/workflows/core.yml → présent

$ gh api repos/G1TS23/dengon/pulls/58/reviews
G1TS23 · DISMISSED · 2026-09-09T15:18:13Z

$ gh api repos/G1TS23/dengon/branches/main/protection
404 — et ce 404 ne prouve toujours rien (compte non admin)
```

---

## 2026-09-09 — Vérification après push : deux affirmations rectifiées (US-113)

**Auteur :** Claude (Opus 5)
**Périmètre :** `docs/suivi/modules/processus-github.md`, `01-etat-du-code.md`,
corps de la PR #58. Aucun fichier `.github/` modifié.
**Lot :** Lot 0 — Fondations (issue #13, US-113).

> Entrée **rectificative** de celle du même jour ci-dessous : le journal est
> append-only, on ne réécrit pas une entrée passée.

### Fait

- Rejoué toute la vérification une fois la branche poussée, y compris les
  contrôles qui n'étaient pas faisables avant.
- **`CODEOWNERS` validé par GitHub** :
  `gh api "repos/G1TS23/dengon/codeowners/errors?ref=chore/US-113-github-setup"`
  → `{"errors":[]}`. L'entrée précédente le listait comme non vérifié : ce
  n'est plus le cas.
- Corrigé la fiche `modules/processus-github.md` et `01-etat-du-code.md` sur
  les deux points ci-dessous, et le corps de la PR #58 en conséquence.

### Ce qui était faux dans l'entrée précédente

1. **« Aucun check ne tournera sur cette PR ».** Faux. Deux applications GitHub
   sont installées sur le dépôt et rapportent un statut sur chaque PR :
   **GitGuardian Security Checks** et **SonarCloud Code Analysis**. Les deux
   étaient `SUCCESS` sur la PR #58. Elles sont candidates à devenir des checks
   requis en plus de `core` — mais la DoD §7.1 ne les nomme pas, donc c'est une
   décision d'équipe, pas une évidence.
2. **Les assignations des issues #4 et #13.** J'avais repris l'affirmation
   qu'elles étaient sur `G1TS23`. Vérifié : **les deux sont assignées à
   `POWLAIR`**. #4 est donc conforme à §13 ; #13 y est attribuée à Olivier mais
   portée par Paul dans les faits — sans conséquence, §13 se dit « indicative,
   à ajuster en réunion ».

### Découvert au passage

- **Le squash-only et la suppression automatique des branches étaient DÉJÀ
  actifs** avant l'US-113 : `gh api repos/G1TS23/dengon` →
  `squash: true, merge_commit: false, rebase: false, suppr_branche: true`.
  Deux des six réglages du 5ᵉ critère d'acceptation sont donc acquis sans rien
  faire, et la seconde commande admin de la fiche est un no-op. Elle y reste :
  un réglage de dépôt se change d'un clic sans laisser de trace, la commande
  sert alors à le remettre.
- **`CODEOWNERS` se lit depuis la branche de BASE, pas depuis celle de la PR.**
  `gh pr view 58 --json reviewRequests` renvoyait `[]` alors que le fichier
  existait sur la branche. Tant qu'il n'est pas sur `main`, il ne gouverne
  rien : la PR qui installe la revue croisée est mécaniquement la seule à ne
  pas en bénéficier. Relecteur demandé à la main (`@G1TS23`).

### État après cette session

- L'issue #13 reste ouverte : **4 critères sur 5**. Le 5ᵉ se décompose en six
  réglages, dont deux (squash, suppression auto) sont acquis et quatre (PR
  obligatoire, 1 approbation, checks requis, historique linéaire) demandent la
  commande admin.
- PR #58 : `MERGEABLE`, deux checks verts, revue demandée à `@G1TS23`.
- `01-etat-du-code.md` mis à jour : oui. Fiche module mise à jour : oui.

### Vérification (commandes réellement exécutées)

```
$ gh api "repos/G1TS23/dengon/codeowners/errors?ref=chore/US-113-github-setup"
{"errors":[]}

$ gh api repos/G1TS23/dengon --jq '{squash,merge_commit,rebase,suppr_branche}'
squash true · merge_commit false · rebase false · suppr_branche true

$ gh pr view 58 --json statusCheckRollup
GitGuardian Security Checks  SUCCESS
SonarCloud Code Analysis     SUCCESS

$ gh issue view 4 / 13 --json assignees
#4 → POWLAIR   #13 → POWLAIR

$ gh api repos/G1TS23/dengon/branches/main/protection   → 404 (inchangé)
$ gh api repos/G1TS23/dengon/rulesets                   → []  (inchangé)
```

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
