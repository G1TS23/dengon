# Apprentissages

Volet « apprentissage » du suivi. On y note les **notions qu'il a fallu comprendre**
pour coder le projet : algorithmes, protocoles, API, pièges, subtilités de langage.

But : pouvoir **réexpliquer** ces notions à l'oral, et éviter de réapprendre deux
fois la même chose.

Format libre mais court. Une note = un concept. Toujours répondre à : *c'est quoi ?*,
*pourquoi on en a besoin ici ?*, *qu'est-ce qui nous a surpris ?*.

---

## Modèle

### [Titre du concept]

**C'est quoi :** définition en 2-3 phrases, avec ses mots.
**Pourquoi dans dengon :** à quoi ça sert concrètement dans notre code.
**Piège / surprise :** ce qui n'était pas évident, l'erreur qu'on a faite.
**Où c'est utilisé :** `chemin:ligne`.
**Pour aller plus loin :** lien(s).

---

## Notes

### `merge=union` — fusionner des fichiers « append » sans conflit

**C'est quoi :** un pilote de fusion **intégré à git**. Sur un fichier marqué
`merge=union` dans `.gitattributes`, quand deux branches modifient la même zone,
git **prend les deux versions** au lieu d'écrire des marqueurs `<<<<<<<`.
**Pourquoi dans dengon :** `docs/suivi/00-journal.md` & co. reçoivent une entrée
par PR, toujours au même endroit → conflit systématique (US-115).
**Piège / surprise :** `union` **ne trie pas**. Il concatène les deux côtés dans
un ordre non garanti, sans forcément remettre de ligne vide. Après la fusion, on
relit le haut du journal et on remet l'ordre anti-chronologique si besoin. À ne
**pas** mettre sur un fichier qu'on *réécrit* (il dupliquerait des paragraphes) —
d'où `01-etat-du-code.md` laissé hors `union` et vidé de son contenu volatil.
**Où c'est utilisé :** `.gitattributes` racine ; règle expliquée dans
`docs/suivi/README.md` § Fusion.
**Pour aller plus loin :** `man gitattributes` (section « Merging branches with
differing checkin/checkout attributes »), `git help merge`.

### Formulaire d'issue (*issue form*) vs template Markdown

**C'est quoi :** deux façons de pré-remplir une issue GitHub. Le template
Markdown est un fichier `.md` recopié dans la zone de texte. Le formulaire est
un fichier `.yml` décrivant des champs typés (`input`, `textarea`, `dropdown`,
`checkboxes`) que GitHub transforme en vrai formulaire web.
**Pourquoi dans dengon :** la Definition of Ready (§6) exige huit informations.
Avec un template Markdown, on peut tout effacer et soumettre une issue vide en
une seconde — la DoR reste une politesse. Avec un formulaire, un champ
`validations: required: true` **empêche la soumission**. C'est la seule
différence, et c'est toute l'US-113.
**Piège / surprise :** deux choses. D'abord `required` ne se met pas à la racine
de l'élément mais sous `validations:` — placé ailleurs, il est ignoré en
silence, sans erreur. Ensuite, **GitHub ne lit `ISSUE_TEMPLATE/` que sur la
branche par défaut** : sur une branche de travail, aucune prévisualisation n'est
possible, il faut merger pour voir le rendu.
**Où c'est utilisé :** `.github/ISSUE_TEMPLATE/user-story.yml`, `spike.yml`,
`bug.yml`.

### `CODEOWNERS` : dernière ligne gagnante, et pas d'auto-approbation

**C'est quoi :** un fichier qui associe des motifs de chemins à des comptes
GitHub. Quand une PR touche un chemin, ses propriétaires sont automatiquement
sollicités pour la revue.
**Pourquoi dans dengon :** c'est le mécanisme qui rend la DoD §7.1 point 4 —
« revue par une personne d'une autre `area:` » — obligatoire au lieu
d'optionnelle.
**Piège / surprise :** trois.
1. La syntaxe des motifs ressemble à `.gitignore`, et comme lui **la dernière
   ligne qui correspond l'emporte**. Une règle générique `*` écrite en bas du
   fichier écrase donc silencieusement toutes les règles précises. Elle se met
   en tête.
2. **L'auteur d'une PR ne peut jamais s'auto-approuver au titre de
   `CODEOWNERS`.** C'est ce qui permet de mettre deux noms par chemin sans
   affaiblir la règle : le second est toujours quelqu'un d'autre que l'auteur.
3. Un compte **sans droit d'écriture** sur le dépôt est ignoré, sans message.
   GitHub expose un endpoint pour le détecter :
   `gh api repos/OWNER/REPO/codeowners/errors?ref=BRANCHE`.
**Où c'est utilisé :** `.github/CODEOWNERS`.

### Un 404 d'API GitHub peut vouloir dire « tu n'as pas le droit de savoir »

**C'est quoi :** `GET /repos/{owner}/{repo}/branches/{branch}/protection` renvoie
**404** à un compte qui n'est pas admin du dépôt — que la protection existe ou
non. Pas 403 : 404, exactement comme si l'objet n'existait pas. GitHub masque
l'existence de la ressource plutôt que d'en révéler la présence.
**Pourquoi dans dengon :** on est authentifié en `POWLAIR`, non admin. Le 404 a
été lu comme « aucune protection » et écrit tel quel plusieurs fois dans le
suivi et dans une PR. Le fait était vrai au moment où on l'écrivait, mais la
**preuve ne prouvait rien** : la même commande renvoie toujours 404 après
l'activation de la protection, le 09/09 à 15:18. Le genre d'erreur qui survit à
la relecture, puisque la conclusion était juste — seul le raisonnement était
creux.
**Piège / surprise :** `gh api repos/OWNER/REPO/rulesets` renvoie `[]` de la même
façon, et `rules/branches/main` ne liste que les *rulesets*, jamais la
protection classique. Aucun des trois ne permet à un non-admin de conclure.
**À utiliser à la place :** l'effet observable sur une PR —
`gh pr view <n> --json mergeStateStatus,statusCheckRollup`. `CLEAN` = rien ne
bloque ; `BLOCKED` = une règle s'applique, et le rollup dit laquelle.
**Règle générale :** ne pas déduire une absence d'un code d'erreur sans savoir
ce que ce code signifie pour le niveau de droits dont on dispose. Vérifier un
**effet**, pas une **permission**.

### Un check requis inexistant fige un dépôt

**C'est quoi :** dans la protection de branche, `required_status_checks.contexts`
liste des **noms de jobs** qui doivent être verts pour merger.
**Pourquoi dans dengon :** la DoD nomme quatre checks (`core`, `sim`, `audit`,
`cross-vectors`) ; trois n'ont pas encore de workflow.
**Piège / surprise :** GitHub n'exige pas que le check existe. Il l'**attend**.
Une PR reste alors sur « Expected — Waiting for status to be reported », sans
message d'erreur, sans échec — juste un bouton de merge grisé pour toujours.
Corollaire déjà rencontré à l'US-104 : un workflow filtré par
`on.pull_request.paths` **ne démarre pas** sur une PR hors périmètre, donc ne
rapporte aucun check, donc produit exactement le même blocage. D'où le filtrage
fait *dans* le job, pas dans le déclencheur (voir `03-ecarts-conception.md`).
Deuxième subtilité : le `PATCH` de `required_status_checks` **remplace** le
tableau `contexts`, il n'y ajoute pas — il faut toujours réécrire la liste
complète.
**Où c'est utilisé :** `docs/suivi/modules/processus-github.md`, commandes de
protection de `main`.

### `workflow_dispatch` n'existe que sur la branche par défaut

**C'est quoi :** le déclencheur qui met un bouton « Run workflow » dans
l'onglet Actions.
**Pourquoi dans dengon :** la synchro des labels est volontairement manuelle —
elle peut supprimer des labels posés sur 55 issues, on ne veut pas qu'un push
la lance.
**Piège / surprise :** GitHub ne propose le bouton que si le fichier est déjà
présent sur la **branche par défaut**. Un workflow `workflow_dispatch` créé dans
une PR est donc **intestable avant merge** — il n'apparaît nulle part. Il faut
l'écrire avec soin, et prévoir un mode simulation par défaut plutôt que de
compter sur un essai préalable.
**Où c'est utilisé :** `.github/workflows/labels.yml`.

---

### `[workspace.lints]` et `clippy.toml` ne font pas la même chose

**C'est quoi :** deux fichiers qui ont l'air de configurer clippy, mais qui ont
des rôles disjoints. `[workspace.lints]` (dans `Cargo.toml`, depuis Cargo 1.74)
**active ou désactive** des lints. `clippy.toml` **configure** ceux qui sont
déjà activés — seuils, exceptions — et ne peut en activer aucun.

**Pourquoi dans dengon :** l'US-104 demandait « une configuration clippy
versionnée ». On aurait pu croire que `clippy.toml` suffisait. En réalité les
deux sont nécessaires et complémentaires : `[workspace.lints.clippy]` active
`unwrap_used`, et `clippy.toml` ajoute `allow-unwrap-in-tests = true` — sans
quoi chaque test croulerait sous les `#[allow]`.

**Piège / surprise :** trois pièges, dont un vicieux.
1. Les lints du workspace sont **ignorés en silence** dans toute crate qui
   n'écrit pas `[lints] workspace = true` dans son propre `Cargo.toml`. Aucune
   erreur, aucun avertissement — la crate n'est simplement pas vérifiée. C'est
   pour ça qu'on a testé la chaîne en ajoutant un `unwrap()` volontaire.
2. Une clé inconnue dans `clippy.toml` fait **échouer** clippy, alors qu'une
   option inconnue dans `rustfmt.toml` est seulement ignorée.
3. `priority = -1` est obligatoire sur un groupe (`all`, `rust_2018_idioms`),
   sinon un lint individuel du même groupe ne peut pas le surcharger.

**Où c'est utilisé :** `Cargo.toml:66-87`, `crates/clippy.toml`, et
`[lints] workspace = true` dans les six `crates/*/Cargo.toml`.
**Pour aller plus loin :** `cargo help lints`, et la liste des options
configurables sur rust-lang.github.io/rust-clippy/master/index.html.

---

---

### En CI, le nom du *job* devient le nom du *check* requis

**C'est quoi :** quand GitHub publie le résultat d'un workflow, l'identifiant
visible dans la protection de branche est le nom du **job**, pas celui du
fichier ni du workflow.

**Pourquoi dans dengon :** la conception exige que `core`, `sim`, `audit` et
`cross-vectors` soient verts pour merger. Il faut donc que le job s'appelle
exactement `core`.

**Piège / surprise :** deux façons de casser ça sans s'en apercevoir. Mettre une
`strategy.matrix` sur le job : le check devient `core (ubuntu-latest)` et la
protection ne le trouve plus. Passer par un workflow réutilisable : il devient
`appelant / appelé`. Découper en quatre jobs (fmt, clippy, test, couverture)
publierait quatre checks et **aucun** nommé `core`.

**Où c'est utilisé :** `.github/workflows/core.yml`, un seul job `core`.

---

---

### MSRV et toolchain épinglée : deux choses différentes

**C'est quoi :** `rust-version` dans `Cargo.toml` est la **MSRV** — la version
minimale de Rust acceptée, un *plancher*. `channel` dans `rust-toolchain.toml`
est la version **exacte** que rustup installe et que la CI utilise.
Invariant : `channel >= rust-version`.

**Pourquoi dans dengon :** la conception dit « MSRV figée » sans donner de
numéro. On a mis les deux : `rust-version = "1.85"` (plancher confortable) et
`channel = "1.98.1"` (ce que tout le monde utilise réellement).

**Piège / surprise :** épingler `channel = "stable"` serait une fausse bonne
idée. Une nouvelle stable sort toutes les six semaines avec de nouveaux lints ;
comme la CI lance `clippy -D warnings`, `main` peut virer au rouge un matin sans
qu'une ligne du dépôt ait changé. Avec une version exacte, la montée de version
devient une PR volontaire. Autre subtilité : rustup lit `rust-toolchain.toml` et
installe tout seul la bonne version au premier `cargo` — y compris les
composants listés (`llvm-tools`, indispensable à `cargo llvm-cov`).

**Où c'est utilisé :** `Cargo.toml` (`rust-version`), `rust-toolchain.toml:19`.
**À noter :** aucun job CI ne vérifie encore que la MSRV annoncée est tenue.

---

---

### Où chaque fichier de configuration Rust doit vivre

**C'est quoi :** les cinq fichiers de configuration Rust n'ont pas du tout les
mêmes règles de localisation, alors qu'on a tendance à tous les jeter à la racine.

| Fichier | Trouvé comment | Peut descendre dans un dossier ? |
|---|---|---|
| `Cargo.toml` | remontée depuis le **répertoire courant** | oui, mais toute commande depuis la racine échoue |
| `Cargo.lock` | toujours à côté de `Cargo.toml` | non, pas configurable |
| `rustfmt.toml` | remontée depuis **chaque fichier source** | oui, sans coût |
| `clippy.toml` | remontée depuis le **manifeste de la crate** | oui, sans coût |
| `rust-toolchain.toml` | remontée depuis le **répertoire courant**, par rustup | déconseillé (voir ci-dessous) |

**Pourquoi dans dengon :** le dépôt est polyglotte — `android/`, `firmware/` et
`dashboard/` vont arriver, et chacun gardera sa configuration chez lui. Laisser
cinq fichiers Rust à la racine était asymétrique. On en a descendu deux dans
`crates/`.

**Piège / surprise :** `rust-toolchain.toml` est résolu par rustup depuis le
**répertoire courant**, pas depuis `--manifest-path`. Si on le range dans
`crates/`, alors `cargo build --manifest-path crates/Cargo.toml` lancé de la
racine compile **en ignorant silencieusement la version épinglée**. Aucun
avertissement. C'est exactement le contraire de ce qu'on cherche en figeant une
toolchain, donc ce fichier reste à la racine.

**Où c'est utilisé :** `crates/rustfmt.toml`, `crates/clippy.toml`, et à la
racine `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`.

---

### Épingler les actions GitHub sur un SHA, pas sur un tag

**C'est quoi :** dans un workflow, `uses: dorny/paths-filter@v4` désigne un tag
Git — donc une étiquette **mutable**. Son propriétaire peut la repointer vers
n'importe quel commit. `@master` est pire encore : c'est une branche, qui bouge
par construction. Un SHA de commit complet, lui, est immuable.

**Pourquoi dans dengon :** SonarCloud (règle `githubactions:S7637`) a fait
échouer la Quality Gate de la PR #57 sur exactement ce point, avec une note de
sécurité « C » sur le nouveau code. C'était justifié : une action tierce
s'exécute dans notre CI avec accès au dépôt.

**Piège / surprise :** on croit qu'épingler `@v4` suffit parce que ça ressemble
à une version figée. Ce n'est pas le cas — seul un SHA l'est. À noter que Sonar
n'a pas signalé `actions/checkout` ni `actions/upload-artifact` : les actions
officielles `actions/*` sont considérées de confiance. La contrepartie de
l'épinglage est qu'il faut mettre à jour à la main ; on garde donc le numéro de
version en commentaire à droite du SHA, pour rester lisible.

**Où c'est utilisé :** `.github/workflows/core.yml`, les six `uses:`.
**Pour aller plus loin :** règle S7637 sur rules.sonarsource.com, et la
documentation GitHub « Using third-party actions ».

---

### `jsonschema` (Python) : `referencing.Registry` remplace `RefResolver`

**C'est quoi :** depuis `jsonschema` 4.18, la résolution des `$ref` inter-
fichiers passe par le paquet `referencing` : un `Registry` qu'on peuple à la
main (`Resource.from_contents(...)`), pas par le `RefResolver` intégré des
versions antérieures (déprécié).
**Pourquoi dans dengon :** `contracts/events/batch.schema.json` référence
`envelope.schema.json` (`$ref: "envelope.schema.json"`, et depuis la revue de
la PR #60, `$ref: "envelope.schema.json#/$defs/node_id"`) — sans registre
peuplé, `Draft202012Validator` ne sait pas résoudre ce chemin relatif.
**Piège / surprise :** une ressource doit être enregistrée **sous son `$id`
et sous son nom de fichier** si les deux formes de `$ref` doivent marcher
(un `$ref` par nom de fichier relatif, un autre potentiel par URI absolue) —
l'oublier fait échouer la résolution silencieusement selon la forme du `$ref`
utilisée.
**Où c'est utilisé :** `contracts/tools/validate.py::_batch_registry()`.
**Pour aller plus loin :** doc du paquet `referencing` (`python-jsonschema.readthedocs.io`).

### Longueur d'une signature Ed25519 en base64

**C'est quoi :** une signature Ed25519 fait **64 octets** fixes. En base64
standard (avec padding), ça donne toujours **88 caractères**, dont les 2
derniers sont le padding `==` (64 octets = 512 bits, non multiple de 3 ×
8 = 24 bits, d'où le padding).
**Pourquoi dans dengon :** `contracts/events/batch.schema.json` contraint
`sig` par un motif de longueur fixe : `^[A-Za-z0-9+/]{86}==$` (86 caractères
utiles + le `==`), plutôt qu'un motif générique de longueur variable — une
signature d'une autre taille (mauvais algorithme, troncature accidentelle)
est rejetée par le schéma lui-même, sans avoir besoin de la décoder.
**Où c'est utilisé :** `contracts/events/batch.schema.json` (champ `sig`).

---

Sujets probables (d'après la conception) — à traiter quand on les rencontre :

- Routage épidémique / gossip / store-carry-forward (DTN).
- TTL, déduplication, jitter de relais : pourquoi chacun est nécessaire.
- Noise Protocol Framework : motifs `XX` vs `X`, ce que « forward secrecy » veut dire.
- Ed25519 vs X25519 : signature vs accord de clés.
- Hash-chain (journal chaîné) : en quoi ça rend une trace « infalsifiable », et ses
  limites (n'empêche pas d'omettre avant de signer).
- Golomb-Coded Set / filtre de Bloom : résumer un ensemble de façon compacte.
- BLE GATT : rôle central vs périphérique, MTU, notify vs write, pourquoi un nœud
  doit être les deux.
- ESP-IDF / FreeRTOS : tâches, files, coexistence BLE + Wi-Fi.
- UniFFI : comment un cœur Rust est appelé depuis Kotlin.
- TimescaleDB : hypertable, rétention, agrégats continus.
- MQTT : QoS, topics, mTLS.
