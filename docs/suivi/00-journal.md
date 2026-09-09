# Journal de développement

Journal **append-only**. Entrée la plus récente en haut. Une entrée par session de
travail sur le code. Modèle : [`templates/entree-journal.md`](templates/entree-journal.md).

> On ne modifie jamais une entrée passée. Pour corriger une info, on ajoute une
> nouvelle entrée qui rectifie.

---

<!-- NOUVELLES ENTRÉES ICI (juste en dessous de cette ligne) -->

---

## 2026-09-09 — `docs/suivi/` : fin des conflits de merge (US-115)

**Auteur :** Claude (Sonnet 5)
**Périmètre :** `.gitattributes` (nouveau), `docs/suivi/02-avancement.md`
(nouveau), `docs/suivi/01-etat-du-code.md`, `docs/suivi/README.md`, `CLAUDE.md`.
**Lot :** Lot 0 — Fondations (issue #61, US-115). Branche
`chore/US-115-suivi-merge-union`.

### Fait
- **`.gitattributes` racine** : `merge=union` sur les 6 fichiers de suivi
  « append » (`00-journal`, `02-avancement`, `03-ecarts-conception`,
  `04-apprentissages`, `05-glossaire`, `modules/_index`). Git garde **les deux
  côtés** au lieu de lever un conflit. Committé → rien à installer côté
  collègues.
- **`02-avancement.md`** (nouveau) : les tableaux « Avancement par composant »
  et « Outillage et processus » sortent de `01`. Consigne : **édité en place**,
  on ne touche que sa/ses ligne(s). Corrigé au passage `Dashboard api` (« Axum +
  Postgres » → « FastAPI + SQLite + SSE », A-5), `Dashboard web` et
  `Déploiement VPS` de la même façon ; ajouté les lignes `contracts/` et
  `.gitattributes`.
- **`01-etat-du-code.md`** : réduit à un tableau de **pointeurs** (où lire
  quoi) + un résumé d'une ligne + les commandes utiles. Plus de liste des PR en
  vol (c'est le rôle du board / `gh pr list`). Ne bouge quasiment plus → pas
  de conflit, donc **pas** en `merge=union`.
- **`README.md` du dossier** : nouvelle section « Fusion » (ce que fait / ne
  fait pas `union`, comment garder le journal lisible) ; règle « `---` + ligne
  vide avant chaque entrée » ; l'étape 2 des règles de mise à jour vise
  `02-avancement.md`.
- **`CLAUDE.md`** : l'étape « Rafraîchir `01-etat-du-code.md` » devient
  « Mettre à jour sa ligne dans `02-avancement.md` ».

### Pourquoi / décisions
- **`union` plutôt qu'un fichier par entrée** (option C de la discussion) :
  coût ~0, aucun changement de workflow, aucune migration. Si `union` produit
  trop de désordre à l'usage, on escaladera vers un fichier par entrée.
- **`01` hors `union`** : c'est une réécriture, pas un ajout — `union` y
  dupliquerait des paragraphes. On le vide de tout ce qui est volatil à la
  place.
- **Ne plus lister les PR en vol dans le markdown** : ça se périme à chaque
  merge, le board est toujours juste.

### Écarts vs conception
- Aucun vs `docs/powl/` / `docs/synthese/` (qui ne prescrivent pas la
  mécanique du dossier de suivi). Changement interne à la convention
  `docs/suivi/`, documenté dans son `README.md`.

### Appris
- **`merge=union` est un pilote intégré à git** (aucune config `merge.*.driver`
  à poser). `.gitattributes` versionné = actif pour tout le monde sans geste.
- Il **concatène sans trier** : après fusion de deux `append` au même ancrage,
  les deux blocs peuvent se coller sans ligne vide → relecture rapide du haut
  du journal. Noté dans `04-apprentissages.md`.

### État après cette session
- `git check-attr merge` renvoie `union` sur les 6 fichiers, `unspecified` sur
  `01`.
- Test concret : deux branches ajoutent chacune une entrée en tête de
  `00-journal.md` → `git merge` **sans conflit**, les deux entrées présentes
  (une ligne vide à remettre à la main entre les deux — cosmétique).
- `02-avancement.md` à jour : oui. Pas de fiche module (chantier `process`).

### Vérification (commandes réellement exécutées)
```
$ git check-attr merge -- docs/suivi/00-journal.md
  docs/suivi/00-journal.md: merge: union
$ git check-attr merge -- docs/suivi/01-etat-du-code.md
  docs/suivi/01-etat-du-code.md: merge: unspecified

# deux append concurrents sur 00-journal.md, puis merge
$ git merge --no-edit tmp/union-test-A
  Auto-merging docs/suivi/00-journal.md
  Merge made by the 'ort' strategy.
$ grep -c '^<<<<<<<' docs/suivi/00-journal.md
  0   (aucun marqueur de conflit — les entrées A et B sont toutes deux là)
```
- Reste à confirmer sur la **première PR concurrente réelle** que GitHub
  n'affiche plus `DIRTY` pour un simple ajout d'entrée de suivi.

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

## 2026-09-09 — Workspace Cargo, rustfmt/clippy et CI `core` (US-104)

**Auteur :** Claude (Opus 5)
**Périmètre :** `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`,
`crates/` (6 crates + `rustfmt.toml` + `clippy.toml`),
`.github/workflows/core.yml`, `.gitignore`.
**Lot :** Lot 0 — Fondations & spikes (issue #4, US-104, sprint S1, jalon J0).

### Fait

- **Toolchain** : installation de rustup sur le poste (rien n'était installé) —
  `rustc 1.98.1`. Figée pour tout le dépôt dans `rust-toolchain.toml:19`.
- **Workspace** : `Cargo.toml` racine (`members = ["crates/*"]`, resolver 2),
  champs de package hérités, et les lints centralisés dans
  `[workspace.lints]` (`Cargo.toml:66-87`).
- **Six crates squelettes** sous `crates/`, conformes au découpage de
  `docs/synthese/04-architecture.md` §5 : `dengon-core` (lib, bascule
  `no_std`), `dengon-ble` (lib), `dengon-node` (bin), `dengon-sim` (lib+bin),
  `dengon-verify` (bin), `dengon-ffi` (lib + cdylib). Chacune compile, expose
  au moins une constante et porte un test.
- **Graphe de dépendances câblé** : `ble→core`, `node→core+ble`, `sim→core`,
  `verify→core`, `ffi→core`. Chaque arête est *réellement utilisée* dans un
  test, sinon une dépendance déclarée mais inutilisée passerait inaperçue.
- **Style et lints** : `crates/rustfmt.toml` (options stables uniquement) et
  `crates/clippy.toml` (configuration des lints, pas leur activation).
- **CI** : `.github/workflows/core.yml`, un seul job nommé `core` : `fmt
  --check`, `build --locked`, `clippy -D warnings`, `check
  --no-default-features` (frontière `no_std`), `llvm-cov nextest`, doctests,
  rapport lcov en artefact et résumé de couverture dans le job.
- **`.gitignore`** : la ligne `Cargo.lock` est retirée, le lock est désormais
  versionné ; ajout de `lcov.info`.

### Pourquoi / décisions

- **Les lints vivent dans `[workspace.lints]`, pas dans `clippy.toml`.** C'est
  la subtilité du sujet : `clippy.toml` ne peut pas *activer* un lint, il ne
  fait que *configurer* ceux activés ailleurs. Les deux sont complémentaires —
  `[workspace.lints]` active `clippy::unwrap_used`, `clippy.toml` dit « sauf
  dans les tests ». Avantage sur un simple `-D warnings` en CI : les lints
  s'appliquent aussi au `cargo clippy` local, donc l'erreur se voit avant le push.
- **`unsafe_code = "deny"` et non `"forbid"`** : `forbid` est inviolable, et
  `dengon-ffi` devra le neutraliser puisque UniFFI génère de l'`unsafe`.
- **Toolchain épinglée à une version exacte, pas `"stable"`** : la CI lance
  `clippy -D warnings` ; une nouvelle stable tous les six semaines apporte de
  nouveaux lints et ferait virer `main` au rouge sans qu'une ligne ait changé.
- **Un vrai test dans chaque crate**, pas un squelette nu : `cargo nextest run`
  échoue sur un workspace sans aucun test, et `llvm-cov` produirait un rapport
  vide.
- **`Cargo.lock` versionné** : le workspace produit trois binaires, et les jobs
  `audit` / `cross-vectors` à venir exigent des builds reproductibles.
- **`rustfmt.toml` et `clippy.toml` rangés dans `crates/`, pas à la racine.**
  Les deux outils cherchent leur configuration en *remontant* l'arborescence —
  rustfmt depuis chaque fichier source, clippy depuis le manifeste de la crate
  — donc `crates/` suffit et `cargo fmt` / `cargo clippy` lancés depuis la
  racine les trouvent quand même. Vérifié dans les deux sens : avec un
  `max_width = 40` de test la signature est bien recoupée, et en retirant
  `clippy.toml` le `unwrap()` en test redevient une erreur. La racine du dépôt
  passe de cinq fichiers Rust à trois, ce qui compte : elle devra bientôt
  accueillir `android/`, `firmware/` et `dashboard/`.
- **En revanche `Cargo.toml`, `Cargo.lock` et `rust-toolchain.toml` restent à
  la racine.** Les deux premiers parce que `04-architecture.md` §5 le prévoit et
  que Cargo cherche son manifeste en remontant depuis le répertoire courant :
  déplacé, toute commande lancée depuis la racine échouerait. Le troisième à
  cause d'un piège **silencieux** : rustup résout `rust-toolchain.toml` depuis
  le répertoire courant et **pas** depuis `--manifest-path`. Testé — avec le
  fichier dans `crates/`, un `cargo build --manifest-path crates/Cargo.toml`
  lancé de la racine compile en ignorant la version épinglée, sans le moindre
  avertissement. Ça viderait de son sens la raison même de figer la toolchain.
- **Pas de seuil de couverture bloquant** : sur des crates squelettes le
  chiffre n'a aucun sens, et `--fail-under-lines` porte sur le rapport entier
  alors que l'objectif de 85 % ne vise que `dengon-core`.
- **Aucune dépendance externe déclarée** : leurs numéros de version n'étaient
  pas vérifiables ici, et une version erronée casse jusqu'à `cargo fmt`. Elles
  arriveront avec les US qui les utilisent (US-105 `btleplug`, US-106
  `uniffi`, US-108 la crypto).

### Écarts vs conception

Deux, tous deux consignés dans [`03-ecarts-conception.md`](03-ecarts-conception.md) :

1. Le filtrage par chemin du workflow est fait **dans le job**, pas via
   `on.pull_request.paths` comme le demandait littéralement l'issue #4.
2. `Cargo.lock` est versionné, ce que le `.gitignore` d'origine interdisait.

### Appris

Quatre notes ajoutées à [`04-apprentissages.md`](04-apprentissages.md) :
`[workspace.lints]` vs `clippy.toml`, le nom du job qui devient le nom du check
requis, le piège `paths:` sur les checks obligatoires, et la distinction
MSRV / toolchain épinglée.

### État après cette session

- Le dépôt **compile** et `cargo test` est vert (8 tests). C'est le premier code
  Rust du projet ; il ne fait encore rien d'utile, c'est voulu.
- Fiches module **créées** : les six, à l'état « esquisse » —
  [`modules/_index.md`](modules/_index.md).
- [`02-avancement.md`](02-avancement.md) : **mes lignes seulement** ont été
  modifiées (les six crates, plus une ligne d'outillage Rust et la ligne du
  workflow `core`), conformément à la règle posée par l'US-115. Rebasée après
  cette US, cette entrée suit donc la nouvelle organisation :
  `01-etat-du-code.md` n'est plus touché, il ne contient que des pointeurs.

### Vérification (commandes réellement exécutées)

```
$ rustc --version
rustc 1.98.1 (48a229cea 2026-09-01)

$ cargo fmt --all -- --check                                          → exit 0
$ cargo build --workspace --all-targets                               → exit 0
$ cargo clippy --workspace --all-targets --all-features --locked \
    -- -D warnings                                                    → exit 0
$ cargo check -p dengon-core --no-default-features --locked           → exit 0
$ cargo test --workspace --all-features --locked                      → exit 0
    8 tests passés, 0 échec (1+2+1+1+2+0+1 selon les cibles)
$ cargo test --workspace --all-features --locked --doc                → exit 0
```

Rejoué à l'identique après avoir déplacé `rustfmt.toml` et `clippy.toml` dans
`crates/` : les six commandes restent à `exit 0`.

Contrôle supplémentaire : pour vérifier que `[workspace.lints]` **mord**
réellement (le piège étant qu'une crate sans `[lints] workspace = true` est
ignorée en silence), un `unwrap()` a été ajouté temporairement hors test dans
`dengon-core` → clippy a bien échoué (`error: used 'unwrap()' on an 'Option'
value`), puis le fichier a été restauré et la vérification rejouée.

**Ce qui n'a PAS pu être vérifié :**

- `cargo nextest` et `cargo llvm-cov` : **non installés en local** (choix
  assumé pour ne pas alourdir le poste). Ces deux étapes du workflow ne sont
  donc validées par personne à ce stade — **seule la CI les exercera**.
- Le critère d'acceptation n°5 (« workflow vert sur une PR de test ») : rien
  n'a été commité ni poussé, à la demande de l'utilisateur. Le workflow n'a
  jamais tourné.
- Les tags des actions GitHub ont été vérifiés via l'API (`checkout@v7`,
  `upload-artifact@v7`, `paths-filter@v4`, `rust-cache@v2`,
  `install-action@v2`), mais aucune n'a été exécutée.

---

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
