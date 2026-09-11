# Journal de développement

Journal **append-only**. Entrée la plus récente en haut. Une entrée par session de
travail sur le code. Modèle : [`templates/entree-journal.md`](templates/entree-journal.md).

> On ne modifie jamais une entrée passée. Pour corriger une info, on ajoute une
> nouvelle entrée qui rectifie.

---

<!-- NOUVELLES ENTRÉES ICI (juste en dessous de cette ligne) -->

## 2026-09-11 — `contracts/events` : relecture approfondie d'OswinFreyr sur la PR #60 (round 2)

**Auteur :** Claude (Sonnet 5)
**Périmètre :** `contracts/tools/{validate,catalogue}.py`,
`contracts/events/{envelope,batch,payloads}.schema.json`,
`docs/suivi/03-ecarts-conception.md`, `docs/suivi/04-apprentissages.md`
**Lot :** US-107 (suite), Sprint 1

### Fait
- @OswinFreyr a retesté la branche dans un `git worktree` séparé (Python 3.13,
  deps installées sans `uv`) après le round 1 : les 5 premiers points sont
  confirmés corrigés (`validate.py`/`build_fixtures.py` rejoués, régénération
  byte-identique). En relisant plus en profondeur, il a trouvé 4 points
  supplémentaires, tous vérifiés dans le code avant correction :
  1. **`seq` en overflow (`>= 2**64`)** : `_valid_seq()` du round 1 ne
     vérifiait qu'un plancher (`>= 0`), pas de plafond ;
     `seq.to_bytes(8, "big")` lève `OverflowError` au-delà de `2**64`.
     Reproduit (`event_id("x", 2**64)` → `OverflowError: int too big to
     convert`), corrigé : `_valid_seq` vérifie aussi `< 2**64`.
  2. **`node_id: null`** : `event.get("node_id", "")` ne couvre que la clé
     **absente**, pas une clé présente à `null` — `event_id(None, seq)` lève
     `AttributeError`. Reproduit, corrigé : `node_id` doit être une `str`
     avant tout calcul, sinon `errors`.
  3. **`payload` non-objet** (ex. une liste) : `_check_event` faisait
     `payload.items()` sans vérifier le type. Reproduit
     (`AttributeError: 'list' object has no attribute 'items'`), corrigé :
     type vérifié avant toute manipulation.
  4. **Motifs hex/sig ancrés avec `$`, qui matche avant un `\n` final en
     Python** (`re`, pas `re.MULTILINE`) : `"<16 hex>\n"` (17 caractères)
     passait `HEX16`. Reproduit en isolant le regex, puis en mutant une
     fixture de travail (jamais committée). **Pas corrigé avec `\Z`** (la
     suggestion du round 1) : `\Z` est une extension Python absente d'ECMA
     262, la norme visée par `pattern` en JSON Schema — l'introduire dans
     des schémas censés rester neutres en langage serait un contre-sens.
     Corrigé avec `minLength`/`maxLength` à côté de `pattern` (mot-clé JSON
     Schema standard) sur les champs de longueur **fixe** seulement
     (`HEX16`/`32`/`64`, `event_id`, `batch_id`, `sig`). Les motifs
     **ouverts** (`node_id`, `name`) restent vulnérables — dette assumée,
     documentée dans `03-ecarts-conception.md`, impact jugé faible (aucun
     calcul ne plante dessus, contrairement aux 3 points précédents).
  - Deux nits non bloquants également corrigés : boucle manuelle sur
    `spec["required"]` remplacée par le `required` natif du schéma ; chaque
    fixture n'est plus lue/parsée qu'une fois par `main()` (avant : 3 fois).
- Rebuild complet : `payloads.schema.json` régénéré (`build_fixtures.py`)
  après les changements de `catalogue.py` — diff limité au fichier généré,
  les 20 fixtures restent byte-identiques (régénération déterministe
  confirmée une nouvelle fois).

### Pourquoi / décisions
- **`minLength`/`maxLength` plutôt que `\Z`** : décision structurante de
  cette entrée. `\Z` aurait été la correction la plus rapide (celle
  suggérée), mais elle aurait fait fuiter une dépendance Python dans un
  artefact dont toute la raison d'être est d'être consommable par n'importe
  quel langage. `minLength`/`maxLength` obtient le même résultat sans ce
  compromis, au prix de ne fonctionner que sur des champs de longueur fixe.
- **`node_id`/`name` non corrigés pareil, assumé plutôt que forcé** : pas de
  borne haute naturelle pour ces deux motifs, et l'impact réel est nul
  (aucun crash, juste une strictness manquante) — mieux vaut le documenter
  explicitly que d'introduire une extension Python pour fermer un trou à
  faible risque.

### Écarts vs conception
- Un nouveau, dans `03-ecarts-conception.md` : le piège `$`/`\n` sur
  `node_id`/`name` non corrigé (dette assumée).

### Appris
- `$` en regex Python matche avant un `\n` final (piège pour un `pattern`
  JSON Schema censé suivre ECMA 262) ; `required` de JSON Schema remplace une
  vérification manuelle. Les deux ajoutés à `04-apprentissages.md`.

### État après cette session
- Les 4 nouveaux points + 2 nits de la relecture d'@OswinFreyr sont traités.
- Fiche(s) module mise(s) à jour : [modules/contracts-events.md](modules/contracts-events.md)
- 01-etat-du-code.md mis à jour : non.

### Vérification (commandes réellement exécutées)
```
$ cd contracts && uv run python3 tools/build_fixtures.py
20 fixtures écrites, 28 noms d'événements couverts.
$ git status --short events/fixtures/        # vide : régénération byte-identique

$ uv run python3 tools/validate.py
✓ 20 fixtures valides — 28 noms d'événements couverts.

$ uv run ruff check . && uv run ruff format --check tools/catalogue.py tools/validate.py
All checks passed!

# Reproduction des 3 crashes, un par un, sur une fixture mutée (jamais
# committée), restaurée après chaque essai :
seq = 2**64        → rapport propre (« seq invalide »), plus de traceback
node_id = null     → rapport propre (« node_id invalide »), plus de traceback
payload = [...]    → rapport propre (« payload invalide (list) »), plus de traceback
msg_log_id + "\n"  → rapport propre (« is too long »), passait avant le fix
```

---



## 2026-09-11 — `contracts/events` : retours de revue d'OswinFreyr sur la PR #60

**Auteur :** Claude (Sonnet 5)
**Périmètre :** `contracts/tools/{validate,catalogue}.py`, `contracts/events/batch.schema.json`,
`docs/suivi/03-ecarts-conception.md`, `docs/suivi/04-apprentissages.md`,
`docs/synthese/09-dashboard-et-donnees.md`
**Lot :** US-107 (suite), Sprint 1

### Fait
- 5 points relevés en revue par @OswinFreyr, tous vérifiés dans le code avant
  correction :
  1. **`validate.py:118` — crash non géré sur `seq` invalide.**
     `event_id(node_id, event.get("seq", -1))` appelle `seq.to_bytes(8,
     "big")` : `OverflowError` si négatif, `AttributeError` si pas un entier.
     `_check_schema` avait déjà signalé le problème dans `errors`, mais le
     script continuait quand même et plantait avec une traceback brute au
     lieu du rapport attendu. **Reproduit** en mettant `seq = -1` dans une
     fixture (copie de travail, jamais committée) : confirmé le crash exact
     décrit, puis confirmé le rapport propre une fois corrigé. `seq` validé
     (entier, pas un bool, ≥ 0) avant tout calcul d'`event_id`.
  2. **`catalogue.py` — `pkt.seen.rssi` optionnel alors que `powl/08` et
     `synthese/09` le listent sans `?`.** Vérifié : c'est la doc de
     conception qui est en retard, pas le contrat — `TransportEvent::
     PeerConnected.rssi` (US-105) est déjà `Option<i16>` pour la même
     raison (RSSI pas toujours fourni côté transport). Écart consigné,
     `synthese/09` corrigé (`rssi?`).
  3. **Apprentissages non propagés** — l'entrée de journal US-107 mentionnait
     deux notions réelles (`referencing.Registry`, longueur de signature
     Ed25519 en base64) sans les ajouter à `04-apprentissages.md` (règle 5,
     `CLAUDE.md`). Ajoutées.
  4. **Motif `node_id` dupliqué** dans `batch.schema.json`,
     `envelope.schema.json#/$defs/node_id` et `catalogue.py`. Le premier
     référence maintenant le second via `$ref` (draft 2020-12 autorise `$ref`
     à côté d'autres mots-clés comme `description`). Le doublon Python
     (`subject_node`) reste — pas de `$ref` possible entre un module Python
     et un fichier JSON Schema — mais nommé (`NODE_ID_PATTERN`) plutôt que
     recopié.
  5. **Perf, non bloquant** — `_check_payload_vs_catalogue` reconstruisait un
     `Draft202012Validator` à chaque événement. Mis en cache par nom
     (`_payload_validators`).
- `uv run ruff check .` propre, `validate.py` toujours vert sur les 20
  fixtures réelles après les 5 correctifs.

### Pourquoi / décisions
- **`rssi` reste optionnel** (pas aligné sur « requis ») : je préfère corriger
  la doc de conception plutôt que le contrat, parce que j'ai une raison
  technique déjà actée ailleurs dans le dépôt (US-105) pour laquelle
  l'exiger serait faux, pas juste une paresse à corriger la conception.
- **`$ref` seulement côté JSON Schema**, pas de tentative de faire lire le
  fichier `.json` depuis `catalogue.py` au moment de l'import pour extraire
  le motif : ça introduirait un couplage fragile (ordre d'import, chemin
  relatif) pour économiser une ligne dupliquée.

### Écarts vs conception
- Un nouveau, décrit dans `03-ecarts-conception.md` : `pkt.seen.rssi`
  optionnel (point 2 ci-dessus).

### Appris
- Rien de nouveau cette session — les deux apprentissages ajoutés
  aujourd'hui dataient de la session précédente (US-107 initiale), juste pas
  encore propagés (point 3 ci-dessus).

### État après cette session
- Les 5 points de la revue d'@OswinFreyr sont traités.
- Fiche(s) module mise(s) à jour : [modules/contracts-events.md](modules/contracts-events.md)
- 01-etat-du-code.md mis à jour : non.

### Vérification (commandes réellement exécutées)
```
$ cd contracts && uv run python tools/validate.py
✓ 20 fixtures valides — 28 noms d'événements couverts.

$ uv run ruff check .
All checks passed!

# Reproduction du crash sans le fix (git stash sur validate.py, seq=-1
# injecté dans une copie de 01-pkt-seen.json, jamais committée) :
OverflowError: can't convert negative int to unsigned

# Avec le fix, même fixture mutée :
✗ 4 problème(s) :
  - schéma batch — -1 is less than the minimum of 0
  - signature invalide : Signature was forged or corrupt
  - batch_id ≠ hex(SHA-256(canonical_json(events)))
  - seq invalide (-1) : event_id non vérifiable
# Fixture restaurée (git checkout --) avant de committer.
```

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

### Retours de revue de Paul (2026-09-10)

Branche resynchronisée sur `main` (US-104 + US-115). Conflit `docs/suivi/`
résolu à la main (idem PR #59). Cinq retours, tous traités :

1. **`payloads.schema.json` généré mais jamais exercé** — `validate.py`
   validait les `payload` contre le `CATALOGUE` en mémoire et ne vérifiait que
   l'égalité fichier ↔ régénération. Le schéma JSON que US-217 va **consommer**
   n'était jamais confronté aux fixtures. Ajouté : chaque `{name, payload}` de
   fixture est aussi validé contre `payloads.schema.json`. Négatif vérifié
   (champ requis retiré → rejeté par les deux voies).
2. **`conv_hash` non réconcilié comme `msg_log_id`** — l'entrée
   `03-ecarts-conception.md` ne couvrait que `msg_log_id`. Élargie à **tous les
   identifiants pseudonymes tronqués** (`msg_log_id`, `conv_hash`,
   `from_peer`/`peer`/`to_peer`) : règle unique = 8 premiers octets → 16 hex.
   `CANONICAL.md` §3 mis à jour dans le même sens.
3. **`canonical_json` sans `allow_nan=False`** — le snippet « fait foi » et
   `catalogue.py` émettaient `NaN`/`Infinity` (JSON invalide) au lieu de lever.
   `allow_nan=False` ajouté aux deux ; règle du tableau §1 reformulée
   (« la sérialisation lève une erreur »).
4. **Discipline des nombres pour Rust** — `CANONICAL.md` §1 : ajout du piège
   `f64` (un entier resérialisé en `2.0` casse la signature) et de la consigne
   de désérialiser les champs numériques du catalogue en entier.
5. **Broutille : `batch_id` jamais recontrôlé** — `validate.py` recalcule
   `hex(SHA-256(canonical_json(events)))` et le compare. Négatif vérifié.

Au passage, 3 *code smells* SonarCloud sur `validate.py` (complexité cognitive
21 > 15, `if` imbriqué, littéral `"(global)"` ×3) : la fonction est éclatée en
petits `_check_*`, constante `GLOBAL`, `if` fusionné.

```
$ uv run ruff check .   → All checks passed!
$ uv run python tools/build_fixtures.py && git diff --exit-code -- events/   → stable
$ uv run python tools/validate.py   → ✓ 20 fixtures valides — 28 noms couverts.
```
## 2026-09-11 — US-109 : corrections suite à la revue de la PR #56

**Auteur :** Claude (Sonnet 5)
**Périmètre :** `android/gradle/verification-metadata.xml`,
`android/app/proguard-rules.pro`, `docs/suivi/modules/_index.md`
**Lot :** US-109 (suite), Sprint 1

### Fait
- Revue de @G1TS23 sur la PR #56 : `changes requested`, un point bloquant et
  deux nits.
- 🔴 Bloquant — `gradle/verification-metadata.xml` incomplet : checksum
  présent uniquement pour le `.pom` de `org.junit:junit-bom` (5.9.2 et
  5.9.3), pas pour le `.module` (Gradle Module Metadata), que Gradle
  résout et **préfère** depuis la version 6 quand les deux existent. Sur un
  clone frais (`GRADLE_USER_HOME` vide), la vérification de dépendances
  échouait dès la configuration du build (`Dependency verification failed
  for configuration ':classpath'`) — reproduit deux fois côté relecteur.
  Corrigé en vidant `~/.gradle/caches/modules-2` (le cache de résolution de
  dépendances, pas les téléchargements de distribution Gradle) puis en
  relançant `./gradlew --write-verification-metadata sha256 clean
  assembleDebug testDebugUnitTest assembleRelease` : le fichier régénéré
  contient maintenant les deux entrées `.module` (diff de 6 lignes
  seulement — rien d'autre n'a bougé).
- 🟡 Nit — `android/app/proguard-rules.pro:1` : le commentaire disait
  « release non minifiée au MVP », qui contredisait `isMinifyEnabled = true`
  / `isShrinkResources = true` (activés au round 1 des corrections
  SonarQube). Reformulé pour refléter l'état réel.
- 🟡 Nit — `docs/suivi/modules/_index.md` : deux tableaux distincts pour la
  fiche `android-app` (artefact de la fusion `merge=union`). Fusionnés dans
  le tableau principal (colonne `État` = esquisse, cohérent avec l'entête de
  `modules/android-app.md`), et la phrase « pas encore de fiche » ne cite
  plus l'app Android.

### Pourquoi / décisions
- Vidage ciblé de `caches/modules-2` plutôt que `rm -rf ~/.gradle` en entier
  (suggestion du relecteur) : suffisant pour forcer une résolution de
  dépendances à froid — donc pour faire réapparaître le bug — sans perdre le
  cache de distribution Gradle (évite un re-téléchargement de plusieurs
  minutes) ni le cache de transformation AAPT2 (une tentative avec un
  `GRADLE_USER_HOME` entièrement neuf a fait échouer le daemon AAPT2 pour
  une raison sans rapport avec ce correctif — environnement Windows local,
  pas creusé plus loin car hors sujet).

### Écarts vs conception
- Aucun.

### Appris
- Le mode `--write-verification-metadata` **n'échoue jamais** : il
  enregistre ce qui est résolu pendant le build au lieu de le vérifier. Si
  un artefact est déjà dans `caches/modules-2` (résolu lors d'un run
  antérieur, avant l'ajout de la dependency verification), sa génération de
  checksum peut être incomplète sans que rien ne le signale sur la machine
  où il tourne. Piège : ça ne se voit qu'au premier build sur une machine
  neuve (ou un `GRADLE_USER_HOME` vide) — donc régénérer systématiquement
  `verification-metadata.xml` depuis un cache de dépendances vidé, jamais
  depuis le poste de dev « chaud ». Ajouté à `04-apprentissages.md`.

### État après cette session
- Les trois points de la revue sont traités ; en attente d'un nouveau passage
  de @G1TS23.
- Fiche(s) module mise(s) à jour : [modules/android-app.md](modules/android-app.md)
- 01-etat-du-code.md mis à jour : non (toujours pointeur seul, pas de
  changement d'avancement).

### Vérification (commandes réellement exécutées)
```
$ rm -rf ~/.gradle/caches/modules-2
$ cd android && ./gradlew --write-verification-metadata sha256 clean assembleDebug testDebugUnitTest assembleRelease --console=plain
BUILD SUCCESSFUL in 2m 39s — 87 actionable tasks: 84 executed, 3 up-to-date
$ grep -n -A2 junit-bom gradle/verification-metadata.xml
→ confirme la présence des entrées junit-bom-5.9.2.module / 5.9.3.module
```
- **Non re-testé** depuis un `GRADLE_USER_HOME` totalement vide (échec
  AAPT2 sans rapport avec la dependency verification en cours de
  reproduction, voir ci-dessus) : la preuve de correction repose sur la
  régénération à froid du fichier de vérification, pas sur une répétition
  complète du scénario exact du relecteur.

---

## 2026-09-10 — Spike A : le cœur Rust cross-compile pour l'ESP32 (US-101)

**Auteur :** Paul Claverie + Claude (Opus 5)
**Périmètre :** `docs/suivi/spikes/US-101-cross-compile-xtensa.md` (nouveau),
`docs/suivi/README.md`, `docs/synthese/01-sujets-a-trancher.md` (B-1 et A-3).
**Aucun code applicatif** — c'est un spike, le code d'essai est jetable et reste
hors du dépôt (critère d'acceptation n°5).
**Lot :** Lot 0 — Fondations (issue #1, US-101). Branche
`spike/US-101-cross-compile-xtensa`.

### Fait
- Installé la toolchain Xtensa : `espup 0.17.1` puis `espup install` →
  toolchain `esp` (`rustc 1.97.0-nightly`, LLVM 21.1.3). La cible
  `xtensa-esp32-none-elf` **n'existe pas** dans le Rust amont, seulement dans le
  fork Espressif.
- Écrit une crate jouet `no_std` + `alloc`, `crate-type = ["staticlib"]` (la
  forme attendue par `08-relais-esp32.md:71`), et ajouté les briques crypto
  **une par une** avec compilation après chacune.
- Résultat : `sha2` 0.10.9, `ed25519-dalek` 2.2.0, `x25519-dalek` 2.0.1,
  `chacha20poly1305` 0.10.1 et `snow` **0.10.0** compilent tous. Compilation
  propre complète en 54,69 s, archive de 2 105 688 octets.
- Écrit et compilé le `CryptoResolver` qui branche `snow` sur
  `esp_fill_random()` de l'ESP-IDF.
- **B-1 tranchée : OUI, tout en Rust.** Consigné dans le rapport de spike, dans
  B-1 et dans A-3 (dont le repli mbedTLS est marqué « non activé »).

### Pourquoi / décisions
- **Dépendances ajoutées une par une, pas toutes d'un coup** : un échec groupé
  aurait donné « ça ne compile pas » sans dire quelle brique. C'est ce qui a
  permis d'isoler `snow` comme seul point dur.
- **Chaque brique est réellement appelée** derrière un `extern "C"` : sinon
  l'éditeur de liens élague le code et on « compile » du vide. Vérifié ensuite
  au `nm` que les 6 symboles sont bien dans l'archive.
- **`snow` 0.9.6 → 0.10.0** : le premier essai a échoué. Plutôt que de conclure
  « non » et d'activer le repli mbedTLS (deux implémentations crypto à
  maintenir), j'ai vérifié l'index crates.io : la 0.10.0 venait de sortir avec
  un vrai support `no_std`. C'est ce qui fait basculer la réponse du spike.

### Écarts vs conception
- **Aucun écart.** Le spike **confirme** l'hypothèse de
  `08-relais-esp32.md:71` (`libdengon_core.a` en `no_std + alloc` cross-compilé
  xtensa) et lève la condition qui y était attachée.

### Appris
- Cible tier 3, `-Z build-std`, `no_std` sans OS, features Cargo additives (on ne
  peut pas *retirer* une feature demandée par une dépendance) → notes ajoutées
  dans `04-apprentissages.md`, termes dans `05-glossaire.md`.

### État après cette session
- B-1 est fermée, la branche firmware (US-307 → 308 → 309 → 312) est débloquée
  et part sur du tout-Rust. Un des deux critères du jalon **J0** est acquis.
- Reste à faire, reporté aux US concernées : épingler `snow = "0.10"` (US-108),
  écrire le resolver ESP32 pour de vrai et vérifier l'entropie réelle
  d'`esp_fill_random` (US-307), valider le link dans un composant ESP-IDF
  (US-307), mesurer flash/RAM (US-308).
- Fiche(s) module mise(s) à jour : **aucune** — un spike ne livre pas de module.
  Le livrable est le rapport `spikes/US-101-cross-compile-xtensa.md`, et le
  dossier `spikes/` devient la convention pour les spikes B et C.

### Vérification (commandes réellement exécutées)
```
$ rustc +esp --print target-list | grep xtensa
xtensa-esp32-none-elf                      (present)

$ cargo build --release          # cible xtensa-esp32-none-elf, build-std
Finished `release` profile [optimized] target(s) in 54.69s     # 0 erreur

$ cargo tree -e normal | grep -c getrandom
0                                # getrandom totalement absent de l'arbre

$ xtensa-esp32-elf-nm libspike_us101.a | grep " T spike_"
spike_aead / spike_ed25519 / spike_noise_xx / spike_sha256 / spike_socle / spike_x25519

$ xtensa-esp32-elf-nm libspike_us101.a | grep esp_fill_random
         U esp_fill_random       # resolu au link final par l'ESP-IDF
```
- **Pas vérifié** : rien n'a tourné sur un vrai ESP32 (aucune carte utilisée) ;
  le link dans un projet ESP-IDF complet n'a pas été fait ; ni la taille flash
  réelle ni les performances n'ont été mesurées. Détaillé au §6 du rapport.

---

## 2026-09-09 — US-109 : corrections SonarQube Cloud, round 2 (PR #56)

**Auteur :** Claude (Sonnet 5)
**Périmètre :** `android/build.gradle.kts`, `android/app/build.gradle.kts`,
`android/gradle/libs.versions.toml` (nouveau), `android/gradle/verification-metadata.xml`
(nouveau), `android/settings-gradle.lockfile` (nouveau)
**Lot :** US-109 (suite), Sprint 1

### Fait
- Nouveau scan SonarCloud sur la PR #56 après le round 1 : 5 issues
  restantes (relevées via l'API `/api/issues/search?...pullRequest=56`) :
  1. `text:S8569` (MAJOR, VULNERABILITY) **toujours ouverte**, mais
     maintenant sur `android/build.gradle.kts` (le fichier racine, pas
     `app/`) : le `gradle.lockfile` ajouté au round 1 ne couvre que les
     configurations de dépendances de `:app` — il ne verrouille pas la
     résolution des **plugins** déclarés dans le `plugins{}` du build
     racine (`com.android.application`, `org.jetbrains.kotlin.android`),
     qui passe par un mécanisme de résolution différent (classpath de
     plugin, avant l'application des blocs `subprojects{}`).
  2. `kotlin:S6624` × 4 (MAJOR, CODE_SMELL) : numéros de version en dur
     dans `app/build.gradle.kts` lignes 64-66 et 75 (`core-ktx:1.13.1`,
     `lifecycle-runtime-ktx:2.8.4`, `activity-compose:1.9.1`,
     `junit:4.13.2`).
- Corrections :
  1. **Version catalog** `android/gradle/libs.versions.toml` : centralise
     toutes les versions (plugins + dépendances). `android/build.gradle.kts`
     et `app/build.gradle.kts` référencent désormais `libs.plugins.*` /
     `libs.*` au lieu de chaînes `"groupe:artefact:version"` — corrige les
     4 `kotlin:S6624`.
  2. **`gradle/verification-metadata.xml`** généré via `./gradlew
     --write-verification-metadata sha256 clean assembleDebug
     testDebugUnitTest assembleRelease` : contrairement au
     `gradle.lockfile` par sous-projet, la vérification de dépendances
     s'accroche au moteur de résolution lui-même et couvre **aussi** la
     résolution des plugins du build racine (vérifié : les entrées
     `com.android.application.gradle.plugin` / `org.jetbrains.kotlin.android
     .gradle.plugin` sont bien présentes dans le fichier généré) — corrige
     `text:S8569` sur `android/build.gradle.kts`.
  3. `app/gradle.lockfile` conservé (toujours valide, régénéré avec les
     mêmes coordonnées après le passage au catalogue) ; nouveau
     `settings-gradle.lockfile` produit en même temps par Gradle (verrou de
     l'import du catalogue lui-même, quasi vide, gardé par cohérence).
- Reconfirmé : `./gradlew clean assembleDebug testDebugUnitTest` avec la
  vérification de dépendances **active** (elle est appliquée à chaque build
  une fois `gradle/verification-metadata.xml` présent, pas seulement à la
  génération) → toujours vert.

### Pourquoi / décisions
- **Deux mécanismes de verrou gardés ensemble** (dependency locking pour
  `:app` + dependency verification pour tout le build, racine incluse)
  plutôt que de choisir l'un ou l'autre : la vérification est plus complète
  (couvre les plugins) mais son but premier est l'intégrité (checksums), pas
  la reproductibilité de résolution ; le locking reste utile si un jour une
  dépendance est déclarée avec une version dynamique. Peu de coût à garder
  les deux ici (peu de dépendances, projet naissant).
- **Version catalog plutôt que corriger ligne par ligne** : `kotlin:S6624`
  ne visait que 4 lignes sur les 9 dépendances versionnées du module, mais
  toutes auraient fini par être flaguées une à une ; centraliser une bonne
  fois dans `libs.versions.toml` (pratique standard Gradle/Android
  actuelle) règle la classe de problème plutôt que les symptômes.

### Écarts vs conception
- Aucun.

### Appris
- Le `gradle.lockfile` de dependency locking (`configurations.all {
  resolutionStrategy.activateDependencyLocking() }`) ne verrouille que les
  **configurations de dépendances** d'un projet Gradle ; il ne touche pas à
  la résolution du **classpath de plugin** (`plugins{}` / `pluginManagement`
  en settings), qui se produit avant même l'évaluation des blocs
  `subprojects{}`/`allprojects{}`. Pour verrouiller/vérifier aussi les
  plugins, il faut la **dependency verification** de Gradle
  (`gradle/verification-metadata.xml`, `--write-verification-metadata`).
  Ajouté à `04-apprentissages.md`.

### État après cette session
- Les 5 issues du round 2 devraient disparaître au prochain scan de la
  PR #56 (non re-vérifié : nécessite un push + re-run CI).
- Fiche(s) module mise(s) à jour : [modules/android-app.md](modules/android-app.md)
- 01-etat-du-code.md mis à jour : non (durcissement de build, pas de
  changement d'avancement fonctionnel)

### Vérification (commandes réellement exécutées)
```
$ cd android && ./gradlew assembleDebug testDebugUnitTest assembleRelease --console=plain
BUILD SUCCESSFUL in 1m 14s — 86 actionable tasks: 86 executed
(après passage au version catalog)

$ ./gradlew --write-verification-metadata sha256 clean assembleDebug testDebugUnitTest assembleRelease --console=plain
BUILD SUCCESSFUL in 46s — 87 actionable tasks: 84 executed, 3 up-to-date
→ gradle/verification-metadata.xml généré (2635 lignes)

$ grep -m5 "com.android.application\|org.jetbrains.kotlin.android" gradle/verification-metadata.xml
→ confirme la présence des artefacts de plugin

$ ./gradlew clean assembleDebug testDebugUnitTest --console=plain
BUILD SUCCESSFUL in 9s — 42 actionable tasks: 41 executed, 1 up-to-date
(build propre avec la vérification de dépendances active)
```
- **Non vérifié** : le nouveau scan SonarCloud (nécessite un push + re-run CI).

---

## 2026-09-09 — US-109 : corrections SonarQube Cloud (PR #56, Security Rating C)

**Auteur :** Claude (Sonnet 5)
**Périmètre :** `android/build.gradle.kts`, `android/app/build.gradle.kts`,
`android/app/src/main/AndroidManifest.xml`, `android/app/gradle.lockfile` (nouveau)
**Lot :** US-109 (suite), Sprint 1

### Fait
- PR #56 (squelette Android) bloquée par le Quality Gate SonarCloud :
  `Security Rating on New Code` = C (requis ≥ A). Trois vulnérabilités
  relevées via l'API SonarCloud (`/api/issues/search?...pullRequest=56`) :
  1. `kotlin:S7204` (MAJOR) — obfuscation désactivée en release
     (`app/build.gradle.kts:26`, `isMinifyEnabled = false`).
  2. `xml:S5332` (MINOR) — `usesCleartextTraffic` implicitement activé sur
     les anciennes versions d'Android (`AndroidManifest.xml:31`, pas
     d'attribut explicite).
  3. `text:S8569` (MAJOR) — pas de fichier de verrouillage des versions de
     dépendances (`android/build.gradle.kts`).
- Corrections :
  1. `isMinifyEnabled = true` + `isShrinkResources = true` sur le
     `buildType release`.
  2. `android:usesCleartextTraffic="false"` explicite sur `<application>`
     (l'app ne fait aucun appel HTTP dans ce squelette — BLE uniquement).
  3. `subprojects { configurations.all { resolutionStrategy
     .activateDependencyLocking() } }` dans `android/build.gradle.kts` +
     génération de `android/app/gradle.lockfile` via
     `./gradlew :app:dependencies --write-locks`.
- Revérifié après coup : `./gradlew assembleDebug testDebugUnitTest` et
  `./gradlew assembleRelease` (le release n'était pas testé avant — c'est
  justement le variant touché par le fix R8/minify) → tous verts.

### Pourquoi / décisions
- Fix ciblé sur les 3 findings réels plutôt qu'un durcissement générique :
  on corrige ce que Sonar a effectivement détecté, pas un audit de sécurité
  complet hors périmètre de l'US.
- `assembleRelease` n'était pas dans la vérification initiale de l'US-109
  (seul `assembleDebug` est un critère d'acceptation explicite) — ajouté ici
  car l'activation de R8/minify est justement le genre de changement qui
  peut casser silencieusement un build release (règles proguard manquantes
  pour Compose/reflection). Résultat : ça passe tel quel avec les consumer
  proguard rules d'AndroidX/Compose, aucune règle custom nécessaire pour
  l'instant.

### Écarts vs conception
- Aucun.

### Appris
- SonarCloud distingue `Security Rating` (vulnérabilités réelles, bloquant
  ce Quality Gate) de `Security Hotspots Reviewed` (hotspots à trier, gate
  séparée) — utile à savoir pour ne pas chercher au mauvais endroit la
  prochaine fois. Ajouté à `04-apprentissages.md`.

### État après cette session
- Les 3 findings devraient disparaître au prochain scan de la PR #56 (non
  re-vérifié ici : le nouveau scan tourne côté CI GitHub Actions, pas
  localement).
- Fiche(s) module mise(s) à jour : [modules/android-app.md](modules/android-app.md)
- 01-etat-du-code.md mis à jour : non (pas de changement d'avancement, juste
  un durcissement du squelette existant)

### Vérification (commandes réellement exécutées)
```
$ cd android && ./gradlew :app:dependencies --write-locks --console=plain
BUILD SUCCESSFUL — gradle.lockfile écrit pour :app et le buildscript racine

$ ./gradlew assembleDebug testDebugUnitTest --console=plain
BUILD SUCCESSFUL in 7s — 41 actionable tasks: 10 executed, 31 up-to-date

$ ./gradlew assembleRelease --console=plain
BUILD SUCCESSFUL in 45s — 46 actionable tasks: 46 executed
(minifyReleaseWithR8, shrinkReleaseRes exécutés sans erreur)
```
- **Non vérifié** : le nouveau scan SonarCloud sur la PR (nécessite un push
  + re-run CI, pas fait depuis cet environnement).

---

## 2026-09-09 — US-109 : squelette Android (Compose + service de fond BLE)

**Auteur :** Claude (Sonnet 5)
**Périmètre :** `android/` (nouveau module Gradle), `.gitignore`
**Lot :** US-109, Sprint 1 (`docs/olivier/proposition-organisation-github.md` §8.1)

### Fait
- Création du module Gradle `android/` (Kotlin DSL, AGP 8.5.2, Gradle 8.9,
  Kotlin 1.9.24, Jetpack Compose via BOM 2024.06.00). `applicationId` /
  `namespace` = `com.dengon.app` (non fixé par la conception — choisi ici,
  voir « Décisions » ci-dessous).
- `AndroidManifest.xml` : permissions BLE d'exécution `BLUETOOTH_SCAN`
  (`neverForLocation`), `BLUETOOTH_CONNECT`, `BLUETOOTH_ADVERTISE` (API 31+) ;
  `BLUETOOTH`/`BLUETOOTH_ADMIN`/`ACCESS_FINE_LOCATION` en repli (`maxSdkVersion=30`) ;
  `FOREGROUND_SERVICE` + `FOREGROUND_SERVICE_CONNECTED_DEVICE` ;
  `POST_NOTIFICATIONS` (API 33+).
- `ble/MeshForegroundService.kt` : service `foregroundServiceType="connectedDevice"`,
  notification permanente (canal `IMPORTANCE_LOW`), `START_STICKY`. Squelette
  seulement — pas encore de vraie logique GATT (arrive avec `AndroidTransport`,
  US-213).
- `ble/BlePermissions.kt` : liste les permissions à demander selon
  `Build.VERSION.SDK_INT` + vérifie si elles sont déjà accordées.
- `MainActivity.kt` (Compose) : demande les permissions à l'exécution
  (`ActivityResultContracts.RequestMultiplePermissions`), démarre/arrête le
  service, affiche l'état.
- Test unitaire minimal `BlePermissionsTest` (JVM pur, sans Robolectric).
- Correction `.gitignore` : `!gradle/wrapper/gradle-wrapper.jar` n'était
  ancré qu'à la racine → ajout de `!**/gradle/wrapper/gradle-wrapper.jar`
  pour que le wrapper d'un module non-racine (ici `android/`) soit versionné.
- SDK Android installé localement pour la vérification (cmdline-tools,
  `platform-tools`, `platforms;android-34`, `build-tools;34.0.0`) — pas encore
  dans le dépôt (outillage machine, pas du code).

### Pourquoi / décisions
- **Package / SDK versions non fixés par `docs/synthese/`** : choisis
  `com.dengon.app`, `minSdk=26` (API BLE peripheral stables sur la majorité
  des OEM), `compileSdk`/`targetSdk=34` (Android 14, la version qui impose
  `foregroundServiceType="connectedDevice"` d'après
  `docs/synthese/10-benchmarks-mvp-tests.md` §2.7). À reconfirmer en réunion
  si l'équipe veut une autre convention de nommage.
- **`neverForLocation` sur `BLUETOOTH_SCAN`** : le scan sert uniquement à
  détecter le service GATT `dengon`, jamais à dériver une position → évite
  de demander la localisation sur Android 12+.
- **`START_STICKY`** : le relais doit rester joignable ; si l'OS tue le
  service pour libérer de la mémoire, il doit redémarrer seul.
- Pas de logique BLE réelle dans le service : US-109 ne livre que le
  squelette (Compose + déclaration + permissions + notification), conforme
  au périmètre de l'US. La suite (GATT server/scanner/advertiser) est US-213.

### Écarts vs conception
- Aucun écart vs `docs/synthese/` : le choix de package/SDK versions est un
  **détail d'implémentation non spécifié**, pas une divergence — pas
  d'entrée dans `03-ecarts-conception.md`.

### Appris
- Rien de nouveau ajouté à `04-apprentissages.md` cette session (mise en
  place d'outillage plus que découverte conceptuelle).

### État après cette session
- `./gradlew assembleDebug` et `./gradlew testDebugUnitTest` passent
  localement (voir vérification ci-dessous).
- **Non vérifié** : le critère d'acceptation « le service tourne encore
  après ≥ 5 min écran éteint sur au moins un appareil réel » — nécessite un
  vrai téléphone Android, indisponible dans cet environnement d'exécution.
  **À faire avant de clore l'US** : installer l'APK sur un appareil réel,
  couper l'écran 5 min, vérifier (notification toujours affichée + `adb
  shell dumpsys activity services` montre le service actif), consigner le
  résultat ici en append.
- Pas de CI (`android.yml`) : hors périmètre US-109 (relève de US-113/US-222,
  pas encore faites). Le dépôt n'a donc **aucune CI verte** au sens de la DoD
  globale §7.1 pt.3 — attendu à ce stade du projet (premier code applicatif).
- Fiche(s) module mise(s) à jour : [modules/android-app.md](modules/android-app.md) (créée)
- 01-etat-du-code.md mis à jour : oui

### Vérification (commandes réellement exécutées)
```
$ cd android && ./gradlew.bat assembleDebug --console=plain
BUILD SUCCESSFUL in 1m 10s — 35 actionable tasks: 35 executed
APK généré : android/app/build/outputs/apk/debug/app-debug.apk

$ ./gradlew.bat testDebugUnitTest --console=plain
BUILD SUCCESSFUL in 5s — 23 actionable tasks: 7 executed, 16 up-to-date
```
- Manifeste fusionné inspecté (`app/build/intermediates/.../AndroidManifest.xml`) :
  présence confirmée de `foregroundServiceType="connectedDevice"` sur le
  service, des permissions BLE et notification.
- **Non exécuté** : test manuel des 5 minutes écran éteint sur appareil réel
  (pas de matériel Android dans cet environnement).
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

### Retours de revue (2026-09-10)

PR **approuvée** par `G1TS23` après vérification sur clone frais. Quatre
retours, aucun bloquant. Traitement :

- **`.gitignore` avale les fixtures de clés** — *corrigé ici*. Le reviewer a
  fait remarquer que l'US qui en souffrirait est **US-108 (crypto)**, sur le
  chemin critique, et que le mode d'échec silencieux est le pire possible.
  Quatre négations ajoutées, portée limitée à `crates/**/tests/**`. Voir
  [`03-ecarts-conception.md`](03-ecarts-conception.md).

- **« `dtolnay/rust-toolchain` lit déjà `rust-toolchain.toml`, l'étape *Lire la
  toolchain figée* est redondante »** — **inexact, et l'étape a été conservée.**
  Vérifié dans la source de l'action : `toolchain` y est déclaré
  `required: true`, et la première étape sort en erreur explicite
  (`'toolchain' is a required input`) si l'entrée est vide. Le mot `toml`
  n'apparaît nulle part dans son `action.yml`. Supprimer notre étape ferait
  donc **échouer le job immédiatement**. La seule alternative serait d'épingler
  la version dans le `@rev` de l'action (`dtolnay/rust-toolchain@1.98.1`), ce
  qui dupliquerait le numéro entre le workflow et `rust-toolchain.toml` —
  exactement ce que l'étape évite.
  Nuance en faveur du reviewer : rustup, *lui*, honore bien `rust-toolchain.toml`
  au moment où `cargo` s'exécute. Le fichier reste donc la source de vérité ;
  c'est l'action qui ne sait pas le lire.
  **Consigné ici pour que personne ne « simplifie » cette étape plus tard.**

- **`modules/_index.md` : modification structurelle d'un fichier `merge=union`**
  — exact. J'ai réécrit l'intro et fait passer le tableau de 3 à 4 colonnes,
  alors que le README de l'US-115 recommande de faire ça en PR seule. Sans
  conséquence ici (rien d'autre ne touchait le fichier) ; le reviewer
  reformatera les lignes de #59 / #60 à leur rebase. À retenir pour la suite.

- **`pub enum Verdict` inerte dans un binaire** — exact, et **déjà consigné**
  avant la revue dans [`modules/dengon-verify.md`](modules/dengon-verify.md)
  (« non importable depuis l'extérieur… il faudra scinder en `src/lib.rs` +
  `src/main.rs` »). Aucune action : l'échange avec le dashboard se fait par la
  sortie du processus, pas par l'API Rust.

Reste due : la revue `/crates/` de `OswinFreyr` (CODEOWNERS). Le push de ce
correctif **invalide l'approbation** de `G1TS23` (la protection de `main`
invalide les approbations à chaque push) ; il devra re-approuver.

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
