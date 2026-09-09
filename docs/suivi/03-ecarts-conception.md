# Écarts entre le code et la conception

La conception ([`docs/powl/`](../powl/)) est une cible, pas un contrat. Quand
l'implémentation s'en écarte (contrainte technique, simplification, meilleure idée,
erreur de conception découverte), on le note **ici**, avec la raison, pour :

- ne pas se faire piéger par une doc de conception périmée ;
- pouvoir l'expliquer à l'oral (« on avait prévu X, en pratique Y parce que Z »).

Si un écart est structurant, mettre aussi à jour le document `docs/powl/` concerné
et le mentionner dans l'entrée de journal.

---

## Modèle d'entrée

### [date] — [titre court de l'écart]

- **Prévu :** ce que dit `docs/powl/NN-....md` (référence précise).
- **Réel :** ce qui est codé.
- **Raison :** pourquoi.
- **Conséquences :** impact sur le reste (autres modules, sécurité, perfs, planning).
- **Doc de conception mise à jour ?** oui / non (+ lien).

---

_(aucun écart pour l'instant)_

---

### 2026-09-09 — Protection de `main` : un seul check requis (`core`) au lieu de quatre

- **Prévu :** l'issue #13 (US-113) et
  [`docs/olivier/proposition-organisation-github.md`](../olivier/proposition-organisation-github.md)
  §4.3 demandent une protection de `main` avec **quatre checks requis** :
  `core`, `sim`, `audit` et `cross-vectors`. La DoD §7.1 point 3 les redit.
- **Réel :** les commandes livrées dans
  [`modules/processus-github.md`](modules/processus-github.md) ne déclarent que
  `"contexts": ["core"]`.
- **Raison :** `sim`, `audit` et `cross-vectors` **n'existent pas**. §4.3 les
  rattache à d'autres US, non commencées. Or un check requis qu'aucun workflow
  ne rapporte laisse la PR sur « Expected — Waiting for status to be reported »
  **indéfiniment** : GitHub attend un statut que personne n'enverra jamais. Les
  déclarer « pour être conforme à la DoD » gèlerait la totalité du dépôt, y
  compris les PR documentaires — c'est-à-dire la majorité de nos PR.
- **Conséquences :** l'intention de la DoD est préservée pour le seul job qui
  existe. Chaque nouveau workflow devra être ajouté à la liste **au moment où
  il est mergé**, en répétant la liste complète (l'API `PATCH` remplace le
  tableau `contexts`, elle ne l'enrichit pas). La commande d'élargissement est
  écrite dans la fiche. Condition de levée : à la fermeture de l'US qui livre
  chaque workflow.
- **Doc de conception mise à jour ?** non — la cible reste bien quatre checks ;
  seul le calendrier d'activation change. Le point est signalé dans le corps de
  la PR de l'US-113 et dans `modules/processus-github.md`.

---

### 2026-09-09 — Le label est `good first issue`, pas `good-first-issue`

- **Prévu :** §5.3 liste le label `good-first-issue`, avec des traits d'union.
- **Réel :** `.github/labels.yml` déclare `good first issue`, avec des espaces.
- **Raison :** c'est le label **par défaut de GitHub**, il existe déjà sur le
  dépôt et il est utilisé. Créer la variante à traits d'union donnerait deux
  labels au sens identique, dont un vide, et casserait la vue du board qui
  filtre sur le nom réel. Le fichier `labels.yml` a pour rôle de décrire
  l'existant : le corriger ici aurait fait mentir sa promesse de « zéro
  changement au premier sync ».
- **Conséquences :** aucune, sinon qu'il faut écrire le nom avec des espaces
  quand on filtre. La politique de déblocage de §10.3 point 5 (« prendre une
  `good-first-issue` d'une autre area ») s'applique inchangée.
- **Doc de conception mise à jour ?** non — coquille de nommage, sans effet sur
  l'organisation décrite.

---

### 2026-09-09 — Filtrage par chemin du workflow `core` : dans le job, pas dans le déclencheur

- **Prévu :** l'issue #4 (US-104) demande littéralement « le workflow est filtré
  par chemin (`paths: crates/**`) », c'est-à-dire un filtre au niveau de
  `on.pull_request.paths`. Même formulation dans
  [`docs/olivier/proposition-organisation-github.md`](../olivier/proposition-organisation-github.md) §4.3.
- **Réel :** `.github/workflows/core.yml` n'a **aucun** `paths:` sur ses
  déclencheurs. Le filtrage se fait à la première étape du job, via
  `dorny/paths-filter`, et chaque étape lourde porte un
  `if: steps.filtre.outputs.rust == 'true'`.
- **Raison :** avec un `paths:` sur le déclencheur, GitHub **ne démarre pas du
  tout** le workflow quand aucun fichier ne correspond. Aucun *check run* nommé
  `core` n'est alors créé. Or la protection de `main` (US-113) exigera un check
  `core` : toute PR ne touchant que `docs/` resterait bloquée indéfiniment sur
  « Expected — Waiting for status to be reported », même approuvée. Et sur ce
  projet, **la majorité des PR ne touchent que `docs/`** (trois dossiers de
  conception, plus la règle de mise à jour du suivi imposée par `CLAUDE.md`).
  La parade « officielle » — un workflow jumeau en `paths-ignore` avec un job
  homonyme — est pire : `paths` et `paths-ignore` ne sont pas complémentaires,
  une PR touchant `docs/` **et** `crates/` déclencherait les deux et créerait
  deux checks `core` concurrents.
- **Conséquences :** l'intention du critère est préservée — aucune commande
  `cargo` ne tourne sur une PR qui ne touche pas Rust. Le coût est d'environ
  15 s de runner sur ces PR, contre 0 s avec un filtre au déclencheur. En
  échange, le check `core` est **toujours** rapporté et peut être rendu
  obligatoire sans piéger l'équipe. **Le même patron devra être appliqué à
  `sim`, `audit` et `cross-vectors`** quand ils seront écrits.
- **Doc de conception mise à jour ?** non — le point est trop fin pour
  `docs/synthese/`. Il est signalé dans le corps de la PR de l'US-104 et en
  commentaire en tête de `.github/workflows/core.yml`.

---

### 2026-09-09 — `Cargo.lock` versionné

- **Prévu :** le `.gitignore` d'origine (ligne 186, commit `c0e2a9b`, écrit
  quand la stack n'était pas encore choisie) ignorait `Cargo.lock`.
- **Réel :** la ligne a été retirée ; `Cargo.lock` est versionné.
- **Raison :** la recommandation Rust est de committer le lock dès qu'un dépôt
  produit un exécutable — ici il y en a trois (`dengon-node`, `dengon-sim`,
  `dengon-verify`). Trois raisons propres au projet s'ajoutent : (a) la CI lance
  `clippy -D warnings`, donc une version patch d'une dépendance transitive
  publiée n'importe quel jour pourrait faire virer `main` au rouge sans qu'une
  ligne du dépôt ait changé ; (b) `cargo audit` et `cargo deny` du futur job
  `audit` lisent le lock — sans lui, un rapport de vulnérabilité ne correspond
  à ce que personne n'a construit ; (c) le job `cross-vectors` suppose une
  reproductibilité entre core, firmware et dashboard.
- **Conséquences :** la CI passe `--locked` partout, ce qui devient un
  garde-fou : une PR qui modifie `Cargo.toml` sans régénérer le lock échoue
  immédiatement avec un message clair. Contrepartie : les montées de version de
  dépendances deviennent des diffs explicites à relire.
- **Doc de conception mise à jour ?** sans objet (le `.gitignore` n'est pas un
  document de conception) ; la raison est écrite en commentaire dans le fichier
  lui-même.

---

### Pièges de `.gitignore` repérés mais **non corrigés** (dette assumée)

Repérés en passant, laissés en l'état parce qu'ils ne gênent pas encore et que
les corriger à l'aveugle serait hors périmètre de l'US-104 :

- Ligne `bin/` (section .NET, non ancrée) → ignorerait `crates/*/src/bin/` le
  jour où une crate aura des binaires secondaires.
- Lignes `*.pem` et `*.key` → ignoreront **silencieusement** les fixtures de
  clés de test de `crypto` et `ledger` (P1.5, P1.7). À neutraliser par un
  `!crates/**/tests/fixtures/**` à ce moment-là. C'est le plus dangereux des
  trois : l'oubli se manifesterait par des tests qui passent en local et
  échouent en CI.
