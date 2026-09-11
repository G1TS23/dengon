# dengon — Sujets à trancher (divergences & points ouverts)

Ce document recense **tout ce qui n'est pas figé** : les choix divergents entre
les trois dossiers de conception, les points ouverts internes à chacun, les
incohérences internes à réconcilier, et les « avis » à valider en équipe.

Le document de contexte [`00-contexte-global.md`](00-contexte-global.md) présente
la conception `powl` comme référence. Ici, on **ne tranche pas à la place de
l'équipe** : chaque fiche donne les options, leurs arguments, et une piste de
résolution.

## Comment lire une fiche

- **Sujet** — la question précise.
- **Sources** — où c'est dans les dossiers (`powl/NN §x`, `olivier/fichier §x`,
  `oswin/NN §x`).
- **Options** — chaque choix possible, qui le défend, ses arguments pour/contre.
- **Piste de résolution** — spike, test terrain, réunion d'équipe, décision d'un
  référent, ou simple alignement de doc.
- **Statut** — `ouvert` / `à confirmer` / `tranché` (avec la référence qui
  tranche).

## Sommaire

- [A. Divergences entre corps de conception (`powl` ↔ `olivier` ↔ `oswin`)](#a-divergences-entre-corps-de-conception)
- [B. Points ouverts internes `powl` (spike Lot 0)](#b-points-ouverts-internes-powl-spike-lot-0)
- [C. Points ouverts internes `olivier`](#c-points-ouverts-internes-olivier)
- [D. Incohérences internes `powl` à réconcilier](#d-incohérences-internes-powl-à-réconcilier)
- [E. « Avis d'Olivier » à valider en équipe](#e-avis-dolivier-à-valider-en-équipe)

---

## A. Divergences entre corps de conception

### A-1. Techno de l'application mobile

- **Sujet** : en quel langage / framework l'app Android est-elle écrite ?
- **Sources** : `powl/01 §2`, `powl/README` ; `olivier/etude-stack §1, §7`,
  `olivier/decisions-v1 §points d'équipe` ; `oswin/07 §6`, `oswin/09 §2`.
- **Options** :
  - **Android/Kotlin natif + Jetpack Compose** (défendu par `powl`). Pour :
    contrôle BLE le plus complet et le plus fiable (`BluetoothGattServer` +
    `BluetoothLeScanner` + `BluetoothLeAdvertiser`, foreground service),
    indispensable pour le double rôle central+peripheral en arrière-plan sur
    Android 14/15 ; API officielle, pas de plugin tiers ; ~70 % du parc mondial ;
    le cœur Rust `dengon-core` via UniFFI porte déjà toute la logique, la partie
    Kotlin reste petite. Contre : code non réutilisable pour iOS ; plus verbeux ;
    l'équipe (débutante) doit apprendre Kotlin + JNI/UniFFI.
  - **Flutter (Dart)** (défendu par `oswin`, envisagé par `olivier`). Pour : un
    seul langage, build `.apk` en une commande, UI rapide, communauté énorme,
    écosystème très documenté pour débutants ; `bluetooth_low_energy` couvre
    central **et** périphérique sur Android/iOS (seule option multiplateforme
    « prête ») ; vise l'iPhone plus tard sans réécrire l'UI. Contre : le BLE
    périphérique dépend d'un plugin tiers **moins éprouvé** que
    `flutter_blue_plus` (central only) ; l'arrière-plan BLE est moins fiable
    qu'en natif ; incompatible avec un cœur Rust partagé sauf via FFI Dart (non
    prévu).
  - **Kotlin natif OU Flutter, décision après un prototype « hello mesh » en
    semaine 1** (défendu par `olivier/etude-stack §7`). Pour : dérisque le point
    le plus dur (le double rôle BLE) par un test réel avant de s'engager ; repli
    Kotlin natif documenté si `bluetooth_low_energy` déçoit. Contre : coûte
    ~1 semaine ; retarde le début de l'app.
- **Piste de résolution** : réunion d'équipe + le **Spike C** de `powl/10 Lot 0`
  (Android `BluetoothGattServer` + scan en foreground service, 2 appareils
  échangent 20 o) = exactement le « hello mesh » de `olivier`. Le résultat du
  spike tranche.
- **Orientation proposée** (2026-09-08, à valider en réunion) : **Kotlin natif +
  Jetpack Compose pour le MVP Android** ; shell **Swift + SwiftUI** ajouté plus
  tard pour iOS ; les deux au-dessus du **même cœur Rust `dengon-core`** via
  UniFFI (bindings Kotlin *et* Swift générés d'une seule source). **Flutter /
  React Native écartés** : un framework d'UI multiplateforme ne mutualise que les
  écrans (≈ 8-10 écrans simples pour une messagerie), **pas** la couche radio
  BLE — le double rôle central + périphérique en arrière-plan reste spécifique à
  chaque OS quel que soit le framework. `bluetooth_low_energy` (seul plugin
  Flutter couvrant les deux rôles) est peu éprouvé sur ce point précis ; en
  pratique il faut quand même écrire un *platform channel* Kotlin (puis Swift)
  pour le rôle périphérique fiable → Flutter fusionne la partie facile, pas la
  partie difficile. React Native est pire (composant d'annonce BLE non maintenu
  depuis ~4 ans, `olivier/etude-stack §1.2`). L'ouverture iOS est préservée par
  le **cœur Rust + le `trait Transport`**, pas par le framework d'UI. Le
  **Spike C** reste le go/no-go.
- **Statut** : `à confirmer`. `powl` a tranché **Kotlin natif** ; l'orientation
  proposée ci-dessus le confirme et écarte Flutter/RN, sous réserve du Spike C et
  d'une validation en réunion. Note : `oswin/07 §1` indique que l'app Android
  (.apk) et Flutter sont « décidés par l'équipe » — à reconfirmer au vu de `powl`.

### A-2. Un seul moteur `dengon-core` ou deux implémentations ?

- **Sujet** : la logique du protocole (circulation, TTL, dédup, gossip, journal)
  est-elle écrite **une fois** (cœur partagé) ou **deux fois** (une par
  plateforme) ?
- **Sources** : `powl/01 §2`, `powl/02 §1-2, §5.1` ; `olivier/etude-stack §3`,
  `olivier/architecture §9`, `olivier/mise-en-commun §4`.
- **Options** :
  - **Un seul cœur Rust `dengon-core`** (défendu par `powl`), partagé par l'app
    (via **UniFFI** → Kotlin), le nœud CLI et le firmware (via lib statique C
    cross-compilée xtensa). Pour : **une seule source de vérité**, un seul audit
    de sécurité, aucun risque de dérive de comportement entre plateformes, tests
    concentrés dans `dengon-core` sans radio ni réseau, `dengon-sim` rejouable.
    Approche éprouvée (Firefox mobile). Contre : outillage à monter (UniFFI,
    cross-compilation xtensa, `cbindgen`) ; courbe d'apprentissage Rust +
    `no_std` ; **lourd pour 3 semaines** ; risque que `dengon-core` ne
    cross-compile pas proprement pour ESP32 (d'où le Spike A).
  - **Deux implémentations** bornées par une spec de trame binaire stricte
    (défendu par `olivier`), le cœur Rust étant reporté en v2. Pour : simple à
    démarrer, pas d'outillage, chaque plateforme dans son langage naturel
    (Kotlin/Dart pour l'app, C/C++ pour l'ESP32) ; réaliste sur 3 semaines pour
    une équipe débutante ; « le format fait foi, pas le code ». Contre : **risque
    de dérive** entre les deux implémentations au fil des évolutions ; double
    effort de test ; la crypto est codée/intégrée deux fois (surface d'erreur
    doublée — cf. leçon Bridgefy).
- **Piste de résolution** : **Spike A** du `powl/10 Lot 0` (`dengon-core`
  (`protocol` + `crypto`) cross-compile-t-il pour xtensa-esp32 ?). Si oui → un
  cœur. Si non partiellement → trait `Crypto` + lib C côté firmware, le reste en
  Rust partagé. Décision d'équipe sur l'ambition (Rust + UniFFI vs deux implés)
  au vu du délai réel.
- **Orientation proposée** (2026-09-08, à valider en réunion) : **adopter le cœur
  Rust unique `dengon-core`**. C'est le seul choix cohérent avec une ouverture
  iOS peu coûteuse — deux implémentations en imposeraient une **troisième** pour
  iOS et tripleraient l'audit crypto. Repli si le **Spike A** échoue : isoler la
  crypto derrière un `trait Crypto` implémenté en C (mbedTLS, déjà dans l'ESP-IDF)
  **côté firmware seulement** ; `protocol`, `sync`, `store` et `ledger` restent
  partagés en Rust. L'outillage (UniFFI, cross-compilation xtensa, `cbindgen`)
  est assumé comme investissement de semaine 1, dérisqué par les Spikes A et C.
- **Statut** : `à confirmer`. `powl` a tranché **un cœur Rust** ; `olivier`
  recommande **deux implés** pour la v1 et le cœur Rust en v2. L'orientation
  proposée ci-dessus suit `powl` (cœur Rust unique, repli C limité au firmware),
  à valider en réunion. C'est la divergence la plus structurante entre les deux
  conceptions.

### A-3. Modèle cryptographique

- **Sujet** : quelles primitives et quel cadre de session pour le chiffrement des
  messages ?
- **Sources** : `powl/01 §3`, `powl/04 §3, §7` ; `olivier/etude-stack §4`,
  `olivier/format-trame §4`, `olivier/protocole §10` ; `oswin/02 §2, §6`,
  `oswin/09 §5`.
- **Options** :
  - **Noise `XX` (session live) + Noise `X` (enveloppes scellées) + Ed25519
    (signature de paquet)** (défendu par `powl`), crates `snow` +
    `ed25519-dalek` + `x25519-dalek` + `chacha20poly1305`. Pour : forward secrecy
    sur la session live ; enveloppes one-shot pour destinataire absent
    (exactement le besoin store-and-forward) ; cadre standard éprouvé (Bitchat,
    WireGuard) ; adressage anonyme par `recipient_tag` tournant ; padding par
    buckets. Contre : `snow` doit cross-compiler pour xtensa (spike) ;
    complexité moyenne ; pas de FS sur les enveloppes (assumé, borné par
    `MSG_TTL_S` = 24 h).
  - **libsodium `crypto_box` (X25519 + chiffrement authentifié) côté téléphone,
    mbedTLS côté ESP32** (défendu par `olivier`). Pour : `crypto_box` = « contenu
    illisible **et** non modifiable » en une API simple ; libsodium dispo
    partout (dont ESP32) ; mbedTLS **déjà présent** dans l'ESP-IDF (accélération
    AES matérielle) ; l'ESP32 ne déchiffre pas, il a surtout besoin de hacher +
    vérifier des signatures. Contre : pas de cadre de session (pas de FS
    « native ») ; deux bibliothèques différentes = risque d'incompatibilité de
    format des deux côtés → nécessite de figer un format d'octets identique dans
    la doc sécurité.
  - **X25519 + AES-256-GCM / ChaCha20-Poly1305 + Ed25519 + Noise, Double Ratchet
    en bonus** (position `oswin`). Pour : trio « standard éprouvé » ; Double
    Ratchet (Signal) mentionné comme montée en gamme si le temps le permet.
    Contre : `oswin` reste au niveau des primitives, sans trancher le cadre exact
    (recoupe largement `powl`).
- **Piste de résolution** : **doc sécurité à écrire** (`securite.md`, référencée
  par `protocole.md` et `format-trame.md`, non encore rédigée — voir C-11) +
  **Spike A** (crypto Rust sur xtensa). La doc sécurité fige les primitives, les
  tailles, l'ordre des octets, la disposition `nonce`/`tag`/`ciphertext`.
- **Orientation proposée** (2026-09-08, à valider en réunion + Spike A) :
  **option 1 — Noise `XX` (session live) + Noise `X` (enveloppes scellées) +
  Ed25519 (signatures)**, implémentée **une seule fois** dans
  `dengon-core::crypto` (crates `snow` + `ed25519-dalek` + `x25519-dalek` +
  `chacha20poly1305` + `sha2`). Motifs :
  - **cohérence avec A-2** : un cœur Rust unique impose une implémentation crypto
    unique et un seul audit. L'option 2 (`crypto_box` côté téléphone + mbedTLS
    côté ESP32) code la crypto **deux fois** dans deux bibliothèques → dérive de
    format + surface d'erreur doublée (leçon Bridgefy), et contredit A-2 ;
  - **répond à l'attendu structurant** (destinataire hors-ligne, cf. réponse à
    « la convergence couvre-t-elle les attendus ») : les enveloppes Noise `X`
    one-shot **sont** le mécanisme store-and-forward ; `crypto_box` le fait aussi
    mais sans cadre pour le chemin live ni forward secrecy ;
  - **forward secrecy gratuite** sur la session live via Noise `XX` (attendu noté
    « bonus v1 » par l'énoncé) ;
  - **cadre spécifié, pas maison** : `Noise_XX_25519_ChaChaPoly_SHA256` est un
    standard → une implé Rust et une implé C conformes **interopèrent par
    construction** ; `recipient_tag` tournant + `PAD_BUCKETS` (métadonnées) déjà
    spécifiés dans `powl/04` ;
  - **primitives = exactement la convergence** (X25519, ChaCha20-Poly1305,
    Ed25519, SHA-256), crates pures Rust éprouvées.

  **Statut du Spike A (2026-09-10, US-101)** : ✅ **exécuté — `snow` cross-compile
  bien pour xtensa** (en version **≥ 0.10.0** ; la 0.9.6 non). L'option 1 est
  donc retenue **sans repli** : une seule implémentation crypto Rust, partagée
  téléphone et ESP32. Voir B-1 et
  [`suivi/spikes/US-101-cross-compile-xtensa.md`](../suivi/spikes/US-101-cross-compile-xtensa.md).
  Le repli ci-dessous est **conservé pour mémoire, non activé**.

  **Repli (NON ACTIVÉ) si le Spike A avait montré que `snow` ne cross-compile pas pour xtensa** :
  - `sha2` + `ed25519-dalek` restent en Rust **partout** (petits crates `no_std`,
    cross-compilation attendue sans souci) → couvrent `msgID`/dédup + vérif et
    signature de paquets + `LOG_ATTEST` du relais ;
  - le **handshake Noise `XX` + l'AEAD de lien BLE** sont isolés derrière un
    `trait Crypto`, implémenté côté ESP32 avec **mbedTLS** (déjà dans l'ESP-IDF :
    X25519, ChaCha20-Poly1305, HKDF-SHA256), au format de fil Noise `XX` à
    l'octet près (figé dans la doc sécurité) ;
  - repli plus radical pour le MVP démo : rendre le Noise `XX` **de lien**
    optionnel (les relais transfèrent les paquets déjà scellés/signés en clair
    sur le saut BLE ; Noise `XX` réservé au chemin live téléphone↔téléphone) →
    l'ESP32 n'a alors besoin que de `sha2` + `ed25519-dalek`.

  **Livrable** : prendre `powl/04-security.md` comme doc sécurité de référence et
  ne rédiger que le delta manquant (voir C-11). Le **Spike A** doit trancher :
  `snow` + `ed25519-dalek` + `sha2` sur `xtensa-esp32-none-elf` (`no_std`) →
  complet / partiel / KO.
- **Statut** : `à confirmer`. `powl` a tranché **Noise XX/X + Ed25519** ;
  `olivier` note « piste : libsodium `crypto_box` » comme choix par défaut à
  détailler en doc sécurité. L'orientation proposée ci-dessus suit `powl`
  (option 1, une implé Rust ; repli `trait Crypto` + mbedTLS limité au lien BLE
  sur ESP32), à valider en réunion et conditionnée au Spike A. Convergence
  acquise sur : X25519 + AEAD moderne + Ed25519, jamais de crypto maison.

### A-4. Carte ESP32 : WROVER ou WROOM ?

- **Sujet** : quel module ESP32 pour les relais ?
- **Sources** : `powl/01 §4.3`, `powl/06 §2` ; `olivier/CONTEXT.md`,
  `olivier/analyse-besoins §4, §6`, `olivier/architecture §3` ; `oswin/07 §2`,
  `oswin/README`.
- **Options** :
  - **ESP32-WROVER(-E)** (4 Mo flash, **8 Mo PSRAM**) (défendu par `powl`). Pour :
    la PSRAM permet des buffers confortables (cache gossip ~512 Ko, enveloppes
    ~1 Mo, réassemblage ~256 Ko) **et** la coexistence BLE + Wi-Fi ; sans PSRAM,
    `powl` divise les caches par ~8 (`ENVELOPE_STORE_MAX` : 512 → 64). Contre :
    ce n'est **pas** la carte que l'équipe possède (`CONTEXT.md` et README
    `oswin` disent **Freenove ESP32-WROOM-32E**) → il faudrait en acheter.
  - **ESP32-WROOM-32E (carte Freenove)** (matériel réel de l'équipe, `oswin` /
    `olivier`). Pour : **déjà en possession** ; double cœur 240 MHz, 520 Ko SRAM,
    4 Mo flash, BLE 4.2 + Wi-Fi ; suffisant pour une démo (`powl/01 §4.3` :
    « prototype only » mais acceptable). Contre : pas de PSRAM → buffers réduits,
    débit BLE + Wi-Fi plus faible ; `olivier/analyse-besoins §4` note la
    « contrainte forte » RAM ~520 Ko.
- **Piste de résolution** : décision d'équipe (budget + délai). Option
  pragmatique : **prototyper sur les WROOM Freenove existantes** avec les
  quotas réduits de `powl` (`ENVELOPE_STORE_MAX = 64`), acheter 1–2 WROVER
  seulement si les tests de charge montrent une saturation.
- **Orientation proposée** (2026-09-08, à valider en réunion) : **ESP32-WROOM-32E
  (cartes Freenove déjà en possession)**. Motifs : zéro achat, zéro délai ; le
  périmètre démo réel (2 téléphones + 1-2 relais, ~5-8 appareils, file ~50
  messages sur ESP32) tient dans les 520 Ko de SRAM avec les quotas réduits de
  `powl` (`ENVELOPE_STORE_MAX = 64`, caches divisés par ~8) ; `powl/01 §4.3`
  qualifie lui-même la WROOM d'« acceptable » pour un prototype. Conséquences à
  acter dans la spec : buffers dimensionnés pour la SRAM (pas de PSRAM), éviction
  LRU stricte, coexistence BLE + Wi-Fi en temps partagé (le relais bascule Wi-Fi
  pour flush MQTT puis revient en BLE). Réévaluer une WROVER seulement si un test
  de charge terrain montre une saturation mémoire ou un débit BLE+Wi-Fi
  insuffisant.
- **Statut** : `à confirmer`. `powl` retient WROVER ; le matériel réel est WROOM.
  L'orientation proposée retient **WROOM** (matériel existant + quotas réduits
  `powl`), à valider en réunion.

### A-5. Stack du dashboard

- **Sujet** : quelle stack pour le serveur d'observabilité et sa page ?
- **Sources** : `powl/01 §5`, `powl/07`, `powl/02 §5` ; `olivier/etude-stack §6`,
  `olivier/dashboard.md`, `olivier/architecture §8` ; `oswin/05 §3-4`,
  `oswin/09 §6-7`.
- **Options** :
  - **MQTT/TLS → Axum (Rust) → PostgreSQL 16 + TimescaleDB → React + TS + Vite**,
    Docker Compose + Caddy + Mosquitto sur le VPS (défendu par `powl`). Pour : le
    backend Rust **réutilise `dengon-core`** (mêmes structs d'événements, même
    `ledger::verify_chain`, mêmes constantes) → un seul langage serveur ↔ core,
    la vérification de journal chaîné n'est pas réimplémentée ; TimescaleDB
    adaptée au flux d'événements haute cardinalité ; hypertable + rétention +
    agrégats continus. Contre : lourd (5 conteneurs) ; suppose l'équipe à l'aise
    en Rust côté serveur ; sur-dimensionné pour une démo de 5-8 appareils.
  - **FastAPI ou Express + SSE + SQLite** sur le VPS Debian (défendu par
    `olivier`). Pour : petite API HTTP + base + canal temps réel + page statique,
    rien d'exotique ; SQLite suffit pour le volume d'une démo ; SSE plus simple
    qu'un WebSocket ; « au choix de qui prend le dashboard » (Olivier). Contre :
    la vérification de hash-chain est à réimplémenter dans le langage du serveur ;
    pas de continuité avec le core.
  - **ThingsBoard ou Node-RED + MQTT** (défendu par `oswin`). Pour : dashboards
    prêts à l'emploi, widgets, MQTT natif, très pédagogique (Node-RED se câble à
    la souris) ; abondants tutoriels ESP32. Contre : outil « boîte noire » peu
    adapté à la vérification de journal chaîné signé et à la reconstruction de
    parcours par corrélation d'événements partiels ; personnalisation limitée.
- **Piste de résolution** : décision d'équipe selon l'ambition et le délai. Le
  choix `powl` n'a de sens que si le cœur Rust (A-2) est retenu. Repli
  raisonnable pour la démo : `olivier` (FastAPI/Express + SSE + SQLite) ou une
  maquette (`dashboard.md §10`).
- **Orientation proposée** (2026-09-08, à valider en réunion) : **option 2
  (`olivier`) — petit backend + SSE + SQLite + page web légère**, malgré le choix
  du cœur Rust (A-2). Raisonnement au vu des choix précédents :
  - le cœur Rust rend l'option `powl` *possible* (réutilisation de
    `ledger::verify_chain` et des structs d'événements `serde`), mais ne la rend
    pas *raisonnable* : Axum + PostgreSQL/TimescaleDB + Mosquitto + Caddy + React
    en Docker Compose = beaucoup d'infra pour une équipe débutante sur ~3 semaines,
    alors que le dashboard est un **bonus explicitement non bloquant**
    (`CONTEXT.md`) pour une démo de 5-8 appareils dont les données sont effacées
    après chaque session (`olivier/dashboard`) ;
  - SQLite suffit largement à ce volume ; SSE suffit au temps réel ; le langage
    du backend = **au choix de la personne qui prend le dashboard** (Olivier) —
    Python/FastAPI ou Node/Fastify ;
  - **point de vigilance** hérité d'A-7 : si le journal chaîné signé est retenu,
    la vérification (marche de hash-chain + `Ed25519 verify`) doit exister côté
    serveur. Deux façons de ne pas la réimplémenter à l'identique : (a) ~100
    lignes dans le langage du backend avec une lib Ed25519 standard, ou (b)
    **déléguer à un petit binaire Rust `dengon-verify`** issu du workspace (mêmes
    règles que le cœur), appelé en sous-processus par le backend. Option (b)
    préférée : une seule source de vérité pour la vérif, sans backend Rust
    complet ;
  - conséquence sur **A-6** : cette orientation tire vers **HTTPS POST par batch**
    (pas de broker Mosquitto à opérer) ; l'ESP32 poste quand il a du Wi-Fi.
  - **ThingsBoard / Node-RED écartés** (déjà argumenté : inadaptés à la vérif de
    journal chaîné et à la reconstruction de parcours par événements partiels).

  Bascule vers l'option `powl` (Axum + Timescale) **seulement si** la personne
  dashboard est déjà à l'aise en Rust côté serveur et veut l'exercice.
- **Statut** : `tranché` (Paul, 2026-09-08, à ratifier en réunion) → **option 2
  (`olivier`)** : **backend FastAPI (Python)** + SSE + SQLite + page web légère ;
  vérif du journal chaîné déléguée à un binaire Rust `dengon-verify` issu du
  workspace. ThingsBoard/Node-RED et la stack Axum/Timescale de `powl` sont
  écartés pour le MVP. Détail (framework de page) : voir C-8.

### A-6. Transport des logs nœud → dashboard

- **Sujet** : comment un nœud remonte ses événements au serveur ?
- **Sources** : `powl/01 §5.1`, `powl/06 §6.2`, `powl/07 §3.1` ;
  `olivier/dashboard §5, §7`, `olivier/architecture §7` ; `oswin/05 §3`.
- **Options** :
  - **MQTT over TLS** (`mqtts://<vps>:8883`, mTLS, QoS 1, topics
    `dengon/logs/<relay_id>` etc.), fallback HTTPS POST par batch pour les
    clients opt-in (défendu par `powl` et `oswin`). Pour : conçu pour une flotte
    d'objets, QoS, reconnexion, léger sur ESP32 (lib officielle) ; buffer ring
    littlefs hors-ligne + flush FIFO. Contre : broker à opérer (Mosquitto).
  - **POST HTTPS d'événements** (défendu par `olivier/dashboard`). Pour : pas de
    broker à opérer ; API de réception HTTPS simple. Contre : pas de push,
    gestion de la reconnexion à la main.
- **Piste de résolution** : aligné dès qu'A-5 est tranché. MQTT si stack `powl` ;
  HTTPS POST possible si stack `olivier`. Les deux conceptions gardent HTTPS
  batch comme canal des **clients opt-in**.
- **Statut** : `tranché` (conséquence d'A-5, 2026-09-08, à ratifier en réunion) →
  **HTTPS POST par batch** (signé), pas de broker MQTT à opérer. L'ESP32 et les
  clients opt-in postent leurs événements par lots quand ils ont du Wi-Fi ;
  buffer local (ring littlefs sur ESP32) + flush FIFO à la reconnexion.

### A-7. Anti-triche du suivi (intégrité des journaux)

- **Sujet** : comment garantir qu'un relais ne peut pas mentir dans ce qu'il
  remonte au dashboard ?
- **Sources** : `powl/01 §1`, `powl/04 §4`, `powl/08 §attest/integrity` ;
  `olivier/dashboard §7` ; `oswin/04 §6`.
- **Options** :
  - **Journal chaîné signé par appareil + `LOG_ATTEST` + vérifications côté
    dashboard (chain_broken / fork_detected / gap) + recoupement multi-relais**
    (défendu par `powl`, aligné avec `oswin/04 §6`). Pour : trace infalsifiable
    *a posteriori* sans consensus ; le dashboard **constate** mais ne peut pas
    forger ; c'est l'interprétation retenue de « sécurité type blockchain » ;
    beau visuel pour la soutenance. Contre : chaque nœud tient un `ledger`
    (append en SQLite / littlefs) ; l'ESP32 doit compiler `ledger` en `no_std` ;
    le nœud peut toujours **omettre** un événement *avant* de signer (d'où le
    recoupement multi-relais comme garde-fou).
  - **« Authentification des envois » simple — jeton partagé entre nœuds et
    serveur** (position `olivier/dashboard §7`, explicitement marquée
    « (ouvert) »). Pour : suffit à empêcher **un tiers** d'injecter de faux
    événements ; trivial à implémenter. Contre : ne protège **pas** contre un
    relais compromis qui ment sur ce qu'il a vu ; pas de détection de
    réécriture *a posteriori*.
- **Piste de résolution** : le journal chaîné est un choix de conception fort de
  `powl` et un argument de soutenance (le « blockchain-like » défendable).
  À conserver si le cœur Rust est retenu (le `ledger` y vit). Sinon, repli sur
  le jeton partagé + une chaîne de hash minimale côté relais.
- **Statut** : `tranché` par transitivité (2026-09-08, à ratifier en réunion) →
  **option 1 — journal chaîné signé + `LOG_ATTEST` + recoupement multi-relais**.
  Découle de : **A-2** (cœur Rust retenu → `ledger` y vit) ; **A-3** repli
  (`sha2` + `ed25519-dalek` cross-compilent en `no_std` pour xtensa = exactement
  ce dont `ledger` a besoin, le « contre ESP32 » est levé) ; **A-5** (binaire
  Rust `dengon-verify` fait la vérif de chaîne côté serveur) ; **A-15** (déjà
  `tranché` : journal chaîné signé, pas de consensus). Le « jeton partagé » de
  l'option 2 n'est pas abandonné : il sert d'**auth de transport** (qui a le
  droit de POST des événements) — voir B-2 / C-5, distinct de l'intégrité du
  journal.

### A-8. Identifiant de nœud

- **Sujet** : sur combien d'octets et comment dérive-t-on l'identifiant d'un
  nœud ?
- **Sources** : `powl/01 §1.2`, `powl/03 §2-3`, `powl/04 §2.1`, `powl/09` ;
  `olivier/protocole §3`, `olivier/format-trame §3`.
- **Options** :
  - **`peerID` = `SHA-256(pub_static)[0..8]` — 8 octets** (défendu par `powl`).
    Pour : compact (économe sur BLE, en-tête L3 22/30 o) ; suffisant pour éviter
    les collisions à l'échelle d'un réseau de terrain ; règle anti-boucle de
    connexion « le plus petit `peerID` initie ». Contre : 64 bits, risque de
    collision théorique plus élevé qu'avec 128 bits.
  - **« Empreinte de clé » — 16 octets** (défendu par `olivier/format-trame`,
    « par ex. les 16 premiers octets d'un hachage de la clé publique »). Pour :
    128 bits, collision négligeable ; aligné sur la taille de l'`id_message`
    (16 o). Contre : double la taille des champs d'adressage dans l'en-tête PDU
    (2 × 16 o vs 2 × 8 o).
- **Piste de résolution** : à figer dans la spec de trame unifiée (voir A-12).
  Recommandation implicite : suivre `powl` (8 o) pour la compacité BLE, sauf
  argument de sécurité contraire dans la doc sécurité.
- **Statut** : `tranché` par transitivité (via A-12 → `powl/03` fait foi) →
  **`peerID` = `SHA-256(pub_static)[0..8]`, 8 octets**. La doc sécurité peut
  encore justifier 128 bits, mais la charge de la preuve incombe à cet argument.

### A-9. Identifiant de message

- **Sujet** : l'identifiant d'un message est-il aléatoire ou dérivé du contenu ?
  Un ou deux identifiants ?
- **Sources** : `powl/03 §3.2, §4.1`, `powl/05 §1` ; `olivier/format-trame §2, §7`,
  `olivier/protocole §5`.
- **Options** :
  - **`msgID` = `SHA-256(sender_id ‖ timestamp_ms ‖ type ‖ payload)` (32 o,
    dérivé du contenu) + `msg_uuid` séparé (16 o, UUIDv4, stable de bout en
    bout)** (défendu par `powl`). Pour : `msgID` rend le paquet **idempotent**
    (le rejouer n'a aucun effet), sert de clé de dédup et de référence de suivi
    (tracé haché) ; `msg_uuid` suit la machine à états des statuts et reste stable
    même si le paquet est **re-scellé** (le `msgID` L3 change alors). Contre :
    deux identifiants à gérer ; `msgID` 32 o.
  - **`id_message` unique — 16 octets aléatoires, stable réseau, jamais modifié
    par un relais** (défendu par `olivier/format-trame`). Pour : simple (un seul
    identifiant) ; 16 o ; « corréler l'accusé » directement. Contre : pas
    d'idempotence par contenu (un attaquant peut forger deux paquets différents
    avec le même ID) ; ne distingue pas « paquet » et « message applicatif ».
- **Piste de résolution** : à figer dans la spec de trame unifiée (A-12) et la
  doc sécurité. Le double identifiant de `powl` répond à un vrai besoin (re-
  scellage d'enveloppe) qui n'existe que si les enveloppes scellées Noise X sont
  retenues (A-3).
- **Statut** : `tranché` par transitivité (via A-3 → enveloppes Noise `X`
  retenues, et A-12 → `powl/03` fait foi) → **`msgID` 32 o (hash contenu) +
  `msg_uuid` 16 o (UUIDv4 stable de bout en bout)**. Le re-scellage d'enveloppe
  (qui change le `msgID` L3) rend le second identifiant nécessaire.

### A-10. Statut « Lu » dans le périmètre v1 ?

- **Sujet** : la v1 gère-t-elle le statut « Lu/vu » ?
- **Sources** : `olivier/CONTEXT.md` (liste canonique à 4 statuts) ;
  `olivier/decisions-v1 §décision 8`, `olivier/protocole §1, §7` ; `powl/00 §4`,
  `powl/05 §1`.
- **Options** :
  - **« Lu » dans le MVP** (défendu par `powl`) : `READ` = read-receipt signé
    reçu quand l'utilisateur ouvre la conversation ; `AppFrame kind 3 ReadRcpt` ;
    « Lu implique distribué ». Pour : messagerie complète, comportement attendu ;
    le mécanisme est le même qu'un ACK (peu de code en plus). Contre : un 2ᵉ type
    d'accusé à faire remonter.
  - **« Lu » reporté en v2** (défendu par `olivier/decisions-v1`) : v1 =
    En attente → Parti → Distribué. Pour : réduit le périmètre v1 (délai serré) ;
    le « Distribué » qui remonte est déjà « la partie la moins triviale de la
    v1 ». Contre : `CONTEXT.md` liste « Lu/vu » comme statut n°4 canonique.
- **Piste de résolution** : décision d'équipe au regard du délai (soutenance
  29/09/2026). Le format `olivier/format-trame §5` **réserve déjà** `statut = 4`
  (LU) pour la v2 → compatible avec les deux options.
- **Statut** : `tranché` (Paul, 2026-09-08, à ratifier en réunion) → **option
  `olivier` : « Lu » reporté en v2**. La v1 s'arrête à *En attente → Parti →
  Distribué*. Le type de paquet `READ` / `ReadRcpt` et le statut `4 = LU` restent
  **réservés** dans `powl/03` et `olivier/format-trame` (pas d'émission ni de
  traitement en v1) → aucune rupture de format à prévoir pour l'ajouter plus
  tard. Conséquence : `powl/05` (machine à états) est amputé de l'état `READ`
  pour le MVP ; à refléter dans `00-contexte-global.md` §9 lors de la
  ratification.

### A-11. iOS

- **Sujet** : iOS est-il visé en v1, prévu par l'architecture, ou reporté ?
- **Sources** : `olivier/CONTEXT.md` (iOS listé) ; `olivier/analyse-besoins §6`,
  `olivier/decisions-v1 §décision 2`, `olivier/etude-stack §1.4` ; `powl/00 §5`,
  `powl/01 §2` ; `oswin/07 §1, §7`.
- **Options** :
  - **Hors MVP mais prévu par l'architecture** (`powl`) : le cœur Rust + UniFFI/
    Swift rend l'app iOS possible plus tard ; BLE en fond bridé sur iOS. Pour :
    ne bloque pas le MVP, garde la porte ouverte. Contre : `CONTEXT.md` met iOS
    dans le besoin métier.
  - **Reporté, « risque technique n°1 »** (`olivier`) : faire une preuve de
    faisabilité iOS **dès le départ** si on y tient ; sinon report explicite. La
    raison technique (`etude-stack §1.4`) : en arrière-plan, un iPhone en rôle
    « visible » place ses services dans une *overflow area* découvrable
    uniquement par un autre appareil Apple → un ESP32/Android ne le voit pas.
    Comportement **système, non contournable**.
  - **Paliers + plan B** (`oswin`) : si les téléphones (Android compris) ne
    savent pas être périphérique, passer le cœur du mesh sur les ESP32.
- **Piste de résolution** : consensus de fait — **iOS hors v1**. À trancher : si
  on fait ou non une preuve de faisabilité iOS anticipée (`olivier` la
  recommande). L'architecture `powl` (cœur Rust) est celle qui préserve le mieux
  l'option iOS future.
- **Statut** : `tranché` (Paul, 2026-09-08, à ratifier en réunion) → **iOS
  reporté en v2, sans preuve de faisabilité anticipée**. Le MVP est Android +
  ESP32 uniquement (`decisions-v1 §2`, `powl/00 §5`). L'ouverture iOS reste
  garantie par l'architecture : cœur Rust `dengon-core` + `trait Transport` +
  bindings Swift via UniFFI (A-1, A-2) → un futur shell SwiftUI + transport
  CoreBluetooth suffira, sans retoucher le cœur. La contrainte système iOS
  (*overflow area* en arrière-plan, `etude-stack §1.4`) est documentée comme
  risque connu à lever en v2, pas maintenant.

### A-12. Format binaire de trame : deux specs à unifier

- **Sujet** : `powl/03 §3` et `olivier/format-trame.md` décrivent **deux formats
  binaires différents**. Lequel fait foi ?
- **Sources** : `powl/03` (déclaré « source de vérité »), `powl/09 §3` ;
  `olivier/format-trame.md` v0.1 (déclaré « fait foi » de son côté),
  `olivier/protocole §5`, `olivier/mise-en-commun §1`.
- **Différences principales** :

  | Aspect | `powl/03` | `olivier/format-trame` |
  | --- | --- | --- |
  | En-tête | L3 : 22 o (broadcast) / 30 o (adressé) + 64 o signature | Fragment BLE : 24 o ; PDU : 44 o |
  | Identifiant nœud | `sender_id`/`recipient_id` = `peerID` 8 o | `empreinte_expediteur`/`_destinataire` 16 o |
  | Identifiant message | `msgID` 32 o (hash contenu) + `msg_uuid` 16 o | `id_message` 16 o aléatoire |
  | Types de PDU | 12 (`0x01`–`0x0C` : ANNOUNCE, NOISE_HS/MSG, SEALED_ENVELOPE, ACK, GOSSIP_*, FRAGMENT, LOG_ATTEST, ENVELOPE_*) | 3 (DONNÉES=1, ACCUSÉ=2, INVENTAIRE=3) |
  | Fragmentation | L2 : `frag_id(8) ‖ index(2) ‖ total(2)`, `FRAG_SIZE=440` | en-tête fragment 24 o inclut `id_message` + `index_fragment` + `nombre_fragments`, charge ~156 o |
  | Chiffrement | Noise (XX transport / X scellé), padding buckets | `nonce(24) ‖ etiquette_auth(16) ‖ contenu_chiffre` (libsodium `crypto_box`) |
  | Handshake / gossip / attestation | types de paquets dédiés | absents (échange d'inventaire à la place du gossip GCS) |
  | Endianness | big-endian | big-endian (identique) |
  | TTL initial | 7 (`TTL_DEFAULT`) | 7 (identique) |

- **Options** :
  - **Adopter `powl/03` comme spec unique** (c'est la source de vérité désignée
    par `CLAUDE.md`). Pour : plus complète (handshake Noise, gossip GCS,
    enveloppes scellées, attestation) ; cohérente avec le reste de la conception
    `powl` (cœur Rust, Noise). Contre : plus lourde à implémenter deux fois si
    A-2 penche vers deux implés ; le format `olivier` est celui autour duquel
    l'équipe `olivier` a travaillé.
  - **Repartir de `olivier/format-trame` et l'étendre** (plus proche du
    périmètre v1 resserré, 3 PDU, `crypto_box`). Pour : plus simple pour une
    démo ; aligné sur « deux implémentations + spec stricte ». Contre : pas de
    handshake de session (chaque message re-fait un `crypto_box` one-shot), pas
    de gossip compact (échange d'inventaire = ~4,8 Ko d'IDs pour une file
    pleine), pas d'attestation de journal.
  - **Fusion** : garder la structure `olivier` (3 PDU) pour le MVP démo, ajouter
    progressivement les types `powl` (gossip, attestation) selon le temps.
- **Piste de résolution** : **doc sécurité + réunion de conception**. C'est le
  « chantier fondateur à figer en premier » (`oswin/08 §3`). Le choix dépend
  d'A-2 (un cœur → `powl/03` naturellement) et d'A-3 (Noise → `powl/03` ;
  `crypto_box` → `olivier`).
- **Statut** : `tranché` par transitivité (2026-09-08, à ratifier en réunion) →
  **`powl/03` fait foi comme spec de trame unique** (découle d'A-2 : un cœur Rust
  → `powl/03` ; et d'A-3 : Noise retenu → `powl/03`). `olivier/format-trame.md`
  n'est plus une spec concurrente : il reste utile comme support pédagogique et
  source de vecteurs de test. **Sous-ensemble MVP à définir dans la doc** : les
  types `ANNOUNCE`, `NOISE_HS`, `NOISE_MSG`, `SEALED_ENVELOPE`, `ACK`, `FRAGMENT`,
  `LOG_ATTEST`, `ENVELOPE_OFFER/REQUEST` sont le cœur ; les `GOSSIP_*` peuvent
  être remplacés au MVP par l'échange d'inventaire d'`olivier` (voir A-13) sans
  changer le format des autres paquets.

### A-13. Détails de routage : ajouts `powl` vs ajouts `olivier`

- **Sujet** : au-delà du socle commun (flood contrôlé + TTL 7 + dédup + « écouter
  avant de rediffuser »), chaque conception ajoute des mécanismes différents.
- **Sources** : `powl/03 §7` ; `olivier/protocole §6, §9`,
  `olivier/format-trame §6`.
- **Socle commun (non divergent)** : diffusion contrôlée à tous les voisins ;
  TTL initial **7**, −1 par relais, 0 → stop ; table « déjà vu » / seen-set ;
  jitter aléatoire « écouter avant de rediffuser » (idée Meshtastic) ; ne pas
  réémettre vers la source ; conditions d'arrêt (TTL, déjà vu, expiration ~24 h,
  ACK passé par là).
- **Ajouts `powl`** : clamp de densité (`ttl' = min(ttl-1, TTL_CLAMP_DENSE=5)` si
  ≥ 6 voisins) ; réconciliation **gossip par GCS** (`GOSSIP_FILTER/PULL/PUSH`,
  Golomb-Coded Set, `p = 1/64`) ; **budget de copies** partagé en deux à chaque
  rencontre de porteurs (Spray-and-Wait) pour les enveloppes ; quotas
  paquets/s par `peerID` et par lien, RSSI-gating ; rejet si `timestamp_ms` hors
  fenêtre ±2 h.
- **Ajouts `olivier`** : **échange d'inventaire** d'`id_message` entre voisins
  (PDU INVENTAIRE), puis chacun n'envoie que ce qui manque à l'autre —
  alternative simple au gossip GCS (repli « pousser toute la file » si problème
  d'implé) ; **limite anti-inondation** ~20 nouveaux `id_message` / min / voisin
  (voisin trop bavard temporairement ignoré) ; réassemblage complet **avant** de
  relayer (plus simple pour vérifier destinataire et dédupliquer).
- **Piste de résolution** : converger dans la spec unifiée (A-12). Le gossip GCS
  (`powl`) est plus économe en BLE que l'échange d'inventaire brut (`olivier`)
  mais plus complexe à coder ; l'échange d'inventaire est un bon repli MVP.
  L'anti-inondation `olivier` (~20 msg/min/voisin) et les quotas `powl` sont
  complémentaires et peuvent coexister.
- **Statut** : `tranché` par transitivité (via A-12 → `powl/03 §7` fait foi,
  2026-09-08, à ratifier en réunion). Découpage explicite point par point :

  | Mécanisme | Décision | MVP / cible |
  | --- | --- | --- |
  | Socle : flood + TTL 7 + seen-set + jitter + « pas vers la source » + arrêts (TTL, déjà-vu, expiration, ACK vu) | **retenu** (jamais divergent) | MVP |
  | Clamp de densité (`ttl' = min(ttl-1, 5)` si ≥ 6 voisins) | **retenu** (`powl`) — coût quasi nul | MVP |
  | Fenêtre `timestamp_ms` ±2 h | **retenu** (`powl`) — attendu anti-rejeu | MVP |
  | Quotas paquets/s par `peerID` et par lien | **retenu** (`powl`) — attendu anti-DoS | MVP |
  | RSSI-gating | **retenu** (`powl`) mais **optionnel au MVP** (calibrage terrain) | cible |
  | Anti-inondation ~20 nouveaux `id_message`/min/voisin | **retenu** (`olivier`) — complémentaire des quotas | MVP |
  | Réconciliation voisin↔voisin | **échange d'inventaire brut d'`id_message`** (`olivier`) au MVP ; **GCS `GOSSIP_*`** (`powl`, `p = 1/64`) en optimisation | MVP = inventaire, cible = GCS |
  | Budget de copies / Spray-and-Wait pour enveloppes scellées | **retenu** (`powl`) — c'est le cœur du store-carry-forward (A-3) ; repli MVP possible : flood d'enveloppes borné par `MSG_TTL_S` + dédup si l'implé Spray-and-Wait déborde | cible (repli MVP acceptable) |
  | Réassemblage **avant** relais | **retenu** (`olivier`) — messages < 1 Ko = 2-3 fragments, permet vérif de signature + dédup sur le paquet entier ; le relais fragment-par-fragment de `powl` reste une évolution pour gros payloads | MVP |

  Le GCS et le budget de copies sont les deux seuls points « cible » : ils
  s'ajoutent plus tard **sans changer le format des autres paquets** (A-12).

### A-14. Structure du dépôt

- **Sujet** : comment organiser le dépôt commun ?
- **Sources** : `powl/02 §4` ; `olivier/mise-en-commun §1` ; réel (`CLAUDE.md`,
  `git`).
- **Options** :
  - **Découpage `crates/` de `powl/02`** : `crates/{dengon-core, dengon-ble,
    dengon-node, dengon-sim, dengon-ffi}`, `android/`, `firmware/dengon-relay/`,
    `dashboard/{api, web, deploy}/`, workspace Rust. Cohérent avec « un cœur
    Rust » (A-2).
  - **Structure `olivier/mise-en-commun §1`** : `app/`, `firmware/`,
    `dashboard/{serveur, page}/`, `outils/`, `docs/{concept, decisions, planning,
    protocole/, architecture, dashboard, stack}.md`, `docs/{olivier,paul,tanguy}/`
    en archive. Cohérente avec « deux implémentations » (A-2).
- **État réel** : le dépôt utilise aujourd'hui `docs/powl/` (conception) +
  `docs/suivi/` (suivi), **aucun** répertoire de code. Ni `crates/` ni `app/`
  n'existent.
- **Piste de résolution** : découle d'A-2. À figer lors de la mise en commun
  (fin phase conception, ~fin semaine 1 selon `olivier`).
- **Statut** : `tranché` par transitivité (via A-2 → cœur Rust unique) →
  **découpage `crates/` de `powl/02`** : `crates/{dengon-core, dengon-ble,
  dengon-node, dengon-sim, dengon-ffi}` + `crates/dengon-verify` (binaire de
  vérif de journal appelé par le dashboard, cf. A-5), `android/`,
  `firmware/dengon-relay/`, workspace Rust. **Ajustement** : `dashboard/` contient
  l'app Python/Node d'`olivier` (A-5), pas un crate Rust `dashboard/api`. Les
  brouillons perso restent sous `docs/{powl,oswin,olivier}/`.

### A-15. « Blockchain » : quelle interprétation retenir ?

- **Sujet** : l'énoncé évoque « un système similaire à de la blockchain ». Que
  fait-on ?
- **Sources** : `powl/01 §1`, `powl/04 §4` ; `oswin/04` (analyse critique
  complète) ; `oswin/05 §5`.
- **Options** :
  - **Journal chaîné signé par appareil, agrégé et audité par le dashboard, sans
    consensus** (défendu par `powl`, aligné avec `oswin/04 §5`). Pour :
    infalsifiabilité (hash-chain) + décentralisation (clés, gossip) **sans** le
    coût du consensus (inutile et nuisible : privé vs registre public, nœuds
    contraints, réseau intermittent, besoin de purge) ; formulation défendable et
    plus impressionnante qu'« on a fait une blockchain ». Arbre de Merkle
    optionnel pour les accusés groupés / l'intégrité des fragments. Contre :
    n'est pas « une blockchain » au sens strict — à bien expliquer à la
    soutenance.
  - **Blockchain locale légère à autorité de confiance** (repli `oswin/04 §6`
    « si l'énoncé impose vraiment ») : registre chaîné des **événements**
    (émis/relayé/livré), pas du contenu ; hébergé/agrégé par la passerelle →
    dashboard = le « registre » ; consensus remplacé par la **signature** de
    chaque nœud (preuve d'autorité simplifiée). Pour : coche la case
    « blockchain » ; se visualise très bien sur le dashboard. Contre : proche du
    journal chaîné de `powl`, sans réel apport.
- **Piste de résolution** : conserver la formulation `powl`/`oswin` (journal
  chaîné signé, pas de consensus) et **préparer l'argumentaire de soutenance**
  (le `docs/suivi/06-support-oral.md` prévoit déjà « ce qu'on a gardé et
  rejeté »). Si un enseignant exige littéralement « une blockchain », basculer
  sur le vocabulaire du repli `oswin/04 §6` sans changer le code.
- **Statut** : `tranché` (Paul, 2026-09-08, à ratifier en réunion) → **option 1 :
  journal chaîné signé par appareil, agrégé et audité par le dashboard, sans
  consensus** (arbre de Merkle optionnel pour les accusés groupés / l'intégrité
  des fragments). Cohérent avec A-7 (même mécanisme) et A-2 (le `ledger` vit dans
  `dengon-core`). Reste `ouvert` uniquement : la **formulation** dans le rapport
  et la soutenance (assumer « ce n'est pas une blockchain au sens strict, et
  voici pourquoi le consensus serait nuisible ici ») ; repli lexical `oswin/04
  §6` si un enseignant exige littéralement le mot « blockchain », **sans changer
  le code**.

---

## B. Points ouverts internes `powl` (spike Lot 0)

### B-1. Crypto sur ESP32 : Rust cross-compilé ou trait `Crypto` + lib C ?

- **Sources** : `powl/01 §4.2`, `powl/02 §2.frontières`, `powl/06 §3`,
  `powl/10 Lot 0 Spike A`.
- **Options** : (a) `snow` + `ed25519-dalek` cross-compilent proprement pour
  xtensa (`getrandom` → source ESP32) → **tout en Rust** ; (b) sinon, isoler
  Noise/Ed25519 derrière un trait `Crypto` et l'implémenter **côté C avec mbedTLS
  (déjà présent) ou libsodium (port ESP-IDF)**.
- **Piste de résolution** : **Spike A** obligatoire au Lot 0. Livrable : rapport
  de décision.
- **Statut** : **`tranché`** — **Spike A exécuté le 2026-09-10 (US-101,
  issue #1). Réponse : OUI, option (a), tout en Rust.** Rapport complet :
  [`suivi/spikes/US-101-cross-compile-xtensa.md`](../suivi/spikes/US-101-cross-compile-xtensa.md).
  `sha2`, `ed25519-dalek`, `x25519-dalek`, `chacha20poly1305` **et `snow`**
  compilent pour `xtensa-esp32-none-elf` en `no_std + alloc` et produisent un
  `.a` linkable. Le curseur Rust/C que A-3 laissait ouvert est donc **100 %
  Rust** : le repli `trait Crypto` + mbedTLS **n'est pas activé**, aucune crypto
  en C n'est à écrire.
  - **Condition 1 — `snow` ≥ 0.10.0 obligatoire.** La 0.9.6 (celle qu'avaient en
    tête `powl/01` et A-3) est **structurellement** impossible en `no_std` :
    `rand_core` y est non optionnel avec `features = ["std", "getrandom"]` en
    dur, et la crate n'a pas de `#![no_std]`. La 0.10.0 rend `std` et
    `getrandom` optionnels. À épingler en US-108.
  - **Condition 2 — le firmware fournit l'aléa.** Sans `use-getrandom`,
    `snow` compile mais `resolve_rng()` renvoie `None` : le handshake échoue
    **au runtime**, pas à la compilation. Il faut un `CryptoResolver` maison sur
    `esp_fill_random()` (écrit et compilé pendant le spike). ⚠️ ce dernier n'est
    un vrai TRNG que Wi-Fi/BT actif ou après `bootloader_random_enable()` — à
    vérifier en US-307.
  - **Non prouvé par le spike** : rien n'a tourné sur matériel, et le link dans
    un vrai projet ESP-IDF reste à valider (US-307).

### B-2. Authentification des relais auprès du VPS : mTLS ou JWT signé ?

- **Sources** : `powl/02 §5`, `powl/04 §5-6`, `powl/06 §6.2`, `powl/07 §3.1`.
- **Options** : **mTLS** (certificat client par relais) — retenu comme principal ;
  **JWT signé court** — listé comme *fallback*. Les deux sont « acceptables ».
- **Piste de résolution** : décision d'implémentation lors du Lot 5 (firmware) /
  Lot 6 (dashboard). mTLS demande une CA MQTT + génération de certs
  (`deploy/mosquitto/gen-certs.sh`).
- **Statut** : `tranché` par transitivité (via A-5 + A-6 → HTTPS POST, plus de
  broker MQTT ; 2026-09-08, à ratifier en réunion) → **jeton/JWT court par nœud
  (liste blanche côté serveur) + signature Ed25519 des batchs d'événements** sur
  l'endpoint HTTPS. mTLS abandonné (sa justification était les certs client
  MQTT). Le jeton authentifie le transport (qui a le droit de POST) ;
  l'intégrité du contenu vient du journal chaîné (A-7). Format exact du jeton =
  détail d'implémentation au Lot 5/6. Recoupe C-5.

### B-3. Chiffrement de la base locale : SQLCipher ou champ-par-champ XChaCha20 ?

- **Sources** : `powl/02 §7`, `powl/04 §1.2, §7`, `powl/09 §1`.
- **Options** : **SQLCipher (AES-256)** via `rusqlite` + feature `sqlcipher` —
  simple, chiffre toute la base ; **chiffrement champ-par-champ XChaCha20** —
  plus fin, pas de dépendance native supplémentaire.
- **Piste de résolution** : décision d'implémentation au Lot 2 (`store`). Test
  `store` : « chiffrement au repos actif ».
- **Statut** : `tranché` par transitivité (2026-09-08, à ratifier en réunion) →
  **XChaCha20-Poly1305 champ par champ** sur les colonnes sensibles (contenu des
  messages ; clés privées de préférence dans Keystore/Keychain). Raisons héritées
  des choix précédents : le crate `chacha20poly1305` est **déjà** dans l'arbre de
  dépendances (A-3) et fournit `XChaCha20Poly1305` ; **pas de dépendance native
  en plus** dans le build (A-2 gère déjà UniFFI + cross-compile xtensa +
  `cbindgen`, SQLCipher ajouterait une lib C à cross-compiler pour chaque ABI
  Android) ; **même primitive réutilisable pour la NVS de l'ESP32** (A-4). Les
  métadonnées locales (horodatage, `peerID`, statut) restent en clair dans la
  base — le modèle de menace local est le vol d'appareil, ciblé sur le contenu
  et les clés.

### B-4. Rétention des événements du dashboard

- **Sources** : `powl/04 §6`, `powl/09 §2` (`add_retention_policy('events',
  INTERVAL '90 days')`).
- **Sujet** : la valeur par défaut est **90 jours**, marquée « config ». À
  confirmer selon la capacité du VPS et les besoins de démo. `olivier/dashboard`
  retient au contraire « effacé après chaque session de démo » (voir E — c'est un
  choix produit distinct pour la v1 minimale).
- **Piste de résolution** : paramètre de config ; valeur par défaut à fixer au
  Lot 6.
- **Statut** : `tranché` par transitivité (via A-5 → stack `olivier` / SQLite,
  2026-09-08, à ratifier en réunion) → **base du dashboard effacée après chaque
  session de démo** (`olivier/dashboard`) ; script de purge simple, pas de
  politique de rétention automatique. La rétention Timescale 90 j de `powl/09`
  n'a plus d'objet (plus de TimescaleDB). Une rétention glissante configurable
  est renvoyée en post-MVP. Cohérent avec la confidentialité stricte visée
  (métadonnées sensibles : qui/quand/où).

### B-5. Backend du dashboard : Rust/Axum ou Node/Fastify ?

- **Sources** : `powl/01 §5.2`.
- **Options** : **Rust + Axum** — retenu (réutilise les types d'événements et la
  vérif hash-chain de `dengon-core`) ; **Node.js + TypeScript (Fastify)** —
  « acceptable si contrainte d'équipe » (mais réimplémentation de la vérif de
  chaîne / parsing).
- **Piste de résolution** : décision d'équipe selon l'aisance en Rust côté
  serveur ; recoupe A-5.
- **Statut** : `tranché` par transitivité (via A-5) → **pas de backend Rust
  complet**. Backend Python/FastAPI ou Node/Fastify (au choix de qui prend le
  dashboard) ; la seule brique Rust est le binaire `dengon-verify` appelé en
  sous-processus pour la vérif de journal. Détail du langage = C-8.

---

## C. Points ouverts internes `olivier`

### C-1. Valeurs numériques à calibrer sur le terrain

- **Sources** : `olivier/format-trame §7-8`, `olivier/protocole §12`,
  `olivier/decisions-v1 §encore à trancher`.
- **Valeurs (points de départ, « à calibrer »)** :

  | Paramètre | Valeur de départ | À calibrer selon |
  | --- | --- | --- |
  | « Écouter avant de rediffuser » | ~50–500 ms | densité réelle, collisions |
  | Délai de réassemblage des fragments | ~30 s | latence terrain |
  | Fenêtre anti-inondation | ~20 nouveaux `id_message` / min / voisin | trafic légitime observé |
  | Taille de la file de retransmission | ~50 (ESP32) / ~300 (téléphone) | mémoire réelle des appareils |
  | Charge utile par fragment | ~156 octets | MTU BLE négocié sur les appareils cibles |
  | Expiration d'un message | ~24 h | (aligné `powl` : `MSG_TTL_S = 86 400`) |
  | TTL initial | 7 | (aligné Meshtastic / Bitchat / `powl`) |

- **Comparaison `powl`** : `powl/03 §2` fige des valeurs voisines mais précises
  (`RELAY_JITTER_MS = 10..=220`, `FRAG_TIMEOUT_S = 30`, `FRAG_SIZE = 440`,
  `MSG_TTL_S = 86_400`, `SEEN_SET_CAP = 1024`, `SEEN_TTL_S = 300`).
- **Piste de résolution** : tests terrain (checklist de recette `powl/11 §6`,
  scénario densité 8-10 appareils) ; consigner les vraies valeurs dans la spec.
- **Statut** : `tranché` par transitivité (2026-09-08, à ratifier en réunion).
  Toutes les valeurs de la spec sont **fixées** par les choix déjà faits ; il ne
  reste que du réglage fin sans enjeu de conception :

  | Paramètre | Valeur figée | Source |
  | --- | --- | --- |
  | Jitter « écouter avant de rediffuser » | `10..=220 ms` (`RELAY_JITTER_MS`) | `powl/03 §2` (A-12) |
  | Timeout de réassemblage | `30 s` (`FRAG_TIMEOUT_S`) | `powl/03 §2` (A-12) |
  | Fenêtre anti-inondation | `20` nouveaux `id_message` / min / voisin | A-13 (retenu MVP) |
  | Seen-set | `SEEN_SET_CAP = 1024`, `SEEN_TTL_S = 300` | `powl/03 §2` (A-12) |
  | Taille de fragment L2 | `FRAG_SIZE = 440` | `powl/03 §2` (A-12) |
  | Caps de file (ESP32) | réduits WROOM : `ENVELOPE_STORE_MAX = 64` etc. | A-4 |
  | Expiration message | `MSG_TTL_S = 86 400` (24 h) | `powl/03 §2`, aligné |
  | TTL initial | `7` | `powl/03 §2`, aligné |

  **Seuls vrais inconnus, non bloquants** : (1) la charge utile réelle par
  écriture BLE = f(MTU négocié) — **mesurée au Spike C** sur les appareils de
  l'équipe ; (2) ajustement éventuel du jitter / des quotas si les tests de
  densité (`powl/11 §6`) montrent des collisions — la valeur de spec est livrée
  telle quelle en attendant.

### C-2. Service et caractéristiques BLE exacts (UUID)

- **Sources** : `olivier/protocole §4`, `olivier/etude-stack §1`,
  `olivier/mise-en-commun §4` ; comparer `powl/03 §2`
  (`SERVICE_UUID = 6d656e67-2d64-656e-676f-6e2d76310000`, `CHAR_RX_UUID = …0001`,
  `CHAR_TX_UUID = …0002`) et `oswin/07 §3` (« un UUID inventé, ex.
  `0000d3n6-....` », caractéristiques `MESSAGE` + `ACK`).
- **Sujet** : `olivier` laisse « (ouvert) — décision d'implémentation (une
  caractéristique pour écrire une trame, une pour en recevoir) ». `powl` a déjà
  choisi des UUID concrets et 2 caractéristiques `RX` (write-w/o-response) / `TX`
  (notify). `oswin` propose une 3ᵉ caractéristique dédiée `ACK`.
- **Piste de résolution** : adopter les UUID `powl/03 §2` dans la spec unifiée
  (A-12) sauf raison contraire.
- **Statut** : `tranché` par transitivité (via A-12 → `powl/03` fait foi) →
  `SERVICE_UUID = 6d656e67-2d64-656e-676f-6e2d76310000`, `CHAR_RX` (…0001,
  write-w/o-response), `CHAR_TX` (…0002, notify). Pas de 3ᵉ caractéristique `ACK`
  dédiée (l'ACK est un type de paquet, pas une caractéristique).

### C-3. Format de l'échange d'inventaire (listing compact des identifiants)

- **Sources** : `olivier/protocole §9, §12`, `olivier/format-trame §6, §9`.
- **Sujet** : une file pleine (~300 messages) = ~4,8 Ko d'IDs ≈ ~30 fragments.
  Faut-il une représentation plus compacte (**filtre de Bloom**) dès la v1 ou
  est-ce une évolution ? `powl` répond par le **GCS** (`GOSSIP_FILTER`,
  Golomb-Coded Set, ~20–30 % plus compact qu'un Bloom).
- **Piste de résolution** : v1 = listing brut (simple) ou GCS si le cœur Rust
  fournit déjà l'implé ; Bloom / GCS comme optimisation sinon. Recoupe A-13.
- **Statut** : `tranché` — **doublon d'A-13** (lui-même tranché). **MVP = listing
  brut d'`id_message`** (simple, débogable) ; **GCS `GOSSIP_*`** (`powl`,
  `p = 1/64`) comme optimisation ajoutée sans changer le format des autres
  paquets. Pas de filtre de Bloom (le GCS le remplace, ~20-30 % plus compact).
  Voir A-13.

### C-4. Compteur « nombre d'appareils actifs » du dashboard

- **Sources** : `olivier/dashboard §3, §6, §11`, `olivier/decisions-v1 §points
  tranchés`.
- **Sujet** : impossible à déduire des événements puisque le code anonyme d'un
  nœud **change à chaque message**. Deux options : (a) ajouter un **battement
  anonyme séparé** (un nœud connecté signale « je suis actif » avec un code qui
  tourne, ex. chaque jour) ; (b) **retirer ce compteur** de la v1.
- **Comparaison `powl`** : le dashboard `powl` dérive `LINKS` et l'état des nœuds
  des événements `peer.*` et `relay.health` (les nœuds y ont un `node_id` stable,
  pas un code par message) → le compteur existe naturellement. La divergence
  vient du choix `olivier` d'un **code anonyme par message** (E, plus strict sur
  la confidentialité).
- **Piste de résolution** : décision produit. Si on garde le code-par-message
  (`olivier`), choisir (a) ou (b).
- **Statut** : `tranché` par transitivité (via A-7 + A-12/`powl/08`,
  2026-09-08, à ratifier en réunion). A-7 (journal chaîné signé **par appareil**)
  impose une **identité de nœud stable** dans les événements `LOG_ATTEST` — on ne
  peut pas vérifier une hash-chain par nœud si l'identité tourne. Donc le modèle
  d'événements est celui de `powl/08` : `node_id` = empreinte pseudonyme stable
  (un hash, non rattachable à une personne). **Le compteur d'appareils actifs se
  dérive naturellement** des événements `peer.*` / `relay.health` (`powl/07`) —
  ni battement séparé ni suppression du compteur. La confidentialité est assurée
  autrement : `node_id` haché, base effacée par session (B-4), aucune rétention
  de métadonnées.

### C-5. Authentification des nœuds auprès de l'API du dashboard

- **Sources** : `olivier/dashboard §7, §11`, `olivier/architecture §9`,
  `olivier/mise-en-commun §4`.
- **Sujet** : « jeton partagé entre les nœuds et le serveur ? » — but : empêcher
  un tiers d'injecter de faux événements. Recoupe **A-7** et **B-2** (`powl` :
  mTLS ou JWT + liste blanche + signature Ed25519 des batchs).
- **Statut** : `tranché` — **doublon de B-2** (lui-même tranché) → jeton/JWT
  court par nœud (liste blanche côté serveur) + signature Ed25519 des batchs sur
  l'endpoint HTTPS. Voir B-2.

### C-6. Somme de contrôle par fragment, ou repos sur le CRC BLE ?

- **Sources** : `olivier/format-trame §9`.
- **Sujet** : faut-il un champ de checksum par fragment, ou le CRC intégré au BLE
  suffit-il ? `powl` ne met pas de checksum par fragment (la signature est dans
  le paquet reconstruit).
- **Piste de résolution** : doc sécurité / spec de trame unifiée.
- **Statut** : `tranché` par transitivité (via A-12 → `powl/03` fait foi) →
  **pas de checksum par fragment**. On s'appuie sur le CRC BLE de la couche
  liaison + la signature du paquet reconstruit ; un fragment corrompu fait
  échouer la vérif de signature, le message est redemandé.

### C-7. Compteur anti-rejeu explicite par expéditeur ?

- **Sources** : `olivier/format-trame §9`, `olivier/protocole §10` ;
  `oswin/02 §3` (« numéro de séquence » dans la charge chiffrée).
- **Sujet** : faut-il numéroter les PDU d'un même expéditeur (compteur
  anti-rejeu) **en plus** de l'`id_message` ? `powl` utilise `conv_seq` (compteur
  par conversation) + `msgID` + seen-set + fenêtre timestamp ±2 h.
- **Piste de résolution** : doc sécurité. Le `conv_seq` de `powl/03 §4.1` répond
  au besoin.
- **Statut** : `tranché` par transitivité (via A-12 → `powl/03` fait foi) →
  **`conv_seq`** (compteur par conversation, dans l'`AppFrame` chiffré), combiné
  au `msgID`, au seen-set et à la fenêtre timestamp ±2 h. Pas de compteur global
  par expéditeur en clair dans l'en-tête.

### C-8. Techno serveur / page du dashboard

- **Sources** : `olivier/dashboard §11`, `olivier/etude-stack §6, §9`,
  `olivier/architecture §8`.
- **Sujet** : « décision d'Olivier » — pistes : FastAPI ou Express + SSE +
  SQLite ; page légère (vanilla ou petit framework). Recoupe **A-5**.
- **Statut** : `tranché` (Paul, 2026-09-08, à ratifier en réunion) → **backend
  FastAPI (Python)** + SSE + SQLite ; vérif de journal via le binaire Rust
  `dengon-verify` appelé en sous-processus. ThingsBoard/Node-RED et Axum/Timescale
  écartés (A-5). Framework de la page web laissé libre à qui prend le dashboard
  (vanilla ou petit framework), sans enjeu de conception.

### C-9. Réduction du périmètre de la démo ; ESP32 = PoC seulement ?

- **Sources** : `olivier/decisions-v1 §Impact du délai`,
  `olivier/architecture §9`, `olivier/mise-en-commun §5`.
- **Sujet** : `olivier` pousse fortement pour **recentrer la démo** (2 téléphones
  - 1 relais, 1-à-1, 1-2 sauts, statut jusqu'à « Distribué », chiffrement si le
  temps le permet) et garder l'**étude** complète. L'ESP32 pourrait rester au
  stade « étude + preuve de concept » si les téléphones suffisent à montrer le
  relais. `powl/10` vise au contraire un MVP matériel complet (2 Android + relais
  ESP32 + dashboard VPS + détection d'altération).
- **Piste de résolution** : réunion d'équipe, arbitrage explicite entre
  l'ambition `powl` et le pragmatisme `olivier` au regard du délai
  (soutenance 29/09/2026). Ordre de repli `olivier` : (1) hors-ligne + relais,
  (2) sécurité, (3) dashboard.
- **Statut** : `tranché` (Paul, 2026-09-08, à ratifier en réunion) → **le relais
  ESP32 fait partie du MVP** (livrable complet, pas une simple preuve de concept).
  Le MVP cible reste celui de `powl/10` : 2 Android + relais ESP32 (WROOM, A-4) +
  dashboard FastAPI (A-5) + détection d'altération (A-7). Les allègements déjà
  actés (« Lu » v2 en A-10, iOS v2 en A-11, dashboard léger en A-5, caps mémoire
  réduits en A-4) restent la marge de manœuvre ; l'ordre de repli d'`olivier`
  — (1) hors-ligne + relais, (2) sécurité, (3) dashboard — s'applique **si** le
  délai dérape, mais le relais ESP32 n'est pas dans ce qu'on sacrifie en premier.

### C-10. Réutiliser du code Meshtastic / Bridgefy, ou seulement leurs idées ?

- **Sources** : `olivier/etude-stack §9`, `olivier/mise-en-commun §4` ;
  `oswin/01 §6` (« réutiliser le code open source de Bitchat comme référence
  d'architecture ») ; `powl/01 §1.5` (bitchat/Briar cités comme références).
- **Sujet** : question de **licence et de langage**. Reprendre du code (format de
  paquet, TTL, cache de Bitchat ; managed flood routing de Meshtastic) ou
  seulement s'inspirer des concepts ?
- **Piste de résolution** : vérifier les licences (Bitchat, Meshtastic,
  Bridgefy SDK) ; décision d'équipe. `powl` part d'une implémentation propre en
  Rust inspirée des idées.
- **Statut** : `tranché` par transitivité (via A-2 → cœur Rust propre) →
  **on s'inspire des idées** (managed flood routing de Meshtastic, TTL, cache et
  format de paquet de Bitchat), **on ne reprend pas de code** (langages
  différents : Bitchat = Swift, Meshtastic = C++). Vérifier les licences reste
  une tâche si on cite/adapte un extrait précis dans le rapport, pas une décision
  ouverte.

### C-11. Doc sécurité (`securite.md`) non encore rédigée

- **Sources** : `olivier/mise-en-commun §1, §5`, `olivier/format-trame §4, §9`,
  `olivier/protocole §12`, `olivier/decisions-v1 §encore à trancher`.
- **Sujet** : `format-trame.md` et `protocole.md` **renvoient** à une doc
  sécurité qui **n'existe pas encore**. Elle doit figer : primitives crypto
  exactes, tailles de clés / nonces / tags, ordre des octets, disposition
  `nonce`/`etiquette_auth`/`contenu_chiffre`, compteur anti-rejeu, checksum de
  fragment. Côté `powl`, ce rôle est tenu par `04-security.md` (déjà écrit).
- **Piste de résolution** : à écrire (Paul ou Tanguy selon
  `mise-en-commun §5`) — ou considérer `powl/04-security.md` comme la doc
  sécurité de référence et ne rédiger qu'un delta.
- **Statut** : `tranché` sur l'approche (via A-3) → **`powl/04-security.md` est la
  doc sécurité de référence**. Ne reste qu'un **delta à rédiger** (tâche, pas
  décision) : résultat du Spike A, formules `conv_seq` / `recipient_tag` figées
  (D-2), incohérences D-1/D-4/D-5 réconciliées, mapping des colonnes chiffrées
  (B-3). Plus une rédaction from scratch.

### C-12. Méthode d'échange de clés initiale

- **Sources** : `olivier/analyse-besoins §1, §5`, `olivier/decisions-v1 §décision
  7`, `olivier/protocole §3` ; `oswin/02 §6`, `oswin/08 §8` ; `powl/04 §2.3`.
- **Sujet** : `oswin` liste « en présentiel, QR code, ou TOFU » comme options « à
  discuter dans le rapport ». `olivier/decisions-v1` a tranché **« en présentiel
  uniquement (scan QR) »** + code court comparé de visu. `powl/04 §2.3` a tranché
  **QR + code de vérification 60 chiffres + TOFU** (confiance à la première clé
  vue, alerte si changement).
- **Piste de résolution** : converger — les trois disent QR en présentiel +
  vérification par code comparé. Reste à fixer : longueur exacte du code (`powl` :
  60 chiffres = safety number Signal ; `olivier` : « code court ») et si TOFU est
  autorisé (`powl` oui, `olivier` implicitement non pour la v1 « en présentiel
  uniquement »).
- **Statut** : `tranché` par transitivité (via C-11 → `powl/04` est la doc
  sécurité de référence ; A-3 → crypto `powl` retenue) → **QR scanné en
  présentiel + code de vérification 60 chiffres (safety number, style Signal) +
  TOFU** (confiance à la première clé vue, alerte visible si la clé change).
  « En présentiel » et « QR » sont compatibles avec la position d'`olivier` ; le
  code court d'`olivier` était lié à une crypto non retenue. Longueur ajustable
  plus tard, mais 60 chiffres est la valeur par défaut figée.

---

## D. Incohérences internes `powl` à réconcilier

Ces points sont des **incohérences de rédaction** entre documents `powl`, pas des
choix de conception. À corriger dans les docs `powl` lors d'une passe de
cohérence. **A-12 rend `powl/03` (et `powl/04`/`09` pour la crypto et les
schémas) spec de référence** → ces réconciliations deviennent obligatoires, pas
optionnelles, et alimentent le delta de doc sécurité (C-11).

### D-1. Entrée du code de vérification : clés brutes ou empreintes ?

- **Sources** : `powl/01 §3.2` vs `powl/04 §2.3` vs `powl/09 §3.2`.
- **Divergence** : `01 §3.2` écrit `SHA-512(min(pubA,pubB) ‖ max(pubA,pubB))`
  (clés publiques **brutes**) ; `04 §2.3` et `09 §3.2` écrivent
  `material = SHA-512( min(fpA, fpB) ‖ max(fpA, fpB) )` avec
  `fp = SHA-256(pub_static ‖ pub_sign)` (les **empreintes**).
- **Recommandation** : retenir la version `04`/`09` (fondée sur les empreintes,
  plus détaillée et plus récente) ; corriger `01 §3.2`.
- **Statut** : `à réconcilier`.

### D-2. Définition exacte de `recipient_tag`

- **Sources** : `powl/01 §3.2` vs `powl/03 §7.3` vs `powl/04 §3.3`.
- **Divergence** : `01 §3.2` = `HMAC(pub_static_dest, jour_UTC)` ; `03 §7.3` =
  `HMAC(ma_pub_static, jour)` ; `04 §3.3` (le plus précis) =
  `HMAC-SHA256( pub_static_destinataire , "dengon-tag" ‖ day_u32 )[0..16]`.
- **Recommandation** : retenir `04 §3.3` (clé = clé statique **du destinataire**,
  chaîne de séparation de domaine `"dengon-tag"`, `day_u32`, troncature 16 o) ;
  aligner `01` et `03`.
- **Statut** : `à réconcilier`.

### D-3. Formule de `msgID`

- **Sources** : `powl/01 §1.2` (informel : `hash(sender, ts, payload)`) vs
  `powl/03 §3.2` (autoritatif : `SHA-256( sender_id ‖ timestamp_ms ‖ type ‖
  payload )`).
- **Recommandation** : `03 §3.2` fait foi (inclut `type`).
- **Statut** : `tranché` par transitivité (via A-12 → `powl/03` fait foi) →
  `msgID = SHA-256( sender_id ‖ timestamp_ms ‖ type ‖ payload )`. Reste à
  corriger la formulation informelle de `01 §1.2`.

### D-4. Enum `EVENTS.integrity`

- **Sources** : `powl/07 §4` (erDiagram : `ok|broken|fork|unverified`) vs
  `powl/09 §2` (SQL CHECK : `ok|broken|fork|gap|unverified|rejected_sig`).
- **Recommandation** : le SQL de `09` est le plus complet (ajoute `gap` et
  `rejected_sig`) ; corriger l'erDiagram de `07`.
- **Statut** : `à réconcilier`.

### D-5. Enum `NODES.status`

- **Sources** : `powl/07 §4` (erDiagram : `online|stale|suspect`) vs `powl/09 §2`
  (SQL : `online|stale|suspect|quarantined`).
- **Recommandation** : le SQL de `09` fait foi (ajoute `quarantined`, cohérent
  avec « `node` inconnu → quarantaine » de `07 §3.3`) ; corriger l'erDiagram.
- **Statut** : `à réconcilier`.

---

## E. « Avis d'Olivier » à valider en équipe

Ces points sont des **propositions d'Olivier** (`decisions-v1.md` §points
d'équipe + produit/UX), explicitement « décision finale à prendre à trois ».
Beaucoup recoupent des choix déjà faits ou implicites dans `powl` ; ils restent
listés ici parce que l'équipe (Paul, Tanguy, Olivier) ne les a pas encore
formellement validés ensemble.

| Sujet | Avis d'Olivier | Position `powl` / remarque | Statut |
| --- | --- | --- | --- |
| Scénario d'usage à privilégier | **Campus / bâtiment** (zone moyenne, téléphones + quelques relais fixes) | `powl/00 §1` : mêmes cas d'usage génériques (zone blanche, manif, festival). Le scénario campus est un bon terrain de test. | à valider |
| Durée de conservation d'un message non délivré | **≈ 24 h** | `powl` : `MSG_TTL_S = 86 400` (24 h). **Aligné.** | à valider (aligné) |
| Techno de l'app mobile | **Multiplateforme (Flutter ou React Native)** | `powl` + orientation proposée (**A-1**) : Kotlin natif + cœur Rust partagé, Flutter/RN écartés (le multiplateforme ne mutualise pas la couche BLE). | à confirmer (voir A-1) |
| Hébergement du dashboard | **VPS Debian de l'équipe** | `powl` : VPS (déjà possédé), Docker Compose. **Aligné.** | à valider (aligné) |
| Répartition du travail | **Par composant** (un mobile, un ESP32, un dashboard), conception commune | `oswin/08 §6` : mêmes rôles. **Aligné.** | à valider |
| Taille de la démo finale | **5-8 appareils** | `powl/11 §6` : scénario densité 8-10 appareils. Cohérent. | à valider |
| Type de contenu d'un message (v1) | **Texte court ~140-500 caractères** | `powl/00 §5` : « texte court < 1 Ko utile » ; `olivier/format-trame` : ≤ ~500 caractères. Cohérent. | à valider |
| Identité d'un utilisateur | **Pseudo libre, sans vérification** (l'identité réelle = la clé échangée en personne) | `powl/04 §2.2` : pseudo dans le QR, non utilisé pour l'adressage. **Aligné.** | à valider (aligné) |
| Fonctionnement en arrière-plan | **Service de fond avec notification permanente (Android)** | `powl/01 §2`, `olivier/etude-stack §1.3` : incontournable sur Android 14/15. **Aligné.** | à valider (aligné) |
| Conservation des messages sur le téléphone | **Jusqu'à suppression manuelle** | `powl/09 §4` : « jusqu'à suppression manuelle ». **Aligné.** | à valider (aligné) |
| Info réseau montrée à l'utilisateur | **Indicateur simple « Connecté au réseau » / « Isolé »** | `powl/10 Lot 4` : « écran réseau (pairs visibles) » (un peu plus riche). | à valider |
| Langue de l'interface (v1) | **Français uniquement** | `CLAUDE.md` : documentation et commentaires en français. Cohérent. | à valider |
| Relais par un téléphone sans contact | **Oui, toujours** — participe dès l'installation | `powl/00 §7` : offline-first, tout nœud relaie. **Aligné.** | à valider (aligné) |
| Réglage de consommation batterie | **Oui, dès la v1 : mode « économie » activable** | `powl` ne mentionne pas de mode éco explicite ; `olivier/architecture §4` le place dans la couche Application. | à valider |
| Vérification lors de l'échange de clés | **Oui : code court identique à comparer de visu après le scan** | `powl/04 §2.3` : code 60 chiffres (safety number Signal). Voir **C-12** (longueur exacte). | à confirmer |
| Nom du projet / de l'appli | **À rediscuter** en équipe (« dengon » = nom de travail) | — | ouvert |
| Message expiré (~24 h sans livraison) | Statut **« Échec »** dans la conversation, sans notification | `powl/05 §1` : `EXPIRED` (« Échec »), UI « non remis » + bouton renvoyer. **Aligné.** | à valider (aligné) |
| Relance d'un message non parti | **Bouton « Renvoyer »** déclenché par l'utilisateur | `powl/05 §7` : « UI non remis + bouton renvoyer ». **Aligné.** | à valider (aligné) |
| Blocage d'un contact indésirable | **Reporté** après la v1 | `powl/09 §1` : colonne `contacts.blocked` prévue (schéma prêt, UI reportée). | à valider |
| Dashboard : rafraîchissement | **Mise à jour automatique** (statuts en direct) | Tranché via A-5 → **SSE** (pas de WebSocket). Principe aligné. | à valider (aligné) |
| Simulateur de réseau | **Léger** : quelques scripts de test, pas un vrai simulateur | A-2 (cœur Rust) rend `dengon-sim` peu coûteux : il pilote le même `dengon-core` sans radio → tests rejouables des scénarios routage/dédup/partition. Recommandé plutôt que des scripts jetables. | à confirmer (lean `dengon-sim`) |
| Livrables | Spec protocole, doc sécurité, doc architecture, **+ rapport écrit + support de présentation** | `docs/suivi/06-support-oral.md` prévoit déjà le support de soutenance. | à valider |
| Rythme de mise en commun | **Fusion après la phase de conception (~fin semaine 1)**, puis travail commun ; points 2-3×/semaine | `docs/suivi/` existe déjà comme dépôt commun de suivi. | à valider |
