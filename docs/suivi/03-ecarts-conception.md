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

### 2026-09-10 — `.gitignore` : les fixtures de clés de test sont ré-incluses

- **Prévu :** rien. Le `.gitignore` générique (commit `c0e2a9b`, écrit quand la
  stack n'était pas choisie) ignore `*.pem`, `*.key`, `*.p12` et `*.pfx` pour
  éviter qu'un vrai secret soit commité.
- **Réel :** quatre négations limitées aux répertoires `tests/` des crates
  (`!crates/**/tests/**/*.pem` et les trois autres).
- **Raison :** signalé en **revue de la PR #57** par `G1TS23`. Une clé
  d'exemple servant de fixture n'est pas un secret, mais les motifs génériques
  l'avalaient **en silence** : le fichier n'était jamais ajouté, sans erreur ni
  avertissement. Le symptôme ne serait apparu que plus tard et ailleurs — tests
  verts en local, rouges en CI — au moment de l'**US-108 (crypto)**, qui est sur
  le chemin critique du sprint 2. Corriger ici coûtait quatre lignes ; découvrir
  le problème pendant une US de gate aurait coûté une demi-journée.
- **Conséquences :** la portée est volontairement étroite — uniquement sous
  `tests/`, uniquement dans `crates/`. Vérifié dans les deux sens :
  `crates/dengon-core/tests/fixtures/alice.key` est désormais ajoutable
  (`git add --dry-run` → `add '...'`), tandis que `crates/dengon-core/prod.pem`,
  `dashboard/api/prod.pem` et `secret.key` restent refusés. GitGuardian, actif
  sur chaque PR, sert de second filet. Les autres areas (`dashboard/`,
  `contracts/`) devront faire le même geste quand elles y toucheront.
- **Doc de conception mise à jour ?** sans objet ; la raison est en commentaire
  dans le `.gitignore` lui-même.

---

### Piège de `.gitignore` repéré mais **non corrigé** (dette assumée)

- Ligne `bin/` (section .NET, non ancrée) → ignorerait `crates/*/src/bin/` le
  jour où une crate aura des binaires secondaires. Laissé en l'état : aucune
  crate n'a de binaire secondaire, et le mode d'échec est bruyant (le fichier
  manque, la compilation échoue) — contrairement à celui des clés, qui était
  silencieux.

---

### `TransportEvent::PeerDisconnected` porte un **motif**, absent de la conception

- **Conception :** [`docs/synthese/04-architecture.md`](../synthese/04-architecture.md)
  §3 décrit `PeerDisconnected { peer_link_id }` — le lien, rien d'autre.
- **Code :** `crates/dengon-ble/src/transport.rs:181` ajoute un champ
  `reason: DisconnectReason`, à trois valeurs : `Propre`, `Brutale`, `Locale`.
- **Pourquoi :** le critère d'acceptation n°4 de l'US-105 demande que « le
  comportement attendu en déconnexion brutale soit spécifié ». On peut le
  spécifier en rustdoc — c'est fait — mais la couche du dessus doit aussi
  pouvoir **distinguer les cas à l'exécution** : une coupure brutale justifie de
  retenter tout de suite (le pair est peut-être encore à portée), un départ
  propre non. Sans ce champ, `sync::routing` (US-209) ne peut que deviner, et
  devinerait mal dans le cas le plus fréquent — un réseau maillé mobile passe
  son temps à perdre des liens brutalement.
- **Conséquences :** c'est un **élargissement du contrat gelé**, à assumer comme
  tel : les quatre implémentations (`btleplug`, Android, NimBLE, bouchon)
  devront remplir ce champ correctement, et la suite de conformité les y oblige
  (`cas_deconnexion_brutale`, `cas_deconnexion_propre_est_distinguee`). Sur
  matériel réel, produire une `Brutale` veut dire couper l'alimentation de la
  carte d'en face, pas appeler `disconnect` — c'est écrit dans la doc du trait
  `BancDEssai`. Le coût est réel mais borné ; le bénéfice est qu'un
  comportement central du maillage cesse d'être implicite.
- **Doc de conception mise à jour ?** **Non.** `04-architecture.md` §3 garde sa
  version. À trancher au point d'équipe qui gèle le contrat : soit on met la
  conception à jour, soit on retire le champ. Le laisser diverger en silence
  serait le pire des trois.

---

### `TransportConfig` et `LinkId` sont inventés ici, sans référence de conception

- **Conception :** `04-architecture.md` §3 **les nomme** dans la signature du
  trait (`fn start(&mut self, cfg: TransportConfig)`, `peer_link_id: LinkId`)
  mais ne les définit nulle part. Aucun autre document ne les décrit.
- **Code :** `crates/dengon-ble/src/transport.rs:44` (`LinkId`) et `:78`
  (`TransportConfig`, 5 champs).
- **Pourquoi :** il fallait bien choisir. Les partis pris, justifiés en rustdoc
  et dans [`modules/dengon-ble.md`](modules/dengon-ble.md) : `LinkId` identifie
  une **connexion** et non un nœud (le `peerID` d'A-8 reste au protocole) ;
  `TransportConfig` ne porte que ce qui **varie d'un nœud à l'autre**, pas les
  constantes de protocole, qui arrivent avec `protocol::consts` (US-108).
- **Conséquences :** ce découplage est ce qui permet à US-105 et US-108
  d'avancer **en parallèle** dans le même sprint, sans dépendance croisée.
  Contrepartie : `TransportConfig::local_peer_id` est typé `[u8; 8]` et non
  `PeerId` ; le remplacer quand US-108 aura livré ce type est un changement de
  signature, pas de sémantique.
- **Doc de conception mise à jour ?** Non — ce n'est pas une divergence mais un
  **comblement**. À verser dans `04-architecture.md` §3 si l'équipe veut que la
  conception reste la référence complète du contrat.
