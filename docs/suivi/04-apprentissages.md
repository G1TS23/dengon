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

### Relier un lien radio à un pair : l'`ANNOUNCE` signé en tête de lien

**C'est quoi :** une connexion BLE ne dit pas **qui** est en face : le
transport ne rend qu'un `LinkId` (et au mieux 4 octets de `peerID` dans
l'annonce BLE, vus seulement par le côté qui scanne). Chaque côté écrit donc
son `ANNOUNCE` (clés publiques + pseudo, **signé** Ed25519) en première trame
du lien ; l'autre le vérifie et en déduit le `peerID`
(`SHA-256(pub_static)[0..8]`) à donner au nœud.
**Pourquoi dans dengon :** `api::Node` ne raisonne qu'en `peerID` ; sans cette
liaison, aucun octet radio ne pouvait atteindre le nœud (US-306).
**Piège / surprise :**
1. L'ordre suffit, pas besoin d'état « en attente » côté nœud : le transport
   garantit l'ordre **par lien**, et un pair écrit son `ANNOUNCE` avant de
   connaître le nôtre — donc avant tout autre paquet.
2. La signature seule ne prouve rien sur le `peerID` : il faut aussi vérifier
   `peerID == SHA-256(pub_static)[0..8]` (test « `peerID` usurpé »). Et même
   là, c'est le handshake `XX` qui prouve la possession de la clé statique.
3. Fin de handshake : l'initiateur écrit le message 3 **et** passe en session
   dans la même fonction. Tout ce que la session débloque (accusés) doit être
   mis en sortie **après** le message 3, sinon le répondeur reçoit un
   ciphertext avant d'avoir fini son handshake et le jette.
**Où c'est utilisé :** `crates/dengon-core/src/api.rs` (`announce_packet`,
`parse_announce`, `handle_handshake_message`),
`android/app/src/main/java/com/dengon/app/ble/Maillage.kt`.
**Pour aller plus loin :** `docs/synthese/07-cycle-de-vie-et-statuts.md` §6.

---

### UniFFI côté Kotlin : JNA, deux artefacts, et une lib hôte pour les tests JVM

**C'est quoi :** les bindings Kotlin qu'UniFFI génère n'utilisent pas JNI mais
**JNA**, qui charge `libdengon_ffi.so` par son nom à l'exécution
(`Native.load("dengon_ffi")`). Sur Android, JNA vient en **AAR** (avec son
`libjnidispatch.so` Android) ; dans un test JVM, il faut le **JAR** (avec le
`jnidispatch` de la machine) et une `libdengon_ffi.so` compilée pour
l'**hôte**, trouvée via `jna.library.path`.
**Pourquoi dans dengon :** c'est ce qui permet au test d'intégration
Kotlin ↔ Rust (US-302) de tourner en `testDebugUnitTest`, sans téléphone ni
émulateur.
**Piège / surprise :** trois pièges, tous rencontrés.
1. Lancé sur un `.udl` (hors métadonnées Cargo), `uniffi-bindgen` suppose que
   la lib s'appelle `uniffi_dengon` : il faut `cdylib_name = "dengon_ffi"` dans
   `uniffi.toml`, sinon tout compile et rien ne se charge.
2. L'AAR de JNA embarque `jnidispatch` pour 7 ABI. Sans `abiFilters`, l'APK
   s'installe sur un armv7 pour lequel `libdengon_ffi.so` n'existe pas → plantage
   au premier appel, pas à l'installation.
3. R8 (release) renomme les classes que JNA retrouve par réflexion : règles
   `-keep` obligatoires, sinon l'erreur n'apparaît qu'à l'exécution.
**Où c'est utilisé :** `crates/dengon-ffi/uniffi.toml`,
`android/app/build.gradle.kts` (`jna`, `abiFilters`, `jna.library.path`),
`android/app/proguard-rules.pro`, `android/app/src/test/java/com/dengon/app/ffi/FfiNatif.kt`.
**Pour aller plus loin :** <https://mozilla.github.io/uniffi-rs/latest/kotlin/gradle.html>

---
### Pile FreeRTOS et crypto Rust : mesurer, pas deviner (US-308)

Une tâche FreeRTOS a une pile **fixe**, choisie à sa création. Le code Rust
appelé par FFI met ses variables sur cette pile. Une signature Ed25519
(`ed25519-dalek`) plus les cadres du routeur ont pris ≈ 7,8 Ko dans
`dengon_route`, créée avec 6 Ko : `stack overflow in task dengon_route` à la
première ouverture de lien, invisible sur PC où les piles font des Mo. ESP-IDF
le détecte (canari de fin de pile) et redémarre. Le correctif est double :
plus de pile (16 Ko), et une **mesure** permanente,
`uxTaskGetStackHighWaterMark()` (plus petite marge jamais vue, en octets sur
ESP-IDF), imprimée dans le bilan de santé.

### ESP-IDF : `OK` est déjà pris, et `esp_fill_random` n'est pas toujours aléatoire (US-308)

- **Collision de noms C.** `rom/ets_sys.h`, inclus par la plupart des en-têtes
  système d'ESP-IDF, déclare un énumérateur `OK`. Un header généré par
  `cbindgen` avec `prefix_with_name = false` déclarait lui aussi `OK`,
  d'où `redeclaration of enumerator 'OK'` au premier `#include` côté
  firmware. En C, les énumérateurs partagent l'espace de noms global : un
  header de bibliothèque doit **toujours** préfixer (`DENGON_STATUS_OK`).
  Le test hôte (C sur PC) ne pouvait pas le voir.
- **Aléa.** `esp_fill_random()` n'est un vrai générateur matériel que si le
  Wi-Fi ou le Bluetooth est allumé, ou si `bootloader_random_enable()` a été
  appelé (bruit du SAR ADC). Sinon c'est un PRNG. `bootloader_random_enable()`
  doit être **coupé** avant d'allumer la radio. D'où l'ordre du relais :
  secrets tirés au tout début (source ADC), puis radio, puis auto-test Noise.
- **Reprise d'un journal chaîné après coupure.** Deux écritures (le fichier
  et le curseur) ne sont jamais atomiques ensemble. L'ordre choisi rend
  chaque coupure réparable : fichier fsync-é **d'abord**, curseur **ensuite**.
  Au boot, le fichier fait foi, sa fin incomplète est tronquée, et le curseur
  ne sert que quand le fichier est vide (après une rotation).
  `dengon-verify` refuse un fichier tronqué (code 65) : la réparation est
  indispensable, pas cosmétique.

### Noise `XX` : c'est l'**écriture**, pas la lecture, qui termine le handshake côté initiateur

**C'est quoi :** dans le patron `XX` (3 messages : `e` / `e,ee,s,es` /
`s,se`), le répondeur finit son échange en **lisant** le dernier message,
mais l'initiateur, lui, finit en **écrivant** ce même dernier message — il
n'y a rien à lire après. `is_finished()` (côté `snow`) ne devient vrai
qu'après l'opération qui clôt réellement l'échange, lecture ou écriture selon
le rôle.
**Pourquoi dans dengon :** `Node::handle_handshake_message` (`api.rs`,
US-301) est appelée sur **réception**, donc elle appelle toujours
`read_message` d'abord. Le réflexe naturel est de ne vérifier
`is_finished()` qu'après cette lecture — correct pour le répondeur, faux
pour l'initiateur, qui doit encore **écrire** avant que ce soit vraiment fini.
**Piège / surprise :** le bug ne casse rien de visible côté initiateur (il
envoie son dernier message sans erreur) — c'est le **répondeur** qui, plus
tard, rejette silencieusement tout ce que l'initiateur lui envoie en
session, parce que côté initiateur `PeerCrypto` est resté en `Handshaking`.
Sans test de bout en bout à deux nœuds réels, ça serait passé inaperçu :
chaque relecture du code semblait correcte isolément.
**Où c'est utilisé :** `crates/dengon-core/src/api.rs`,
`handle_handshake_message` ; découvert via `crates/dengon-core/tests/api_mock.rs`.
**Pour aller plus loin :** [spécification Noise, patron `XX`](https://noiseprotocol.org/noise.html#interactive-handshake-patterns-fundamental).
### `conn_handle` NimBLE vs `LinkId` : un identifiant recyclé n'est pas une identité

**C'est quoi :** NimBLE désigne chaque connexion par un `conn_handle`
(`uint16_t`) et **redonne le même numéro** à la connexion suivante dès que le
précédent est libéré. Le contrat `Transport` exige au contraire un `LinkId`
**jamais réutilisé** au cours d'une exécution.
**Pourquoi dans dengon :** une trame en retard sur un lien mort, ou un
événement GAP traité après la fermeture, serait attribué au pair **suivant** —
et la déduplication en amont ne rattraperait rien, elle raisonne sur le
`msgID`. Le cœur du transport tient donc une table `conn_handle ↔ LinkId`,
rompue à la fermeture ; le `LinkId` vient d'un compteur monotone 64 bits.
**Piège / surprise :** un test naïf (« deux connexions successives ont deux
`LinkId` différents ») passe même avec le bug si le banc donne des
`conn_handle` différents. Le banc Unity imite NimBLE et redonne le **plus
petit `conn_handle` libre** : c'est ce qui rend le test significatif.
**Où c'est utilisé :** `firmware/dengon-relay/components/dengon_transport_core/dengon_transport_core.c:146`
(`dengon_tc_link_open`), test `cas_link_id_jamais_reutilise`.
**Pour aller plus loin :** rustdoc de `LinkId`, `crates/dengon-ble/src/transport.rs`.

---

### Séparer un pilote radio en « cœur pur » + « glue » pour le tester sans matériel

**C'est quoi :** toute la logique qui ne dépend pas de la radio (états,
tables, files, validation, ordre des événements) est écrite en C pur dans un
composant qui ne connaît que des entiers (`conn_handle`, codes HCI). La glue
NimBLE ne fait que traduire les callbacks en appels au cœur. ESP-IDF sait
compiler un tel composant pour la cible **`linux`** : le binaire de test Unity
tourne alors sur le PC, en CI.
**Pourquoi dans dengon :** « testé sur 2 cartes réelles » ne peut pas tourner
en CI. Réduire ce qui n'est vérifiable qu'avec deux cartes au strict minimum
(la traduction) laisse 32 tests automatiques sur la sémantique, dont le
portage 1:1 des 12 cas de conformité Rust.
**Piège / surprise :** (1) Unity attribue par défaut **tous** les cas au
fichier de `UNITY_BEGIN` — faux numéros de ligne dans les échecs ; remède :
`UnitySetTestFile(__FILE__)` au début de chaque lanceur. (2) Sur la cible
linux, `app_main` doit appeler `exit()` avec le résultat, sinon la CI ne voit
jamais un échec. (3) Deux cibles dans le même projet s'écrasent : `-B
build-esp32 -D SDKCONFIG=build-esp32/sdkconfig` pour la seconde. (4) Un test
de mutation (injecter volontairement le bogue « purge de la file à la
fermeture ») a confirmé que la suite le voit : 2 échecs, code de sortie 1.
**Où c'est utilisé :** `firmware/dengon-relay/components/dengon_transport_core/test_apps/`,
étape « Tests Unity du transport » de `.github/workflows/firmware.yml`.
**Pour aller plus loin :** <https://docs.espressif.com/projects/esp-idf/en/v5.5/esp32/api-guides/host-apps.html>

---

### BLE : ce qui s'arrête tout seul et qu'il faut relancer

**C'est quoi :** trois activités radio de NimBLE s'arrêtent sans que le code
le demande : l'**annonce** dès qu'un pair se connecte en périphérique ; le
**scan**, qu'il faut couper soi-même avant `ble_gap_connect()` (sinon
`BLE_HS_EBUSY`) ; et un scan avec **filtre de doublons** qui, lancé « pour
toujours », ne reverra jamais une carte déjà vue une fois — même partie puis
revenue.
**Pourquoi dans dengon :** c'est exactement le critère « survit à une
déconnexion brutale » : après une coupure, la carte doit redevenir visible et
retrouver ses voisins. Chaque fin d'activité (`DISCONNECT`, `ADV_COMPLETE`,
`DISC_COMPLETE`, échec de connexion) repasse par `ensure_advertising()` et
`ensure_scanning()`, et le scan tourne par passes de 10 s pour que le filtre de
doublons soit remis à zéro.
**Piège / surprise :** une coupure brutale n'a **aucun** message associé : le
lien meurt au *supervision timeout* (code HCI `0x08`), quelques secondes plus
tard. Une déconnexion « propre » est un `LL_TERMINATE_IND` du pair (code
`0x13`), une fermeture par nous revient avec `0x16`. Et NimBLE ne rend pas le
code HCI brut mais `BLE_HS_ERR_HCI_BASE + code`. **Couper le Bluetooth
d'un téléphone n'est pas une coupure brutale** : Android envoie d'abord un
`LL_TERMINATE_IND` (`0x15`, « power off »). Pour en provoquer une vraie
depuis un Pixel sans le déplacer : `adb shell am force-stop
com.google.android.bluetooth` tue la pile sans prévenir → `0x08` ~5 s plus
tard côté ESP32.
**Où c'est utilisé :** `firmware/dengon-relay/main/transport_nimble.c`
(`on_disconnect`, `ensure_scanning`), `dengon_tc_map_hci_reason`.
**Pour aller plus loin :** Core Spec v5.4, Vol 1 Part F (codes d'erreur).
### `merge=union` est un pilote LOCAL — GitHub ne l'applique pas

**C'est quoi :** dans `.gitattributes`, `fichier merge=union` demande à git, en
cas de conflit sur ce fichier, de garder **les deux côtés** au lieu d'écrire des
marqueurs. C'est le filet posé par l'US-115 sur les six fichiers de suivi en
append (journal, avancement, écarts, apprentissages, glossaire, index).

**Pourquoi dans dengon :** six à sept PR sont ouvertes en permanence et **toutes**
écrivent dans `00-journal.md`, au même endroit (juste sous le marqueur
« NOUVELLES ENTRÉES ICI »). Sans le filet, chaque PR conflicterait avec toutes
les autres.

**Piège / surprise :** deux, découverts le même jour sur la PR #105.

1. **Le filet ne marche que sur le poste.** Les pilotes de fusion de
   `.gitattributes` sont exécutés par le git **local** ; le serveur GitHub
   fusionne avec le pilote par défaut et **ignore** `merge=union`. La PR est
   donc sortie en `mergeStateStatus: DIRTY` sur le seul `00-journal.md`, alors
   qu'un `git rebase origin/main` en local passait sans un conflit. Le remède
   est le rebase local puis un `push --force-with-lease` — pas un réglage de
   dépôt. L'en-tête de `02-avancement.md` promet « fusion automatique ;
   `merge=union` sert de filet » : à lire comme « en local ».
2. **Union ≠ fusion juste.** Sur un fichier en *append* (le journal), garder les
   deux côtés donne le bon résultat. Sur une **table éditée en place**
   (`02-avancement.md`), si deux branches touchent la **même ligne**, union
   garde les **deux versions** — la périmée et la neuve. C'est arrivé sur trois
   lignes (`Workflow firmware`, `dashboard`, `contracts`) et il a fallu retirer
   les doublons à la main. La consigne « on modifie uniquement la ligne du
   composant touché » n'y suffit pas : il faut que deux PR ne touchent pas la
   **même** ligne.

**Où c'est utilisé :** `.gitattributes`, `docs/suivi/*`.

**Pour aller plus loin :** `gitattributes(5)`, section « Defining a custom merge
driver » — et `git check-attr merge -- docs/suivi/00-journal.md` pour vérifier
que l'attribut est bien actif localement.

---

### Unification des features de Cargo : `--no-default-features` ne fait pas ce qu'on croit

**C'est quoi :** quand plusieurs paquets d'un même graphe de build dépendent
d'une même crate avec des features différentes, Cargo ne compile **pas** cette
crate plusieurs fois : il fait l'**union** des features demandées et compile
une seule version. C'est ce qui garde les temps de build raisonnables — et
c'est très bien, sauf quand la configuration *est* ce qu'on veut tester.

**Pourquoi dans dengon :** l'US-222 devait rejouer les vecteurs de trame
contre `dengon-core` compilé **sans `std`**, la configuration que l'ESP32
embarquera. Le réflexe — `cargo test -p dengon-core --no-default-features` —
ne marche pas : `dengon-core` a `dengon-ble` en **dev-dependency** (pour les
tests de `sync::routing`), `dengon-ble` dépend de `dengon-core` avec ses
features par défaut, donc `std` revient par la porte de derrière, avec
`rusqlite` et son sqlite3 en C. Le test aurait tourné vert en n'ayant rien
prouvé.

**Piège / surprise :** trois pièges empilés, découverts dans cet ordre.

1. `cargo tree -p dengon-core --no-default-features -e features | grep rusqlite`
   le montre en une ligne. À réflexe pour toute question « quelle feature est
   réellement active ? » — l'intuition se trompe, l'arbre non.
2. La crate séparée (`crates/dengon-conformance/`) ne suffit pas non plus si
   elle écrit `dengon-core = { workspace = true, default-features = false }` :
   avec l'héritage de workspace, **`default-features` est ignoré** tant que
   la ligne de `[workspace.dependencies]` ne le déclare pas elle-même. Cargo
   le dit, en avertissement facile à survoler : *« `default-features` is
   ignored for dengon-core »*. Il a fallu écrire la dépendance **en chemin**.
3. Une assertion « je suis bien sans `std` » compilée dans la crate
   (`const _: () = assert!(...)`) casse `cargo clippy --workspace` : un build
   `--workspace` unifie tout le graphe, `std` revient par `dengon-node`, et
   l'assertion échoue **à raison**. La garantie ne vit que dans la résolution
   **isolée** (`-p dengon-conformance`), donc le garde-fou doit être à
   l'extérieur : une étape de CI qui inspecte `cargo tree`.

**Où c'est utilisé :** `crates/dengon-conformance/Cargo.toml`,
`crates/dengon-conformance/src/lib.rs`,
`.github/workflows/cross-vectors.yml` (étape « 2/4 bis »).

**Pour aller plus loin :** *The Cargo Book*, « Features — Feature unification »
et « Dependencies — Inheriting a dependency from a workspace ».

---

### `dorny/paths-filter` n'a pas de base de comparaison sur `schedule`

**C'est quoi :** l'action calcule un diff entre deux références git pour dire
quels chemins ont changé. Sur `pull_request` elle compare à la base de la PR,
sur `push` au commit précédent. Sur `schedule` (un cron), il n'y a **ni PR ni
push** : rien à comparer.

**Pourquoi dans dengon :** le job `audit` tourne sur PR **et** en cron
quotidien. Sur PR, on veut le filtrage par chemin (ne pas relancer un audit
pour une PR documentaire) ; sur cron, on veut **tout** exécuter — c'est
justement le cas où le dépôt n'a pas bougé mais où un avis RUSTSEC vient de
paraître. Les deux besoins sont opposés, et un seul `if` ne les couvre pas.

**Piège / surprise :** il faut désactiver l'**étape de filtre elle-même**
(`if: github.event_name != 'schedule'`), pas seulement l'ignorer ensuite —
sinon elle échoue avant d'avoir servi. Et comme un `steps.filtre.outputs.*`
d'une étape sautée vaut la chaîne vide, chaque étape réelle porte
`if: github.event_name == 'schedule' || steps.filtre.outputs.deps == 'true'`.

**Où c'est utilisé :** `.github/workflows/audit.yml`.

---

### `cargo-deny` : quatre contrôles, un seul fichier, et le piège du code privé

**C'est quoi :** `cargo deny check` lit `Cargo.lock` et vérifie quatre choses
d'un coup — `advisories` (avis RUSTSEC), `licenses` (celles qu'on autorise),
`bans` (doublons de version, dépendances en `"*"`), `sources` (d'où viennent
les crates). `cargo audit`, lui, ne fait que le premier, mais avec une base
d'avis rafraîchie à chaque exécution.

**Pourquoi dans dengon :** les deux sont dans le job `audit`
(`synthese/10` §4.7). Ils se recouvrent sur les avis, et c'est voulu : ils ne
rafraîchissent pas leur base au même moment.

**Piège / surprise :** trois.

1. **Notre propre code fait échouer le job** si on ne dit rien : nos sept
   crates sont `publish = false` et sans champ `license` (la licence du
   projet n'est pas tranchée), donc cargo-deny les compte *unlicensed*.
   `[licenses.private].ignore = true` règle ça.
2. **La liste `allow` dépend de `[graph].targets`.** Deux licences
   (`Apache-2.0 WITH LLVM-exception`, `BSD-1-Clause`) n'apparaissaient que sur
   des cibles qu'on ne construit pas ; cargo-deny les a signalées en
   `license-not-encountered`. Une entrée inutile dans `allow` est une
   permission qu'on ne comprend plus six mois après.
3. **`cargo audit` ne lit pas `deny.toml`** (son fichier serait
   `.cargo/audit.toml`). Deux listes d'exceptions dérivent au premier oubli :
   le workflow **dérive** ses `--ignore` de `deny.toml` par un `grep` sur les
   identifiants `RUSTSEC-AAAA-NNNN`.

**Où c'est utilisé :** `deny.toml`, `.github/workflows/audit.yml`.

**Pour aller plus loin :** <https://embarkstudios.github.io/cargo-deny/>

---

### Machine à états : une fonction pure + un property test de monotonie

**C'est quoi :** au lieu de disperser des `if statut == …` dans le code, toutes
les transitions passent par **une seule fonction pure** `next_status(statut,
événement) -> Option<statut>`. Un *property test* (crate `proptest`) génère
ensuite des centaines de suites d'événements aléatoires et vérifie une
**propriété** plutôt qu'un exemple : « le rang du statut ne baisse jamais, un
statut terminal ne bouge plus ».
**Pourquoi dans dengon :** les événements arrivent dans le désordre et en
double (Ack rejoué par plusieurs relais, `cancel` après l'envoi, redémarrage
en plein milieu). Un statut qui redescend (« Distribué » → « Parti ») serait
un bug visible pour l'utilisateur.
**Piège / surprise :** la propriété doit aussi tenir **à travers un
redémarrage** : le second property test mélange des opérations d'outbox et
des « redémarrages » (l'outbox est détruite, seul le stockage survit). Autre
piège : persister **avant** de modifier la mémoire, sinon un échec d'écriture
laisse en mémoire un état qui disparaîtra au redémarrage.
**Où c'est utilisé :** `crates/dengon-core/src/sync/status.rs:186`
(`next_status`), property tests dans le même fichier et dans
`crates/dengon-core/src/sync/status/outbox/tests.rs`.
**Pour aller plus loin :** <https://proptest-rs.github.io/proptest/>
### Un property test trouve le cas que l'exemple rate

**C'est quoi :** un property test (`proptest`) génère des entrées au hasard,
vérifie une propriété, et quand elle casse il **réduit** l'entrée jusqu'au plus
petit contre-exemple (*shrinking*).
**Pourquoi dans dengon :** pour la fragmentation, la propriété est « avec un MTU,
un ordre d'arrivée et des doublons quelconques, le paquet ressort exactement une
fois, identique ». Les tests écrits à la main (paquets de 300 à 1000 octets)
passaient tous.
**Piège / surprise :** le test a réduit l'échec à `p = [0]` avec un doublon : un
paquet d'**un seul** fragment n'est jamais mis en attente, donc rien ne se
souvient qu'il est déjà sorti, et le doublon le fait ressortir. Personne
n'avait pensé à ce cas limite ; la correction (mémoire bornée des `frag_id`
terminés) a aussi supprimé les réassemblages « zombies » ouverts par un doublon
tardif.
**Où c'est utilisé :** `crates/dengon-core/src/protocol/fragment/tests.rs`
(`reassemblage_mtu_et_ordre_aleatoires`).
**Pour aller plus loin :** <https://proptest-rs.github.io/proptest/proptest/tutorial/shrinking-basics.html>
### Un QR code peut être valide et pourtant indétectable

**C'est quoi :** un lecteur de QR fait deux choses : **détecter** le code dans
l'image (les trois carrés de repérage) puis **décoder** les modules. Le
contenu est brouillé par un **masque** (8 possibles) choisi par l'encodeur
pour éviter les motifs gênants.
**Pourquoi dans dengon :** l'appairage repose sur le scan caméra du QR de
l'autre. Un QR que le détecteur ne trouve pas bloque l'appairage.
**Piège / surprise :** le QR de l'identité de test « alice », généré par
ZXing avec son masque par défaut, se décodait en mode « image pure » (données
justes) mais n'était **jamais détecté** — à toute échelle, même en
`TRY_HARDER` —, alors que ceux de « bob » ou « Élodie » passaient. Le défaut
dépend du contenu : un test sur un seul exemple ne l'aurait pas vu.
Correctif : vérifier la relecture par détection juste après l'encodage et
changer de masque si besoin, plus un balayage de 300 identités en test.
**Où c'est utilisé :** `android/app/src/main/java/com/dengon/app/ui/appairage/QrCode.kt`
(`matriceQr`, `seRelitParDetection`).
**Pour aller plus loin :** ISO/IEC 18004 (masques et évaluation des pénalités).

---

### L'encodage hexadécimal double la taille en octets

**C'est quoi :** représenter N octets en hexadécimal ASCII (`"%02x"` par
octet) produit 2×N caractères, donc 2×N octets une fois ré-encodés en
UTF-8 (chaque caractère hexadécimal est un octet ASCII). Ce n'est **pas**
une transformation 1:1 octet à octet.
**Pourquoi dans dengon :** `IdentiteLocale.pseudoPour` doit tenir dans les 8
premiers octets du pseudo (c'est toute la fenêtre que le bouchon FFI utilise
pour dériver le `peerId`, US-215/US-106). Vouloir y faire tenir 8 octets de
véritable aléa en les hexadécimant en donnerait 16 — la moitié déborderait
hors de la fenêtre utile.
**Piège / surprise :** la revue automatisée de la PR #94 recommandait
d'« utiliser les 8 octets disponibles pour l'aléatoire… donnerait 2^64
valeurs possibles », en supposant implicitement un octet source par octet de
pseudo. En pratique, avec un encodage hexadécimal, seuls **4** octets de
source aléatoire tiennent dans 8 octets de pseudo — 2^32 valeurs, pas 2^64.
Toujours vrai que c'est un gain énorme sur les 2^16 d'avant (le préfixe
constant `tel-` gaspillait la moitié de la fenêtre), mais le chiffre exact
de la revue ne tenait pas compte du doublement de taille de l'encodage.
**Où c'est utilisé :** `android/app/src/main/java/com/dengon/app/identite/IdentiteLocale.kt`
(`pseudoPour`, `OCTETS_ALEATOIRES = 4`).
**Pour aller plus loin :** RFC 4648 (encodages base16/base32/base64 et leurs
ratios octets source / octets encodés).

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
### Un docstring qui énumère ce qu'il vérifie est une mesure de couverture

**C'est quoi :** quand un test porte un commentaire du type « vérifie les points
1, 4 et 5 du contrat », ce commentaire devient comparable à la liste des règles
du contrat. Le trou de couverture se lit alors à l'œil nu, sans instrumenter
quoi que ce soit — bien avant qu'un rapport `llvm-cov` puisse aider, puisque
celui-ci mesure des **lignes exécutées**, pas des **règles vérifiées**.

**Pourquoi dans dengon :** c'est exactement comme ça que la revue de la PR #65 a
trouvé que la règle 2 du contrat de déconnexion brutale n'était testée nulle
part. Le corps de la PR affirmait « vérifié par `cas_deconnexion_brutale` » ; le
docstring de ce cas disait « vérifie les points 1, 4 et 5 ». Les deux phrases se
contredisaient dans le même dépôt, et la couverture de lignes était à 100 % sur
ce fichier — elle ne pouvait rien y voir.

**Piège / surprise :** l'énumération doit être **maintenue**, sinon elle ment et
devient pire qu'absente. Le corollaire utile : quand une règle n'est
délibérément pas testée, l'écrire noir sur blanc (ici le point 3, les fragments
partiels, que `MockTransport` ne peut pas simuler) vaut mieux qu'un cas vide qui
passe au vert sans rien prouver.

**Où c'est utilisé :** `crates/dengon-ble/src/conformance.rs`, section « Ce que
la suite ne vérifie pas » du module doc, et le docstring de
`cas_deconnexion_brutale`.
**Pour aller plus loin :** PR #65, revue de G1TS23 du 2026-09-11.

---
### Cible « tier 3 » et `-Z build-std` — quand `core` n'est pas livré compilé

**C'est quoi :** Rust classe ses cibles en 3 niveaux. Une cible **tier 3** est
supportée par le compilateur mais **personne ne distribue de `core`/`alloc`
précompilés** pour elle. Il faut donc les rebâtir depuis les sources à chaque
projet, via `-Z build-std=core,alloc` — une option *nightly*.
**Pourquoi dans dengon :** `xtensa-esp32-none-elf` est tier 3, et n'existe même
pas dans le Rust amont : seul le **fork d'Espressif** (installé par `espup`) la
connaît. D'où une toolchain `esp` séparée, en plus de la 1.98.1 figée par
`rust-toolchain.toml`.
**Piège / surprise :** `rustup target add xtensa-esp32-none-elf` ne marchera
jamais sur la toolchain du dépôt — le message d'erreur ne dit pas qu'il faut un
autre compilateur. Et `espup install` pèse **1,9 Go** : à prévoir dans la CI si
on veut y compiler le firmware un jour.
**Où c'est utilisé :** spike US-101 (crate jetable, hors dépôt) ; à reprendre en
US-307. Voir [`spikes/US-101-cross-compile-xtensa.md`](spikes/US-101-cross-compile-xtensa.md) §3.
**Pour aller plus loin :** <https://docs.esp-rs.org/book/>

### Les features Cargo sont **additives** — on ne peut pas en retirer une

**C'est quoi :** quand une crate A dépend de B avec `features = ["std"]`, aucun
utilisateur de A ne peut désactiver ce `std`. Cargo *unifie* les features
demandées par tout le graphe : elles ne peuvent que s'ajouter. `default-features
= false` ne retire que les features **par défaut**, jamais celles explicitement
demandées par une dépendance intermédiaire.
**Pourquoi dans dengon :** c'est ce qui rend `snow` 0.9.6 **définitivement**
incompatible `no_std` : son manifeste écrit `rand_core = { features = ["std",
"getrandom"] }` en dur. Aucun réglage de notre côté n'y change quoi que ce soit.
La 0.10.0 a dû rendre ces dépendances optionnelles pour que ça devienne possible.
**Piège / surprise :** on perd facilement une heure à essayer des combinaisons de
features avant de comprendre que la réponse est dans le `Cargo.toml` **de la
dépendance**. Le réflexe qui fait gagner du temps : lire
`~/.cargo/registry/src/*/<crate>-<version>/Cargo.toml` **avant** de tâtonner.
**Où c'est utilisé :** [`spikes/US-101-cross-compile-xtensa.md`](spikes/US-101-cross-compile-xtensa.md) §5.

### `no_std` : ce qui disparaît, et ce que le compilateur ne dira pas

**C'est quoi :** sans système d'exploitation, la bibliothèque standard n'existe
pas. Restent `core` (le langage) et, si on fournit un allocateur, `alloc`
(`Vec`, `Box`, `String`). Il faut déclarer soi-même un `#[global_allocator]` et
un `#[panic_handler]`.
**Pourquoi dans dengon :** le relais ESP32 embarque `libdengon_core.a` en
`no_std + alloc` (`docs/synthese/08-relais-esp32.md:71`), et `dengon-core` porte
déjà le `#![cfg_attr(not(feature = "std"), no_std)]` posé par US-104.
**Piège / surprise :** deux pièges non détectés par le compilateur.
1. **Le code non appelé est élagué.** Déclarer une dépendance ne prouve rien : si
   on ne l'appelle pas depuis un symbole exporté, l'archive ne la contient pas et
   on croit avoir « compilé » du vide. Vérifier au `nm`.
2. **L'aléa devient un problème d'exécution.** Sans OS, pas de `/dev/urandom` :
   `getrandom` échoue *à la compilation* (tant mieux), mais une crate qui rend sa
   source d'aléa optionnelle — comme `snow` 0.10 — **compile** puis échoue *au
   runtime*. Le firmware doit fournir son propre RNG matériel.
**Où c'est utilisé :** [`spikes/US-101-cross-compile-xtensa.md`](spikes/US-101-cross-compile-xtensa.md) §4-5.


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

### SonarCloud : Security Rating vs Security Hotspots Reviewed

**C'est quoi :** deux conditions de Quality Gate distinctes. `Security
Rating` = pire sévérité parmi les issues de type **Vulnerability**
(bugs de sécurité avérés, ex. obfuscation désactivée, cleartext traffic
ambigu, dépendances non verrouillées). `Security Hotspots Reviewed` = %
de **Security Hotspots** (code sensible à trier manuellement, ex. usage de
crypto, permissions) qui ont été revus — gate séparée.
**Pourquoi dans dengon :** la PR #56 (squelette Android, US-109) a été
bloquée par `Security Rating on New Code = C`. Chercher dans les
Hotspots aurait été une perte de temps : il fallait l'onglet
`Vulnerabilities` / filtre `types=VULNERABILITY` de l'API
`/api/issues/search`.
**Piège / surprise :** le nom de la gate ne dit pas explicitement
« Vulnerabilities » — facile de confondre avec les Hotspots qui, eux,
demandent une revue humaine plutôt qu'un vrai fix de code.
**Où c'est utilisé :** `android/app/build.gradle.kts` (release
`isMinifyEnabled`), `android/app/src/main/AndroidManifest.xml`
(`usesCleartextTraffic`), `android/build.gradle.kts` (dependency locking).
**Pour aller plus loin :** `https://sonarcloud.io/api/issues/search?componentKeys=<projet>&pullRequest=<n>&types=VULNERABILITY`.

### Gradle : dependency locking vs dependency verification

**C'est quoi :** deux mécanismes Gradle différents, souvent confondus.
**Dependency locking** (`gradle.lockfile`, `resolutionStrategy
.activateDependencyLocking()`) fige les versions **résolues** d'un
sous-projet pour la reproductibilité (utile surtout avec des versions
dynamiques, `1.+`). **Dependency verification**
(`gradle/verification-metadata.xml`, `--write-verification-metadata`)
enregistre des **checksums** de tout ce que Gradle télécharge, pour
l'intégrité (détecter un artefact corrompu/remplacé) — et ça couvre aussi
la résolution des **plugins**, que le locking ne touche pas.
**Pourquoi dans dengon :** le `gradle.lockfile` de `:app` ne suffisait pas
à faire disparaître `text:S8569` (Sonar) sur `android/build.gradle.kts` —
c'est le fichier racine où sont déclarés les plugins (AGP, Kotlin), résolus
*avant* que les blocs `subprojects{}` (et donc le locking) s'appliquent.
**Piège / surprise :** les deux mécanismes ont des fichiers différents mais
tous deux qualifiés de « lock file » en langage courant — la doc Sonar
elle-même les traite comme équivalents (« gradle.lockfile **or**
verification-metadata.xml ») alors qu'ils ne couvrent pas le même
périmètre de résolution.
**Où c'est utilisé :** `android/app/gradle.lockfile` (locking),
`android/gradle/verification-metadata.xml` (verification, régénéré via
`./gradlew --write-verification-metadata sha256 <tasks>`).
**Pour aller plus loin :** doc Gradle « Verifying dependencies » et
« Locking dependency versions ».

### `--write-verification-metadata` n'échoue jamais — piège du cache chaud

**C'est quoi :** en mode écriture (`./gradlew --write-verification-metadata
sha256 <tasks>`), Gradle **enregistre** les checksums de ce qu'il résout
pendant ce build précis, il ne **vérifie** rien. Si un artefact est déjà
présent dans `~/.gradle/caches/modules-2` (résolu lors d'un run antérieur,
avant l'ajout de la vérification), sa checksum peut manquer sans que la
commande échoue ou avertisse.
**Pourquoi dans dengon :** relevé en revue de la PR #56 —
`gradle/verification-metadata.xml` ne contenait que le `.pom` de
`org.junit:junit-bom` (5.9.2/5.9.3), pas le `.module` (Gradle Module
Metadata, préféré par Gradle depuis la version 6 dès qu'il existe). Sur un
clone frais où la vérification s'applique **réellement** (mode normal, pas
`--write-verification-metadata`), le build échouait dès la configuration
(`Dependency verification failed for configuration ':classpath'`) —
invisible sur le poste où le fichier avait été généré, parce que ce
`.module` y était déjà en cache.
**Piège / surprise :** régénérer `verification-metadata.xml` « ça marche »
localement ne prouve rien tant que le `GRADLE_USER_HOME` n'est pas
repassé à froid — le seul test fiable est de vider
`~/.gradle/caches/modules-2` (ou d'utiliser un `GRADLE_USER_HOME` vide)
avant de régénérer, sinon le fichier peut être incomplet à l'insu de son
auteur et casser seulement sur la machine de quelqu'un d'autre (ou la
future CI).
**Où c'est utilisé :** `android/gradle/verification-metadata.xml`.
**Pour aller plus loin :** doc Gradle « Gradle Module Metadata » — pourquoi
`.module` est préféré à `.pom` quand les deux sont publiés.

### `BluetoothGatt` Android : les opérations ne sont pas mises en file d'attente

**C'est quoi :** sur une même connexion GATT, `BluetoothGatt` (côté client)
n'accepte qu'**une opération asynchrone à la fois** (`writeCharacteristic`,
`readCharacteristic`, `writeDescriptor`, `requestMtu`, `discoverServices`…).
Le framework ne fait **pas** de file d'attente interne : il attend que le
callback de l'opération en cours revienne avant d'en accepter une nouvelle
sur la même connexion.
**Pourquoi dans dengon :** dans le Spike C (US-103), `HelloMeshCentral`
enchaînait `g.writeDescriptor(cccd)` (activation des notifications) puis
`g.writeCharacteristic(rx)` sans attendre la fin du premier — relevé en
revue de la PR #67.
**Piège / surprise :** l'échec de la deuxième opération est **silencieux** :
pas d'exception, `writeCharacteristic` renvoie simplement `false` (ou
l'opération est ignorée). Sans logguer la valeur de retour, ça ressemble à
un blocage côté matériel plutôt qu'à un bug de séquencement. La bonne
pratique est de chaîner chaque opération GATT **depuis le callback de fin**
de la précédente (`onDescriptorWrite` → déclenche `writeCharacteristic`,
etc.), ou de maintenir sa propre file. Symétrique côté serveur GATT
(`BluetoothGattServer`) : toute écriture avec accusé (`WRITE_TYPE_DEFAULT`)
attend un `sendResponse()` explicite ; sans lui, l'écriture ne se termine
jamais proprement côté client (timeout ATT).
**Où c'est utilisé :** `android/app/src/main/java/com/dengon/app/ble/spike/HelloMeshCentral.kt`
(`onDescriptorWrite`), `HelloMeshPeripheral.kt` (`onDescriptorWriteRequest`).
**Pour aller plus loin :** doc Android « BluetoothGatt » — section sur le
séquencement des opérations ; issue tracker AOSP historique sur ce
comportement (recherche « BluetoothGatt operation already in progress »).
---

### Régénérer `verification-metadata.xml` ne couvre que l'OS de la machine qui régénère

**C'est quoi :** certains artefacts Maven publient une variante **par
plateforme**, via un *classifier* (`aapt2-<version>-osx.jar`,
`-linux.jar`, `-windows.jar` : trois fichiers distincts, pas trois copies
du même). Gradle ne résout que le classifier de l'OS courant, donc
`--write-verification-metadata` n'enregistre jamais que le checksum de
**cette** plateforme.
**Pourquoi dans dengon :** la PR #56 a régénéré `verification-metadata.xml`
sur macOS (voir la note ci-dessus sur le cache chaud) — ça a bien corrigé
l'entrée `osx`, mais a laissé le fichier sans checksum `linux`, invisible
tant que personne ne clone sur Linux ou en CI. Découvert en pratique par
Paul (poste Linux) puis recorrigé dans la PR #72.
**Piège / surprise :** la procédure « vider le cache + régénérer »
(apprentissage précédent) **ne suffit pas** à elle seule pour ce cas — elle
corrige le cache chaud, pas l'absence structurelle des autres plateformes.
Le seul moyen de couvrir un classifier qu'on n'a pas la machine pour
résoudre soi-même : télécharger le jar officiel depuis
`dl.google.com/android/maven2/...` et calculer `sha256sum` à la main.
Reviendra identiquement à chaque montée de version d'AGP tant que
`aapt2` (ou tout autre artefact à classifier) change de version.
**Où c'est utilisé :** `android/gradle/verification-metadata.xml`, entrées
`com.android.tools.build:aapt2`.
**Pour aller plus loin :** `docs/suivi/modules/android-app.md`, section
« Décisions d'implémentation » (puce `aapt2`) ; la section « Trois pièges
rencontrés » (onboarding) couvre le piège voisin du cache chaud.

---

### `$` en regex Python matche avant un `\n` final — piège pour un motif JSON Schema

**C'est quoi :** en Python (`re`, sans `re.MULTILINE`), `$` matche soit la fin
absolue de la chaîne, soit la position juste avant un unique `\n` final. Un
motif `^[0-9a-f]{16}$` accepte donc une chaîne de **17** caractères si le
17ᵉ est `\n`. Ce n'est pas le comportement d'ECMA 262 (JavaScript), la norme
visée par le mot-clé `pattern` de JSON Schema — donc un validateur JS serait
strict là où le validateur Python (`jsonschema`, qui utilise `re` en
interne) ne l'est pas.
**Pourquoi dans dengon :** relevé en **relecture approfondie de la revue de
la PR #60** — `"<16 hex>\n"` passait `HEX16`/`HEX32`/`HEX64` dans
`contracts/tools/catalogue.py`. `\Z` (extension Python, pas de `\n` de
tolérance) aurait corrigé le symptôme, mais ces fragments finissent dans
`payloads.schema.json`, censé rester neutre en langage — y introduire une
extension Python irait contre l'objectif même de `contracts/`.
**Piège / surprise :** la correction n'est pas `\Z` mais `minLength`/
`maxLength` en plus du `pattern` — un mot-clé JSON Schema standard, qui ferme
le même trou sans dépendre du moteur regex. Seuls les champs de longueur
**fixe** peuvent en profiter ; un motif ouvert (`{6,}` sans borne haute) reste
vulnérable, documenté comme dette assumée dans `03-ecarts-conception.md`.
**Où c'est utilisé :** `contracts/tools/catalogue.py` (`HEX16`/`HEX32`/
`HEX64`), `contracts/events/{envelope,batch}.schema.json` (`event_id`,
`batch_id`, `sig`).

### JSON Schema `required` fait déjà ce qu'une boucle manuelle referait

**C'est quoi :** le mot-clé `required` d'un schéma JSON (`{"required": [...]}`)
vérifie la présence de champs — exactement ce qu'une boucle `for field in
required: if field not in payload: ...` referait à côté, en double.
**Pourquoi dans dengon :** `contracts/tools/validate.py` construisait un
`Draft202012Validator` **sans** `required` (seulement `properties`), et
compensait par une boucle manuelle juste avant — repéré en revue de la
PR #60. Ajouter `required` au schéma du validateur a permis de supprimer la
boucle : une seule vérification, native, au lieu de deux qui doivent rester
synchronisées.
**Où c'est utilisé :** `contracts/tools/validate.py::_payload_validator`.
### `BLE_UUID128_INIT` attend du little-endian — un UUID inversé compile très bien

**C'est quoi :** NimBLE range les UUID 128 bits dans un tableau de 16 octets
`ble_uuid128_t.value[]`, stocké **à l'envers de la forme textuelle**. Sa propre
fonction `ble_uuid_to_str()` le prouve : elle réimprime `value[15]`, puis
`value[14]`, jusqu'à `value[0]`. Donc pour écrire `6d656e67-…-0000` dans un
`BLE_UUID128_INIT(...)`, on saisit les octets **de droite à gauche**.

**Pourquoi dans dengon :** les trois UUID du service `dengon` sont figés par la
décision C-2 et doivent correspondre **exactement** entre le firmware, l'app
Android et `dengon-core`. Un octet dans le mauvais sens et les deux moitiés du
maillage ne se voient tout simplement pas.

**Piège / surprise :** l'erreur est **totalement silencieuse**. Recopier la
chaîne de gauche à droite compile, link, boote et annonce — un UUID inversé que
seul un scanner BLE révèle, et encore, à condition de le lire caractère par
caractère. Aucun outil de la chaîne ne peut aider : pour le compilateur, ce sont
seize octets valides. Deux garde-fous retenus : (1) un contrôle visuel — le
**dernier** octet de chaque tableau vaut `0x6d`, le `m` de `meng`, et le
**premier** est le discriminant `0x00`/`0x01`/`0x02` ; (2) le callback
`gatts_register_cb` imprime au démarrage les UUID tels que NimBLE les a
réellement enregistrés, ce qui déplace la vérification du scanner vers le
moniteur série. Détail amusant : ces UUID ne sont pas aléatoires, ce sont
14 octets d'ASCII (`meng-dengon-v1`) suivis de 2 octets de discriminant — ce qui
rend l'inversion lisible à l'œil nu une fois qu'on le sait.

**Où c'est utilisé :** `firmware/dengon-relay/main/dengon_gatt.c`, les trois
constantes en tête de fichier.

**Pour aller plus loin :** `mynewt-nimble/nimble/host/src/ble_uuid.c`, fonction
`ble_uuid_to_str()`.

---

### Un paquet d'annonce BLE tient dans 31 octets, et le dépassement se voit à l'exécution

**C'est quoi :** une annonce BLE 4.x transporte au plus **31 octets** de données,
découpés en champs `longueur ‖ type ‖ valeur` — donc 2 octets de surcoût par
champ. Un second paquet de 31 octets, la **réponse de scan**, est envoyé à la
demande : un scanner qui voit une annonce `ADV_IND` émet un `SCAN_REQ` et reçoit
ce complément.

**Pourquoi dans dengon :** ce que la conception impose sature déjà le budget —
Flags (3) + liste complète d'UUID 128 bits (18) + manufacturer data (9) =
**30 octets sur 31**. Le nom de l'appareil ne tenait pas : il est parti en
réponse de scan. L'UUID de service, lui, devait impérativement rester dans le
paquet principal, puisque c'est sur lui que les pairs filtrent leur scan.

**Piège / surprise :** le dépassement n'est **pas** détecté à la compilation.
`ble_gap_adv_set_fields()` renvoie `BLE_HS_EMSGSIZE` (`0x0C`) au démarrage, et
si l'on ne teste pas son code de retour, la carte se contente de ne jamais
annoncer — sans le moindre message. L'exemple officiel `bleprph` met un
`tx_pwr_lvl` dans le paquet principal : ces 3 octets sont exactement ceux qui
nous manquaient. L'ESP32 étant en Bluetooth 4.2, l'*extended advertising* de
BLE 5 (jusqu'à 255 octets) n'est pas une porte de sortie.

**Où c'est utilisé :** `firmware/dengon-relay/main/main.c`, fonction
`dengon_advertise()`.

---

### `sdkconfig.defaults` n'est lu que si `sdkconfig` n'existe pas

**C'est quoi :** un projet ESP-IDF a deux fichiers de configuration.
`sdkconfig.defaults` est écrit à la main et versionné ; `sdkconfig` est
**généré** au premier build à partir des defaults, puis modifié par
`idf.py menuconfig`. C'est `sdkconfig` que lit la compilation.

**Pourquoi dans dengon :** toute la configuration qui fait foi — host NimBLE
plutôt que Bluedroid, rôles BLE, MTU préféré, table de partitions — vit dans
`sdkconfig.defaults`, le seul des deux qui soit versionné.

**Piège / surprise :** la génération n'a lieu **qu'une fois**. Corriger une
ligne de `sdkconfig.defaults` après un premier build ne produit **aucun effet**,
et surtout **aucun avertissement** : on relit son fichier dix fois en cherchant
la faute de frappe. Remède : `idf.py fullclean`, ou supprimer `sdkconfig`. Le
corollaire est plus vicieux : un réglage fait en `menuconfig` atterrit dans
`sdkconfig`, qui est dans le `.gitignore` — il marche sur votre machine, et
nulle part ailleurs, CI comprise.

**Où c'est utilisé :** `firmware/dengon-relay/sdkconfig.defaults` (l'avertissement
est en tête du fichier), rappelé dans `docs/suivi/modules/firmware-relay.md`.

---

### L'ordre d'itération d'un `HashMap` Rust change à chaque exécution

**C'est quoi :** `std::collections::HashMap` utilise par défaut un hachage
(SipHash) initialisé avec une clé **aléatoire tirée au démarrage du
processus**, pour résister aux attaques par collisions. Conséquence : deux
exécutions du même programme parcourent le même `HashMap` dans un ordre
différent.

**Pourquoi dans dengon :** le simulateur (`dengon-sim`, US-221) doit être
**rejouable** : même graine → même trace. Un seul `for (k, v) in &hashmap`
qui envoie des trames suffirait à rendre l'ordre des émissions, donc les
tirages de perte et de gigue, différent d'une exécution à l'autre.

**Piège / surprise :** le bug ne se voit pas dans un test unitaire qui
compare deux exécutions **dans le même processus** (même clé des deux côtés).
D'où le job CI `sim`, qui lance le **binaire** deux fois. Dans le simulateur :
`BTreeMap`/`BTreeSet` partout.

**Où c'est utilisé :** `crates/dengon-sim/src/reseau.rs`, `harness.rs` ;
`.github/workflows/sim.yml`.
### `isReturnDefaultValues = true` fait taire les API Android en test, pas les exécuter

**C'est quoi :** sans Robolectric, un test JVM pur ne peut pas exécuter le
vrai code du framework Android (`android.jar` fourni au classpath de test est
un bouchon dont chaque méthode lève par défaut). `unitTests
.isReturnDefaultValues = true` (`app/build.gradle.kts`) remplace ce lever
d'exception par un **retour silencieux** de la valeur par défaut du type de
retour (`null` pour un objet, `0`/`false` pour un primitif).
**Pourquoi dans dengon :** en écrivant le bouchon `DengonIdentity` (US-106),
la première version du QR code utilisait `android.util.Base64.encodeToString`.
Un test d'aller-retour (encoder puis décoder) aurait **passé silencieusement**
en comparant deux valeurs dérivées de `null` — ou aurait produit un NPE
confus, selon l'endroit — sans jamais exercer le vrai algorithme.
**Piège / surprise :** ce n'est pas un échec bruyant : `isReturnDefaultValues`
existe justement pour qu'un code qui *appelle accidentellement* une API
Android ne fasse pas planter tous les tests JVM purs du projet. Ça veut dire
qu'un test peut être vert **pour la mauvaise raison** — il faut se demander,
pour chaque appel à une classe `android.*` dans du code testé en JVM pur, si
le test l'exerce réellement ou observe juste sa valeur par défaut.
**Parade :** n'utiliser des API `android.*` que dans du code qui ne sera
testé qu'en instrumenté/Robolectric ; sinon, écrire l'équivalent en Kotlin/
Java pur (ici : un encodeur/décodeur base64url à la main).
**Où c'est utilisé :** `android/.../ffi/DengonNodeStub.kt` (repéré avant
d'écrire le test, pas après un échec silencieux) ; réglage source :
`android/app/build.gradle.kts`.

### UniFFI en mode UDL : le contrat vit dans un fichier séparé, pas dans les macros

**C'est quoi :** UniFFI a deux façons de décrire une interface FFI : des
macros procédurales (`#[uniffi::export]` directement sur le code Rust) ou un
fichier `.udl` séparé, lu par `build.rs`
(`uniffi::generate_scaffolding("src/x.udl")`) puis inclus dans `lib.rs`
(`uniffi::include_scaffolding!("x")`). Le `.udl` déclare les types
(`dictionary`, `enum`, `[Enum] interface` pour un enum à données associées,
`[Error] enum`, `interface` pour un objet avec état) ; le Rust doit fournir
des types du **même nom**, avec les **mêmes champs**, mais reste du Rust
ordinaire (pas d'attribut spécial dessus).
**Pourquoi dans dengon :** la DoR de l'US-106 impose explicitement un fichier
`.udl` (pas les macros) — c'est un contrat qu'on veut pouvoir lire et geler
sans lire le code Rust qui l'implémente.
**Piège / surprise :** le scaffolding généré ne produit **que** le pont côté
Rust (fonctions `extern "C"`) — pas les classes Kotlin. Ça, c'est une
commande séparée (`uniffi-bindgen generate`, avec la feature `"bindgen"`/`
"cli"`), volontairement pas activée ici : l'US-106 ne demande qu'un bouchon
Kotlin écrit à la main, pas une génération réelle (ça viendra avec l'US-302).
**Où c'est utilisé :** `crates/dengon-ffi/src/dengon.udl`, `build.rs`,
`src/lib.rs`.
**Pour aller plus loin :** doc officielle UniFFI, section « UDL » vs
« Procedural macros ».

---

### UniFFI 0.28 : la forme Kotlin générée ne se devine pas depuis le `.udl`

**C'est quoi :** UniFFI traduit le `.udl` en Kotlin avec des choix qui ne
sont pas évidents : `sequence<u8>` devient `List<UByte>` (et non
`ByteArray`, réservé au type `bytes`), `u64`/`u32` deviennent
`ULong`/`UInt`, une `interface X` devient `open class X` **plus**
`interface XInterface`, un `[Error] enum` devient une exception **scellée**
avec une sous-classe par variante, les fonctions d'un `namespace` sont de
premier niveau, et un `dictionary` devient une `data class` à champs `var`.

**Pourquoi dans dengon :** l'UI Android est écrite contre un bouchon Kotlin
écrit à la main (US-106), censé être remplacé par les bindings générés à
l'US-302 sans casser l'UI. Écrit « de tête », il divergeait sur six points
(revue PR #69).

**Piège / surprise :** une `data class` avec des champs `ByteArray` n'a
**pas** d'égalité par contenu (tableaux comparés par référence). Le bouchon
qui corrigeait ça « proprement » créait une égalité que le code généré ne
fournira pas. Seul remède fiable : **générer** les bindings de référence et
compiler le code client contre eux.

**Où c'est utilisé :** `crates/dengon-ffi/src/dengon.udl`,
`crates/dengon-ffi/uniffi.toml`, `android/.../ffi/DengonTypes.kt`.


---

### `verify_strict` : décoder une clé publique ne suffit pas à la juger

**Contexte :** US-203, signature Ed25519 avec `ed25519-dalek` 2.x.

Ed25519 accepte des clés « de faible ordre » (comme le point neutre, `y = 1`) qui
se décodent sans erreur. Avec certaines de ces clés, on peut fabriquer des
signatures qui passent la vérification standard sans connaître de secret. Le mode
`verify_strict` les refuse, et refuse aussi les signatures malléables (deux
encodages valides pour un même message).

Piège rencontré : on croit que `VerifyingKey::from_bytes` filtre les mauvaises
clés. Faux — j'ai supposé que `0xFF…FF` serait rejeté, et une sonde a montré que
dalek le décode. Ce qui protège, c'est la vérification stricte, pas le décodage.

Deuxième leçon : un vecteur de test (KAT) copié de mémoire n'est pas une preuve.
Les valeurs du RFC 8032 ont été recalculées avec une seconde implémentation
(Python `cryptography` / OpenSSL) avant d'être figées dans les tests, et on a
vérifié qu'altérer un octet fait bien échouer les tests.

**Où c'est utilisé :** `crates/dengon-core/src/crypto.rs`
(`VerifyingKey::verify`, tests `cle_de_faible_ordre_*` et `kat_rfc8032_*`).
### `core::error::Error` existe en `no_std` depuis Rust 1.81

**C'est quoi :** le trait `Error` a longtemps vécu seulement dans `std`
(`std::error::Error`). Depuis Rust 1.81 il est aussi dans `core`, et
`std::error::Error` n'en est plus qu'un ré-export.

**Pourquoi dans dengon :** `dengon-core` doit compiler en `no_std` pour
l'ESP32. Les erreurs du codec (`DecodeError`, `EncodeError`, `AppDecodeError`)
implémentent `core::error::Error` : elles restent utilisables avec `?` et
`Box<dyn Error>` côté Android/CLI, sans feature `std` conditionnelle.

**Piège / surprise :** beaucoup d'exemples en ligne mettent encore
`#[cfg(feature = "std")] impl std::error::Error for …` — c'est inutile avec
notre MSRV (1.85) et ça crée deux variantes de l'API selon la feature.

**Où c'est utilisé :** `crates/dengon-core/src/protocol/codec/mod.rs`,
`codec/app.rs`.

### `snow` sans `getrandom` : fournir soi-même l'aléa via un `CryptoResolver`

**C'est quoi :** `snow` obtient ses primitives (DH, hash, AEAD, RNG) par un
*resolver*. Compilé sans `use-getrandom` (indispensable pour xtensa, Spike A),
son `DefaultResolver` n'a plus de RNG : `resolve_rng()` renvoie `None` et tout
`build_initiator()` échoue **au runtime**, pas à la compilation.
**Pourquoi dans dengon :** `crypto::rng::CallerResolver` délègue tout au
`DefaultResolver` sauf le RNG, pris dans un `RngCore + CryptoRng` passé par
l'appelant (`OsRng` sur hôte, `esp_fill_random` sur ESP32, `ChaCha20Rng` graine
fixe dans les tests). `snow` n'appelle `resolve_rng()` qu'**une fois** par état
construit, donc le resolver le cède par `RefCell<Option<…>>::take()`.
**Piège / surprise :** même le répondeur Noise `X`, qui ne tire aucune clé
éphémère, exige un RNG pour se construire → `NoRng`, qui échoue toujours.
Bonus : un RNG déterministe rend tout le transcript Noise reproductible, ce qui
permet des vecteurs de conformité. On a vérifié que l'éphémère = les 32 premiers
octets du RNG (`e.pub` en tête du message 1).
**Où c'est utilisé :** `crates/dengon-core/src/crypto/rng.rs`,
`crates/dengon-core/src/crypto/noise.rs` (`builder`, `open`).
**Pour aller plus loin :** `snow/src/builder.rs` (`Builder::build`), Noise spec rev. 34 §5.

### PKCS#7 ne sait pas bourrer plus de 255 octets

**C'est quoi :** PKCS#7 écrit N fois l'octet N ; N tient sur un octet, donc le
bourrage est limité à 255 octets. Conçu pour des blocs de chiffrement (16 o),
pas pour des *buckets* de plusieurs centaines d'octets.
**Pourquoi dans dengon :** la conception demandait PKCS#7 vers
`[256, 512, 1024, 2048]` : impossible au-delà de 512. Remplacé par un préfixe
de longueur `u16` + zéros (voir `03-ecarts-conception.md`).
**Piège / surprise :** le défaut n'apparaît qu'aux gros buckets ; un test sur
des messages courts serait passé.
**Où c'est utilisé :** `crates/dengon-core/src/crypto/pad.rs`.

### Recouper un transcript Noise sans bibliothèque Noise

**C'est quoi :** un handshake Noise n'est qu'une suite de `MixHash` (SHA-256),
`MixKey` (HKDF-HMAC-SHA256) et `EncryptAndHash` (ChaCha20-Poly1305, nonce =
4 zéros ‖ compteur u64 **little-endian**, AD = `h`). Avec des éphémères fixés, on
peut le réimplémenter en ~30 lignes de Python (`cryptography`).
**Pourquoi dans dengon :** l'enveloppe Noise `X` des vecteurs
(`tests/vectors/crypto_v0.json`) a été recalculée ainsi, identique octet par
octet à `snow` — preuve indépendante que les vecteurs sont du vrai Noise.
**Piège / surprise :** le nom de protocole (`Noise_X_25519_ChaChaPoly_SHA256`,
31 o) fait ≤ 32 o : `h` initial = nom **complété de zéros**, pas son hash.
**Où c'est utilisé :** vérification ponctuelle (script non versionné, voir journal).

### Transport Noise sur un lien non fiable : nonce explicite + fenêtre anti-rejeu

**C'est quoi :** en mode transport, Noise chiffre chaque message avec un
compteur implicite (0, 1, 2…) que les deux côtés incrémentent. Si un message
se perd ou arrive dans le désordre, le récepteur essaie le mauvais nonce et
**tout** échoue ensuite. Sur UDP, WireGuard et DTLS envoient donc le compteur en
clair avec le message et tiennent une **fenêtre glissante** (bitmap de 64 bits,
RFC 6479) des nonces déjà vus pour refuser les rejeux.
**Pourquoi dans dengon :** les `NOISE_MSG` sont relayés à travers le maillage ;
pertes et désordre sont la norme. `snow` fournit `StatelessTransportState`
(nonce passé en argument) ; la fenêtre est à écrire soi-même.
**Piège / surprise :** (1) les tests « tout arrive dans l'ordre » passaient
avec le transport standard : le bug n'est apparu qu'à la revue, en simulant une
perte. (2) Ne mettre la fenêtre à jour **qu'après** authentification, sinon un
attaquant envoie un nonce énorme forgé et fait rejeter tous les messages
légitimes (test `xx_nonce_forge_ne_fait_pas_avancer_la_fenetre`).
**Où c'est utilisé :** `crates/dengon-core/src/crypto/noise.rs` (`Session`,
`ReplayWindow`).
**Pour aller plus loin :** RFC 6479 ; WireGuard whitepaper §5.4.6.

---

### Garanties message par message d'un handshake Noise `XX` (§7.7 de la spec)

**C'est quoi :** dans `XX` (`-> e` / `<- e, ee, s, es` / `-> s, se`), chaque
message n'a pas les mêmes garanties. Le message 1 est émis **avant tout DH** :
son payload part en clair. Le message 2 est chiffré, mais vers un initiateur
qu'on n'a pas encore authentifié. Seuls le message 3 et le transport ont les
garanties complètes (confidentialité + authentification mutuelle).
**Pourquoi dans dengon :** `sync` sera tenté de glisser des métadonnées
(version, capacités, inventaire) dans le handshake. `Handshake::write_message`
refuse donc tout payload au message 1 (`PayloadNotAllowed`).
**Piège / surprise :** padder le message 1 donnait l'illusion d'une
protection : un `b"SECRETPAYLOAD"` se retrouvait tel quel dans les 288 octets
émis (constaté en revue de #81). Et comme les tailles de handshake sont fixées
par le motif quand les payloads sont vides, le padding ne cachait rien : il
coûtait 768 octets par handshake.
**Où c'est utilisé :** `crates/dengon-core/src/crypto/noise.rs`, doc de `Handshake`.
**Pour aller plus loin :** <https://noiseprotocol.org/noise.html#payload-security-properties>.

---

### Les clés de transport Noise ne dépendent pas des payloads de handshake

**C'est quoi :** `Split()` dérive les clés de transport de la *chaining key*
`ck`, qui n'est mise à jour que par les DH (`MixKey`). Les payloads de handshake
n'entrent que dans le *handshake hash* `h` (`MixHash`).
**Pourquoi dans dengon :** en retirant le padding du handshake (revue #81), seuls
les trois messages de handshake de `crypto_v0.json` ont changé ; les chiffrés de
transport sont restés identiques octet par octet.
**Piège / surprise :** on s'attendait à devoir régénérer tout le transcript.
**Où c'est utilisé :** `crates/dengon-core/tests/vectors/crypto_v0.json`.
**Pour aller plus loin :** spec Noise §5.2 (`MixKey`, `MixHash`, `Split`).
### Biais du modulo et le piège `u16 % 100000` (US-205)

**C'est quoi :** pour tirer un nombre dans `0..N` depuis des octets
aléatoires, on calcule `v % N`. Si `v` couvre `0..M` et que `M` n'est pas un
multiple de `N`, les petites valeurs sortent un peu plus souvent : c'est le
biais du modulo, d'autant plus négligeable que `M` est grand devant `N`. Le
cas extrême est `M < N` : le modulo ne fait rien, et les valeurs `M..N` ne
sortent jamais.
**Pourquoi dans dengon :** la formule de conception du code de vérification
prenait 2 octets (`M = 65 536`) pour un groupe de 5 chiffres (`N = 100 000`).
Aucun groupe au-delà de 65535 n'était possible. On prend 5 octets
(`M = 2⁴⁰`), comme Signal.
**Piège / surprise :** le bug ne se voit pas dans un test d'égalité A = B.
Il se voit en regardant la **distribution** : aucun groupe ne commence par
7, 8 ou 9. Le test `toujours_60_chiffres` vérifie la forme ; la formule
elle-même est recoupée en Python.
**Où c'est utilisé :** `crates/dengon-core/src/identity/safety.rs:80`.
**Pour aller plus loin :** Signal, *Safety number updates* (2016) ;
libsignal `displayable_fingerprint` (chunks de 5 octets, `% 100000`).

---

### Écriture atomique d'un fichier : `rename` ne suffit pas (US-205)

**C'est quoi :** écrire dans `x.tmp`, `fsync`, puis `rename(x.tmp, x)` garantit
qu'un lecteur voit l'ancien ou le nouveau contenu, jamais un mélange. Mais le
renommage est une modification **du répertoire** : tant que le répertoire
n'est pas lui-même synchronisé (`File::open(dir)?.sync_all()`), une coupure
de courant peut l'annuler.
**Pourquoi dans dengon :** le coffre d'identité (`FileVault`) ; perdre le
renommage ne corrompt rien (l'ancien coffre reste), mais la nouvelle écriture
serait perdue.
**Piège / surprise :** `OpenOptions::mode(0o600)` ne s'applique qu'à la
**création**. Un `.tmp` laissé par un crash, avec d'autres droits, les
garde après `truncate`. D'où : supprimer le `.tmp`, puis `create_new`.
Autre piège, côté tests : sous Windows, `temp_dir()` finit par `\` et
`fs::read` sur ce chemin rend `NotFound`, pas une erreur « c'est un
répertoire ».
**Où c'est utilisé :** `crates/dengon-core/src/identity/vault.rs`
(`FileVault::save`).
**Pour aller plus loin :** `man 2 rename`, `man 2 fsync` (section sur les
répertoires).

### AAD : authentifier l'en-tête d'un blob chiffré sans le chiffrer (US-205)

**C'est quoi :** un AEAD (ChaCha20-Poly1305, XChaCha20-Poly1305) prend en
entrée, en plus du clair, des *associated data* (AAD). Elles ne sont pas
chiffrées mais sont couvertes par le tag : modifier un seul de leurs octets
fait échouer le déchiffrement.
**Pourquoi dans dengon :** l'en-tête du coffre `"DGID" ‖ version` est lisible,
pour que le code choisisse le bon format avant de déchiffrer. Il est passé en
AAD : un attaquant ne peut donc pas faire passer un blob v1 pour un blob v2
(attaque par rétrogradation), ni l'inverse.
**Piège / surprise :** « l'en-tête n'est pas secret » ne veut pas dire « il
n'a pas besoin d'être protégé ». Sans AAD, la version serait modifiable
librement. Le test `octet_altere_refuse` modifie un bit au hasard dans tout
le blob, en-tête compris, et vérifie le refus.
**Où c'est utilisé :** `crates/dengon-core/src/identity/vault.rs:74`.
**Pour aller plus loin :** RFC 8439 §2.8 ; draft-irtf-cfrg-xchacha
(nonce de 24 octets, sûr en tirage aléatoire).

---

### TLS sur une IP littérale : le client n'envoie pas de SNI (US-224)

**C'est quoi :** SNI (*Server Name Indication*) est l'extension TLS par
laquelle un client indique, en clair, quel nom d'hôte il cherche à joindre
— c'est ce qui permet à un serveur de choisir le bon certificat quand
plusieurs sites partagent la même IP/le même port. La RFC 6066 ne définit
SNI que pour des **noms d'hôte** ; un client qui se connecte à une IP
littérale (`https://51.255.38.214:8443`) n'a, par construction, aucun nom
à y mettre, et n'envoie donc **aucune** extension SNI.
**Pourquoi dans dengon :** le VPS de démo (US-224) n'a pas de nom de
domaine, seulement une IP. Le reverse-proxy Caddy sélectionne pourtant son
certificat par SNI (`tls_connection_policies` matchées par nom d'hôte) —
sans nom envoyé par le client, Caddy ne trouve aucune politique
correspondante et refuse la poignée de main (`tlsv1 alert internal error`).
**Piège / surprise :** ça marchait en local avec `https://localhost:8443`
(un nom, donc du SNI est envoyé) et cassait uniquement en pointant vers
l'IP publique du VPS — le symptôme ne dépendait donc pas du réseau
(local vs. Internet) mais du **type d'adresse** utilisé pour se connecter,
ce qui n'était pas évident au premier abord. Le diagnostic décisif :
`openssl s_client -connect <ip>:8443 -servername <ip>` réussissait (il
permet de forcer une SNI arbitraire, y compris une IP, ce qu'un vrai client
ne ferait jamais), alors que `curl https://<ip>:8443/...` échouait sur la
même configuration serveur — la différence entre les deux commandes EST le
diagnostic. La solution est l'option `default_sni` de Caddy : un nom de
repli utilisé quand la connexion n'en fournit aucun.
**Où c'est utilisé :** `dashboard/deploy/Caddyfile`.
**Pour aller plus loin :** RFC 6066 §3 (SNI) ; documentation Caddy sur
`default_sni` et `tls_connection_policies`.

---

### Routeur « sans I/O » (*sans-IO*) : l'heure et l'aléa en arguments

**C'est quoi :** écrire une logique réseau comme une machine à états pure :
elle reçoit des événements (« paquet reçu à t »), rend des décisions
(« relayer à t + 57 ms »), et n'appelle jamais elle-même la radio, l'horloge
ou un générateur aléatoire système.
**Pourquoi dans dengon :** le même routeur doit tourner sur Android, sur PC
et sur l'ESP32 (`no_std`), et être testé sans radio. Passer `now_ms` en
argument et la graine au constructeur rend chaque test **rejouable** à
l'identique — c'est un critère d'acceptation de l'US-209.
**Piège / surprise :** le « délai aléatoire avant relais » ne peut pas être
un `sleep`. Il faut le couper en deux : `on_packet` *programme* le relais,
`poll_due(now)` le rend quand il est échu. C'est aussi ce qui permet
d'annuler un relais si des doublons arrivent entre-temps.
**Où c'est utilisé :** `crates/dengon-core/src/sync/routing.rs:337` et `:364`.
**Pour aller plus loin :** <https://sans-io.readthedocs.io/>

---

### Tempête de diffusion : pourquoi « abandonner au premier doublon » peut empêcher la livraison

**C'est quoi :** en flood, chaque nœud rediffuse tout ce qu'il reçoit ; en
zone dense, ça sature (*broadcast storm*). Parade classique : attendre un
délai aléatoire et renoncer si on entend assez de voisins rediffuser le même
paquet (schéma « à compteur »).
**Pourquoi dans dengon :** c'est le « écouter avant de rediffuser » de
`synthese/05` §6.1.
**Piège / surprise :** avec un seuil de **1** doublon, un nœud qui entend le
paquet par deux chemins renonce — même si c'est le seul à pouvoir servir un
voisin plus loin. Mesuré sur un losange A→{B,C}→D→E : E ne reçoit rien.
Seuil passé à 2 (écart consigné).
**Où c'est utilisé :** `crates/dengon-core/src/sync/routing.rs:87`,
`crates/dengon-core/tests/routing_mock.rs` (`seuil_litteral_affame_le_losange`).
**Pour aller plus loin :** Ni, Tseng, Chen, Sheu, *The broadcast storm
problem in a mobile ad hoc network*, MobiCom 1999.

---

### Cargo accepte un cycle de dépendances s'il ne passe que par les dev-dependencies

**C'est quoi :** `dengon-ble` dépend de `dengon-core` ; `dengon-core` peut
malgré tout déclarer `dengon-ble` en `[dev-dependencies]`. Les tests
d'intégration (`tests/*.rs`) sont des crates à part qui lient la même
bibliothèque `dengon-core` que `dengon-ble` : les types sont compatibles.
**Pourquoi dans dengon :** tester `sync::routing` contre le vrai
`MockTransport` sans le recopier.
**Piège / surprise :** ça ne marche **pas** depuis les tests unitaires
(`#[cfg(test)]` dans `src/`) : là, `dengon-core` est recompilé en mode test
et ses types diffèrent de ceux que voit `dengon-ble`. D'où le fichier séparé
`tests/routing_mock.rs`.
**Où c'est utilisé :** `crates/dengon-core/Cargo.toml` (`[dev-dependencies]`).

---

### SplitMix64 : un PRNG de 10 lignes pour du `no_std` déterministe

**C'est quoi :** générateur pseudo-aléatoire à 64 bits d'état (Steele, Lea,
Flood 2014) : une addition d'une constante puis trois mélanges xor-shift /
multiplication. Rapide, bonne qualité statistique, **pas** cryptographique.
**Pourquoi dans dengon :** tirer le jitter de relais sans ajouter la crate
`rand` à `dengon-core` (cible ESP32) et en restant reproductible à graine
fixe.
**Piège / surprise :** vérifier l'implémentation contre les sorties de
référence (graine 0 → `0xE220A8397B1DCDAF`), sinon une faute de frappe dans
une constante passe inaperçue.
**Où c'est utilisé :** `crates/dengon-core/src/sync/routing.rs:616`.

---

### `starlette.testclient.TestClient` ne streame pas vraiment (US-218)

**C'est quoi :** `TestClient` (utilisé par `fixture client` dans
`conftest.py`) exécute la coroutine ASGI de l'app **jusqu'à sa fin complète**
avant de rendre la main à l'appelant — y compris pour une réponse en
streaming. Dans `starlette/testclient.py`, `handle_request()` fait
`portal.call(self.app, scope, receive, send)`, où `send()` accumule chaque
morceau du corps dans un `io.BytesIO()` ; `portal.call` ne revient que quand
cette coroutine se termine.
**Pourquoi dans dengon :** `GET /api/stream` (SSE) ne se termine **jamais**
tant que le client ne se déconnecte pas (boucle `while True` avec
heartbeat). Un test écrit avec `client.stream("GET", "/api/stream")` reste
donc bloqué indéfiniment dès `__enter__` — avant même d'avoir lu un octet.
**Piège / surprise :** ça ne lève aucune erreur, ne timeout pas, ne produit
aucun message — juste un hang silencieux. Le diagnostic a demandé un script
autonome avec un thread « chien de garde » (`faulthandler.dump_traceback()`
après N secondes) pour voir que le thread de la boucle asyncio interne
était idle en `select()`, preuve qu'il attendait le prochain événement
plutôt que d'être bloqué dans une boucle infinie côté app — le blocage
était bien côté `TestClient`, pas côté route.
**La solution :** un vrai serveur `uvicorn.Server` lancé dans un thread
(port choisi par l'OS, `port=0`), avec un `httpx.Client` réel dessus — un
vrai socket TCP lit les octets progressivement dès qu'ils arrivent, sans
attendre la fin de la réponse. Toujours dans le même process que le test :
l'objet `app` (et donc `app.state.broadcaster`) reste directement
inspectable pour synchroniser le test sans `sleep` fixe (poll borné sur
`subscriber_count()`).
**Où c'est utilisé :** `dashboard/api/tests/test_stream.py`
(fixture `live_server`).
### Réconciliation et anti-inondation se marchent dessus

**C'est quoi :** deux règles saines isolément — « à la rencontre, pousse
tout ce qui manque » et « n'accepte pas plus de N nouveaux messages par
minute d'un même voisin » — qui, combinées, font jeter par le receveur ce
que l'émetteur vient d'envoyer.
**Pourquoi dans dengon :** un relais ESP32 qui a stocké 120 paquets et
rencontre un téléphone : sans cadence, 100 seraient rejetés, et la bande
BLE dépensée pour rien.
**Piège / surprise :** la perte est silencieuse — la rencontre « réussit »
avec 29 paquets sur 35 ; seul le test témoin (même scène, sans cadence)
la chiffre (6 `FloodLimited`). La cadence
d'émission doit rester **sous** le quota du receveur, pas égale : l'`INVENTORY`
lui-même et le trafic direct comptent aussi.
**Où c'est utilisé :** `crates/dengon-core/src/sync/inventory.rs:456`
(`poll_push`), `tests/inventory_mock.rs`.
### Génération (epoch) : désambiguïser deux connexions successives à la même identité

**C'est quoi :** quand une identité stable (ici une adresse BLE) peut être
réutilisée par deux connexions physiques différentes dans le temps
(déconnexion puis reconnexion immédiate), un code qui la résout **sous
verrou** à un instant T puis agit dessus **hors verrou** un peu plus tard
peut agir sur la mauvaise connexion sans qu'aucune structure de données ne
s'en aperçoive — l'identité seule ne suffit pas à détecter le changement.
Parade : associer à l'identité un compteur monotone assigné à la création de
chaque connexion (« génération »/« epoch »), inclus dans l'égalité de la
valeur transportée ; toute opération résolue avant le changement échoue
proprement au lieu de viser la nouvelle connexion.
**Pourquoi dans dengon :** `AndroidTransport.send()` résout le `RadioPeer`
d'un lien sous son verrou, puis appelle `GattRadio.ecrire()` **hors
verrou** (nécessaire : la radio peut rappeler `deconnecte()` pendant
l'écriture). Entre les deux, une reconnexion rapide à la même adresse MAC
peut se produire côté radio.
**Piège / surprise :** les rappels Android (`BluetoothGattServerCallback`)
ne donnent qu'une adresse, jamais un identifiant de connexion — il faut donc
un point de résolution séparé (« quel est le `RadioPeer` **actuel** pour
cette adresse ? ») pour les rappels entrants, distinct de la génération
figée dans une closure pour le rôle central (chaque `connectGatt` a son
propre callback lié à une connexion précise).
**Où c'est utilisé :** `android/app/src/main/java/com/dengon/app/ble/transport/GattRadio.kt`
(`RadioPeer.generation`, `pairActuel()`), corrigé en revue de la PR #98
(US-213).

### Un petit trait sous `Transport` rend la logique de liens testable sans radio (US-303)

**Ce que c'est :** `btleplug` est asynchrone et demande un adaptateur ; le contrat
`Transport` est synchrone et non bloquant. Plutôt que d'écrire `Transport`
directement sur `btleplug`, on met dessous un trait `CentralRadio` (4 méthodes)
et une implémentation générique `CentralTransport<R>` qui porte tout ce que le
contrat exige (identifiants de lien, quota, ordre des événements, coupure).
**Pourquoi dans dengon :** la suite de conformité peut ainsi tourner en CI sur
une fausse radio ; seule la fine couche de traduction `btleplug` reste sans test
automatique.
**Piège / surprise :** la suite lit l'événement `PeerConnected` avec `poll` *après*
`connecter_un_pair` : le banc ne doit pas consommer l'événement lui-même. Il
s'appuie sur le fait que le premier `LinkId` attribué vaut 0 (compteur monotone).
**Où c'est utilisé :** `crates/dengon-ble/src/central.rs`,
`crates/dengon-ble/tests/conformite_central.rs`.

### Windows refuse de supprimer une base SQLite encore ouverte (US-303)

**Ce que c'est :** `remove_dir_all` échoue (erreur 32) tant qu'une connexion
SQLite vit dans le même processus ; Linux l'aurait accepté.
**Pourquoi dans dengon :** le test `etat::tests::le_peer_id_survit_a_un_redemarrage`
doit `drop` le nœud (donc son `Store`) avant de nettoyer le dossier.
**Où c'est utilisé :** `crates/dengon-node/src/etat.rs`.

