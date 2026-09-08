# Étude de la stack technique — dengon (brouillon v0.1)

> Rédigé le 2026-09-08, à partir de recherches web (sources en fin de document).
> À relire et trancher avec Paul et Tanguy. S'appuie sur `protocole.md`,
> `architecture.md`, `dashboard.md`, `decisions-v1.md`.
> Rappel : décision finale de la stack = choix d'équipe ; ce document est une
> base de discussion.

---

## 0. Résumé pour les pressés

- **Le point dur, c'est le Bluetooth de l'app mobile**, pas le reste.
  Un téléphone dans le maillage doit jouer **deux rôles en même temps** :
  *chercheur* (scanner les voisins) **et** *visible/serveur* (s'annoncer et
  accepter les connexions). Beaucoup de bibliothèques ne font que le premier.
- Les bibliothèques BLE les plus connues en multiplateforme
  (**flutter_blue_plus**, **react-native-ble-plx**) sont **« chercheur »
  uniquement** → inutilisables telles quelles pour du téléphone-à-téléphone.
- Côté **Flutter**, une bibliothèque, **`bluetooth_low_energy`**, fait les
  **deux rôles** sur Android et iOS. C'est la seule option multiplateforme
  « prête » qui coche la case.
- Côté **React Native**, il faut assembler plusieurs bibliothèques dont
  certaines **non maintenues** → plus fragile.
- **Natif Android (Kotlin)** fait tout, de façon fiable, mais le code ne
  resservira pas pour l'iPhone.
- Recommandation : **prototype « hello mesh » en semaine 1** pour choisir en
  connaissance de cause (voir §7).

## 1. Application mobile

### 1.1 Ce que dengon exige de la couche Bluetooth

| Besoin | Détail |
|--------|--------|
| Rôle **chercheur** (*central*) | Scanner en continu, se connecter à un voisin, lire/écrire. |
| Rôle **visible + serveur** (*peripheral* + *GATT server*) | S'annoncer avec l'identifiant de service dengon, accepter les connexions entrantes, exposer les caractéristiques d'échange de trames. |
| Les **deux à la fois** | Un nœud relaie dans les deux sens ; il est simultanément l'un et l'autre. |
| **Arrière-plan** | Continuer à relayer quand l'app n'est pas au premier plan. |

### 1.2 Bibliothèques comparées

| Option | Rôle chercheur | Rôle visible/serveur | Arrière-plan | Maintenue | Remarque |
|--------|:---:|:---:|:---:|:---:|----------|
| **Flutter · flutter_blue_plus** | ✅ | ❌ | partiel (chercheur) | ✅ très active | La plus populaire, mais *central only*. |
| **Flutter · bluetooth_low_energy** | ✅ | ✅ (Android/iOS/Windows) | oui, avec config | ✅ | **Seule option multiplateforme couvrant les deux rôles.** Moins éprouvée que flutter_blue_plus. |
| **Flutter · ble_peripheral** | ❌ | ✅ | — | ✅ | Complément « serveur » si on garde flutter_blue_plus pour le reste (assemblage). |
| **React Native · react-native-ble-plx** | ✅ | ❌ | partiel (chercheur) | ✅ | *« ne permet pas la communication entre téléphones »* (doc du projet). |
| **React Native · react-native-peripheral** | ❌ | ✅ | — | ⚠️ petit projet | Pour le rôle serveur ; à combiner avec ble-plx. |
| **React Native · react-native-ble-advertiser** | ❌ | annonce seule | — | ❌ ~4 ans sans MAJ | À éviter. |
| **Natif Android · API Kotlin (`android.bluetooth.le`) / bibliothèque Nordic** | ✅ | ✅ | contrôle total du service de fond | ✅ | Le plus fiable ; non réutilisable iOS. |

### 1.3 Contraintes d'arrière-plan sur Android récent

- **Android 14** : un service de fond qui fait du BLE doit déclarer
  `foregroundServiceType="connectedDevice"` **et** la permission
  `FOREGROUND_SERVICE_CONNECTED_DEVICE`, sinon il est tué.
- **Android 15** : restrictions supplémentaires sur le **scan BLE en fond** ;
  le lancement direct d'un service de fond est bloqué sans exemption ; en
  **Doze profond** (écran éteint, appareil immobile), les scans peuvent être
  différés et les connexions coupées.
- Conséquences pour dengon :
  - la **notification permanente** est incontournable (déjà acté dans
    `decisions-v1.md`) ;
  - un téléphone **posé, écran éteint**, relaie mal → le **mode éco** et les
    **relais ESP32 fixes** prennent tout leur sens ;
  - pour la **démo**, garder les écrans allumés / appareils en main.

### 1.4 Pour information : pourquoi l'iPhone est reporté

En arrière-plan, un iPhone en rôle « visible » **n'annonce plus son nom** et
place tous ses identifiants de service dans une **« zone de débordement »**
(*overflow area*). Ces services ne sont **découvrables que par un autre appareil
Apple qui scanne explicitement cet identifiant précis**. Un ESP32 ou un Android
risquent donc de **ne pas voir** un iPhone en arrière-plan. C'est un
comportement **système, non contournable** — d'où le report d'iOS après la v1.

## 2. Firmware ESP32

### 2.1 Pile Bluetooth

ESP-IDF propose deux piles BLE :

| Pile | RAM | Flash | Usage |
|------|-----|-------|-------|
| **Bluedroid** | élevée | élevée | complète (BLE + Bluetooth Classic). |
| **NimBLE** | **~40 à ~100 Ko de moins** | **~50 % de moins** | BLE seul ; recommandée pour un **nœud relais contraint**. |

→ **NimBLE** pour dengon (on ne fait que du BLE, la carte est limitée).
`NimBLE-Arduino` offre le même gain si on reste sous l'IDE Arduino.

### 2.2 ESP-IDF vs cœur Arduino

- **ESP-IDF** (C) : recommandé pour du maillage « sérieux », contrôle fin de la
  mémoire.
- **Cœur Arduino** : plus rapide à prototyper, communauté large.
- Compromis possible : prototypage Arduino + NimBLE-Arduino, puis bascule
  ESP-IDF si besoin.

### 2.3 Faut-il utiliser le « BLE Mesh » natif de l'ESP32 ?

L'ESP32 embarque **ESP-BLE-MESH** (basé sur la pile Zephyr) : il gère déjà
*provisioning*, relais, *low power*, *friend*. **Mais** c'est la **norme
Bluetooth Mesh** du SIG : modèle *publish/subscribe*, sécurité par clés
« réseau » et « application » partagées. Ce **n'est pas** le modèle de dengon
(1-à-1, chiffré de bout en bout, accusés qui remontent vers l'expéditeur).

→ L'adopter reviendrait à adopter **tout son modèle**. Pour un **protocole
maison**, il est plus cohérent de partir de **NimBLE « brut »** (GATT +
annonce) et de **s'inspirer** des idées de Bluetooth Mesh et de Meshtastic
(ci-dessous) sans en prendre la tuyauterie.

## 3. Le « moteur dengon » : une implémentation ou deux ?

Le moteur = la logique de `protocole.md` §6 (circulation, TTL, déduplication,
file, réassemblage, accusés). Il tourne côté téléphone **et** côté ESP32.

| Option | Principe | Pour | Contre |
|--------|----------|------|--------|
| **A — deux implémentations** | Une version dans le langage du téléphone, une en C/C++ pour l'ESP32. | Simple à démarrer, pas d'outillage. | Risque de **dérive** entre les deux au fil des évolutions. |
| **B — noyau partagé en Rust** | Un cœur écrit une fois en Rust, compilé en bibliothèque native, appelé depuis le téléphone (via **UniFFI**, qui génère les liaisons) et depuis l'ESP32 (bibliothèque C). Approche éprouvée (Firefox mobile). | Une seule source de vérité. | **Outillage à monter** ; courbe d'apprentissage ; lourd pour 3 semaines. |

→ **Recommandation v1 : Option A**, avec en garde-fou une **spécification du
format binaire des trames** très précise (le format fait foi, pas le code).
Garder **l'Option B comme évolution** post-soutenance.

## 4. Cryptographie

| Cible | Recommandation | Détail |
|-------|----------------|--------|
| **Téléphone** | **libsodium** via `sodium_libs` / `flutter_sodium` (Flutter) ou binding équivalent (RN) | `crypto_box` = **X25519 + chiffrement authentifié** : exactement le besoin (contenu illisible **et** non modifiable, clés de longue durée échangées en personne). |
| **ESP32** | **mbedTLS** (déjà fourni avec l'ESP32, accélération matérielle AES) | L'ESP32 **ne déchiffre pas** le contenu (il relaie). Il a surtout besoin de **hacher** (empreintes, table « déjà vu ») et éventuellement **vérifier une signature** sur les accusés. libsodium a aussi un portage ESP-IDF si on veut le même outil des deux côtés. |

- Pas de **secret persistant** (*forward secrecy*) en v1 — assumé dans
  `decisions-v1.md` et `protocole.md` §10.
- Le **code court à comparer** lors de l'ajout de contact (`protocole.md` §3)
  se calcule par un hachage des deux clés publiques — primitives déjà couvertes
  par libsodium.

## 5. Inspiration protocole : Meshtastic

Meshtastic utilise du **« managed flood routing »**, très proche de notre §6 :

- **limite de sauts = 7** (et non « infini ») ;
- un nœud **ne rediffuse que si** la limite de sauts n'est pas nulle **et**
  qu'il n'a **pas déjà entendu** ce paquet ;
- **« écouter avant de rediffuser »** : le nœud attend un court instant ; si un
  voisin est déjà en train de rediffuser, il **s'abstient** → évite les
  tempêtes de rediffusion ;
- la fenêtre d'attente dépend de la qualité du signal : les nœuds **lointains**
  rediffusent en premier, les **proches** se taisent alors (propagation plus
  efficace).

→ **Impact sur `protocole.md`** : aligner la limite de sauts de départ sur
**7** (au lieu de 8), et ajouter **« écouter un court instant avant de
rediffuser »** comme règle anti-tempête. (Ajusté dans la v0.2 du protocole.)

## 6. Serveur du dashboard (VPS Debian)

Rien d'exotique : petite **API HTTP** + **base** + **canal temps réel** + **page
statique**.

| Élément | Options | Reco |
|---------|---------|------|
| API + logique | Node/Express, Python/FastAPI, Go | Au choix de qui prend le dashboard (**Olivier**) ; **FastAPI** ou **Express** conviennent. |
| Base | SQLite, PostgreSQL | **SQLite** suffit pour le volume d'une démo. |
| Temps réel | WebSocket, SSE (*Server-Sent Events*) | **SSE** : sens unique serveur → page, plus simple, suffisant ici. |
| Page | Page légère (vanilla ou petit framework) | Affichage de la liste + du parcours (voir `dashboard.md` §9). |

## 7. Recommandation par brique (base de discussion d'équipe)

| Brique | Recommandation v1 | Repli / alternative |
|--------|-------------------|---------------------|
| **App mobile** | **Flutter + `bluetooth_low_energy`** si on tient à viser l'iPhone plus tard ; **natif Kotlin** si on privilégie la fiabilité BLE maximale | Basculer sur Kotlin natif si `bluetooth_low_energy` déçoit à l'essai |
| **Validation techno** | **Prototype « hello mesh » en semaine 1** : 2 téléphones + 1 relais, 1 message qui passe. Décision go / no-go sur la techno mobile. | — |
| **Moteur dengon** | **Deux implémentations** (téléphone + ESP32) encadrées par une spec de trame binaire stricte | Noyau **Rust + UniFFI** en v2 |
| **Firmware ESP32** | **ESP-IDF + NimBLE**, GATT + annonce « bruts » | **Arduino + NimBLE-Arduino** pour prototyper |
| **Crypto téléphone** | **libsodium** (`crypto_box`) | — |
| **Crypto ESP32** | **mbedTLS** (déjà présent) | libsodium porté ESP-IDF |
| **Serveur dashboard** | **FastAPI ou Express + SSE + SQLite** sur le VPS | Page rafraîchie à la main / maquette |

## 8. Risques techniques principaux

1. **Double rôle BLE en arrière-plan sur Android 14/15** : notification
   obligatoire, Doze, scan restreint écran éteint. → mode éco + relais ESP32
   fixes + démo écrans allumés.
2. **`bluetooth_low_energy` moins éprouvée** que flutter_blue_plus → à valider
   par le prototype de la semaine 1 ; sinon repli Kotlin natif.
3. **Tenir deux implémentations cohérentes** du moteur en 3 semaines → spec de
   trame stricte, périmètre réduit.
4. **VPS pas prêt** le jour J → repli serveur local ou maquette.
5. **React Native** : chemin périphérique = assemblage de bibliothèques dont
   certaines peu maintenues → si multiplateforme, **préférer Flutter**.

## 9. Points ouverts

- Choix final app mobile : Flutter (`bluetooth_low_energy`) vs Kotlin natif —
  à décider **après** le prototype semaine 1.
- Qui prend quelle brique (voir répartition « par composant » dans
  `decisions-v1.md`).
- Faut-il tenter le noyau Rust partagé, ou l'assumer explicitement comme
  « hors v1 » ?
- Techno précise du serveur dashboard (décision d'Olivier).
- Réutilise-t-on des morceaux de Meshtastic / Bridgefy (licence, langage), ou
  seulement leurs idées ?

## Sources

- flutter_blue_plus — https://pub.dev/packages/flutter_blue_plus et https://github.com/chipweinberger/flutter_blue_plus
- bluetooth_low_energy (Flutter, central + peripheral) — https://pub.dev/packages/bluetooth_low_energy
- ble_peripheral (Flutter) — https://pub.dev/packages/ble_peripheral
- react-native-ble-plx (central only, pas de communication téléphone-à-téléphone) — https://github.com/dotintent/react-native-ble-plx/blob/master/README.md et https://www.npmjs.com/package/react-native-ble-plx
- react-native-ble-plx — mode arrière-plan iOS — https://github.com/dotintent/react-native-ble-plx/wiki/Background-mode-(iOS)
- react-native-peripheral — https://github.com/petrbela/react-native-peripheral
- react-native-ble-advertiser (non maintenu) — https://www.npmjs.com/package/react-native-ble-advertiser
- iOS « overflow area » pour l'annonce en arrière-plan — https://github.com/davidgyoung/ios-overflow-area et https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/CoreBluetoothBackgroundProcessingForIOSApps/PerformingTasksWhileYourAppIsInTheBackground.html
- Android 14 — types de services de fond obligatoires — https://developer.android.com/about/versions/14/changes/fgs-types-required et https://developer.android.com/develop/background-work/services/fgs/service-types
- Android 15 — restrictions scan BLE en arrière-plan — https://bleadvertiserapp.medium.com/android-15-broke-your-ble-app-new-permission-rules-3d8cb3c9ba86
- Android — communiquer en arrière-plan (doc officielle) — https://developer.android.com/develop/connectivity/bluetooth/ble/background
- ESP-IDF — NimBLE vs Bluedroid (empreinte mémoire) — https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/ble/overview.html
- NimBLE-Arduino (empreinte réduite) — https://osrtos.com/projects/nimble-arduino/
- ESP-BLE-MESH (norme Bluetooth Mesh sur ESP32) — https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/esp-ble-mesh/ble-mesh-index.html
- Meshtastic — « managed flood routing » — https://meshtastic.org/blog/why-meshtastic-uses-managed-flood-routing/ et https://meshtastic.org/docs/overview/mesh-algo/
- flutter_sodium / libsodium pour Flutter — https://github.com/firstfloorsoftware/flutter_sodium
- Rust partagé mobile via UniFFI — https://github.com/ivnsch/rust_android_ios et https://appstimes.in/rust-in-mobile-development-2026-is-it-ready-to-replace-kotlin-and-swift-for-core-logic/
- Briar (audit Cure53, BLE + Wi-Fi Direct + Tor) — https://havenmessenger.com/blog/posts/mesh-networking-briar/
- Bridgefy SDK Android — https://github.com/bridgefy/sdk-android
