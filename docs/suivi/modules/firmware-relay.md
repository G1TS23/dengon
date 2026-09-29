# Module : `firmware-relay` (`firmware/dengon-relay/`)

**Rôle en une phrase :** le firmware du relais ESP32 : un nœud dengon (routage,
réconciliation d'inventaire, courrier d'enveloppes, journal chaîné, logique de
`dengon-core` liée en `libdengon_core.a`, US-308). Il tourne au-dessus d'un
transport BLE NimBLE à rôle double qui échange des **octets opaques**, conforme
au contrat `Transport` d'US-105 (US-220), sur le squelette d'US-114.
**Correspond à la conception :** [`docs/synthese/08-relais-esp32.md`](../../synthese/08-relais-esp32.md)
§2-4, [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md)
§3 et §5, [`docs/powl/03-network-protocol.md`](../../powl/03-network-protocol.md)
§2 et §6 (UUID, rôle double, anti-boucle, MTU) — décision C-2 ; contrat
[`crates/dengon-ble/src/transport.rs`](../../../crates/dengon-ble/src/transport.rs).
**Dernière mise à jour :** 2026-09-29
**État :** partiel.
- Transport BLE : complet ; testé sur l'hôte, sur carte (Unity) et en rôle
  périphérique contre un téléphone.
- Relais dengon (US-308, branche empilée sur la PR #108 non mergée) :
  vérifié sur **une** carte (Unity, redémarrages, `dengon-verify`,
  téléphone).
- L'essai sur 2 cartes n'a jamais été fait (ni US-220, ni US-308).
- Export vers le dashboard (US-309, branche `feat/US-309-https-dashboard`
  empilée sur US-308) : codé, compilé, cœur testé sur l'hôte, batch accepté
  par le dashboard lancé en local. **Jamais exécuté sur carte ni contre le
  VPS** (racine CA du VPS à récupérer, voir `main/certs/README.md`).

---

## Onboarding `firmware/` — à lire avant de toucher au code

C'est la note d'onboarding exigée par le critère d'acceptation n°4 de l'US-114.
Elle s'adresse à quelqu'un qui n'a jamais compilé pour ESP32.

### Chaîne de compilation : rien à installer

Le firmware se compile dans l'**image Docker officielle d'Espressif**, épinglée
par digest. Personne n'installe ESP-IDF sur sa machine : la chaîne pèse ~2 Go
d'outils, et l'épingler garantit que la CI et le poste de travail compilent avec
exactement le même compilateur.

```bash
IDF=espressif/idf:v5.5.5@sha256:a9231d0697ab8f7517cc072e93b7c83e04907bfbfba80b6440d7dbbf90665cf2
```

Le digest est celui de l'**index** multi-architecture, pas celui d'un manifeste
de plateforme. Pour le rafraîchir lors d'une montée de version :

```bash
docker buildx imagetools inspect espressif/idf:v5.5.5 --format '{{.Manifest.Digest}}'
```

⚠ `docker manifest inspect --verbose` affiche d'abord l'entrée `linux/amd64` :
épingler *ce* digest-là ferait échouer le build sur une machine arm64.

### Compiler

Toutes les commandes se lancent **depuis la racine du dépôt** :

```bash
# 0. libdengon_core.a (Rust, xtensa) — AVANT tout build du firmware (US-308).
#    Une fois : `espup install --targets esp32` (toolchain `esp`, ~2 Go).
firmware/dengon-relay/tools/build_core.sh

# Compiler
docker run --rm -it -u "$(id -u):$(id -g)" -e HOME=/tmp \
  -v "$PWD:/repo" -w /repo/firmware/dengon-relay "$IDF" idf.py build

# Repartir de zéro — OBLIGATOIRE après toute modification de sdkconfig.defaults
docker run --rm -it -u "$(id -u):$(id -g)" -e HOME=/tmp \
  -v "$PWD:/repo" -w /repo/firmware/dengon-relay "$IDF" idf.py fullclean

# Empreinte mémoire
docker run --rm -it -u "$(id -u):$(id -g)" -e HOME=/tmp \
  -v "$PWD:/repo" -w /repo/firmware/dengon-relay "$IDF" idf.py size

# Tests Unity du cœur du transport, sur l'HÔTE (cible linux) — comme la CI
docker run --rm -it -u "$(id -u):$(id -g)" -e HOME=/tmp -v "$PWD:/repo" \
  -w /repo/firmware/dengon-relay/components/dengon_transport_core/test_apps "$IDF" \
  sh -ec 'idf.py --preview set-target linux && idf.py build && ./build/test_dengon_transport_core.elf'

# Les MÊMES tests, sur une carte (« tests Unity sur cible »)
docker run --rm -it -u "$(id -u):$(id -g)" -e HOME=/tmp --device=/dev/ttyUSB0 -v "$PWD:/repo" \
  -w /repo/firmware/dengon-relay/components/dengon_transport_core/test_apps "$IDF" \
  idf.py -B build-esp32 -D SDKCONFIG=build-esp32/sdkconfig set-target esp32 build \
         -p /dev/ttyUSB0 flash monitor

# Tests Unity du Store (US-308), sur l'HÔTE : journal sur fichier seulement
docker run --rm -it -u "$(id -u):$(id -g)" -e HOME=/tmp -v "$PWD:/repo" \
  -w /repo/firmware/dengon-relay/components/dengon_store/test_apps "$IDF" \
  sh -ec 'idf.py --preview set-target linux && idf.py build && ./build/test_dengon_store.elf'

# Les mêmes + NVS/littlefs réels, sur une carte de TEST (efface l'identité !)
docker run --rm -it -u "$(id -u):$(id -g)" -e HOME=/tmp --device=/dev/ttyUSB0 -v "$PWD:/repo" \
  -w /repo/firmware/dengon-relay/components/dengon_store/test_apps "$IDF" \
  idf.py -B build-esp32 -D SDKCONFIG=build-esp32/sdkconfig set-target esp32 build \
         -p /dev/ttyUSB0 flash monitor

# Tests Unity de l'export (US-309), sur l'HÔTE : buffer ring + politique de réessai
docker run --rm -it -u "$(id -u):$(id -g)" -e HOME=/tmp -v "$PWD:/repo" \
  -w /repo/firmware/dengon-relay/components/dengon_ship_core/test_apps "$IDF" \
  sh -ec 'idf.py --preview set-target linux && idf.py build && ./build/test_dengon_ship_core.elf'

# Explorer la configuration (écrit dans sdkconfig, PAS dans sdkconfig.defaults)
docker run --rm -it -u "$(id -u):$(id -g)" -e HOME=/tmp \
  -v "$PWD:/repo" -w /repo/firmware/dengon-relay "$IDF" idf.py menuconfig
```

`-u "$(id -u):$(id -g)"` et `-e HOME=/tmp` ne sont pas décoratifs : voir le
piège n°2 plus bas.

⚠ **US-308 change la table de partitions** (`partitions.csv`) : une carte
flashée avant doit être effacée une fois (`idf.py erase-flash flash`). Sinon
NVS et littlefs gardent l'ancien découpage. Et comme toujours après un
changement de `sdkconfig.defaults` : `idf.py fullclean`.

### Rendre la carte visible (WSL2)

Sous WSL2, un périphérique USB branché sur Windows **n'existe pas** côté Linux
tant qu'on ne l'y a pas explicitement rattaché. Côté **Windows**, dans un
PowerShell **administrateur** :

```powershell
winget install --exact dorssel.usbipd-win     # une seule fois
usbipd list                                    # repérer le BUSID du « CP2102 USB to UART Bridge »
usbipd bind   --busid <BUSID>                  # une seule fois par port physique
usbipd attach --wsl --busid <BUSID>            # à REFAIRE après chaque débranchement
```

Puis côté **WSL** :

```bash
sudo modprobe cp210x          # ou ftdi_sio / ch341 selon la puce USB-UART de la carte
ls -l /dev/ttyUSB0            # doit exister, groupe `dialout`
sudo usermod -aG dialout "$USER"   # une seule fois, puis `wsl --shutdown` côté Windows
```

### Flasher et observer

```bash
docker run --rm -it -u "$(id -u):$(id -g)" -e HOME=/tmp \
  --device=/dev/ttyUSB0 \
  -v "$PWD:/repo" -w /repo/firmware/dengon-relay \
  "$IDF" idf.py -p /dev/ttyUSB0 -b 460800 flash monitor
```

- `-it` est **obligatoire** pour `monitor` : sans TTY, la sortie reste muette.
- Quitter le moniteur : `Ctrl-]`.
- Si le bootloader refuse de démarrer (*Failed to connect … Wrong boot mode*) :
  maintenir **BOOT**, appuyer puis relâcher **EN**, relâcher **BOOT**, relancer.

**Repli sans WSL** : le workflow `firmware` publie les binaires en artefact. On
peut donc flasher depuis Windows sans rejouer le build, avec
`pip install esptool` et les offsets lus dans `build/flash_args`
(`0x1000` bootloader, `0x8000` table de partitions, `0x10000` application).

### Les 3 pièges

1. **WSL2 ne voit aucun port série, et l'erreur ne le dit pas.** `idf.py flash`
   répond `No serial ports found`, ce qui fait soupçonner un câble mort ou une
   carte grillée. Il y a en réalité **trois conditions indépendantes**, avec
   trois symptômes différents : `usbipd bind` + `attach` (sinon rien),
   `modprobe cp210x` (sinon rien non plus), et l'appartenance au groupe
   `dialout` (sinon `Permission denied`). Et l'`attach` est à refaire **après
   chaque débranchement et chaque `wsl --shutdown`**.

2. **Docker en root pourrit l'arbre de travail.** Sans `-u "$(id -u):$(id -g)"`,
   `build/` et `sdkconfig` appartiennent à `root` : `git status` devient
   inutilisable, le build suivant échoue en écriture, et `rm -rf build` réclame
   `sudo`. Le corollaire `-e HOME=/tmp` est nécessaire parce que l'uid injecté
   n'existe pas dans le `/etc/passwd` de l'image — il n'a donc pas de home, et
   le gestionnaire de composants d'ESP-IDF en exige un.

3. **`sdkconfig.defaults` n'est lu que si `sdkconfig` n'existe pas.** On corrige
   une ligne, on relance le build, **rien ne change** — sans le moindre
   avertissement. C'est le piège n°1 de tout projet ESP-IDF. Remède :
   `idf.py fullclean`. Corollaire plus vicieux : `idf.py menuconfig` écrit dans
   `sdkconfig`, qui est dans le `.gitignore`. Un réglage fait en menuconfig et
   non reporté dans `sdkconfig.defaults` marche sur votre machine, et nulle part
   ailleurs — CI comprise.

### Les autres pièges, pour mémoire

- **`BLE_UUID128_INIT` attend du little-endian.** Recopier un UUID dans le sens
  de la lecture compile, link et tourne — en annonçant un UUID inversé. Voir
  [`04-apprentissages.md`](../04-apprentissages.md).
- **Le paquet d'annonce est limité à 31 octets**, et le dépassement se manifeste
  par un `BLE_HS_EMSGSIZE` (`0x0C`) **à l'exécution**, pas à la compilation.
- **Bluedroid est le host Bluetooth par défaut** d'ESP-IDF. Activer NimBLE sans
  le désactiver donne `host/ble_hs.h: No such file or directory`, message qui ne
  mentionne jamais Bluedroid.
- **NVS doit être initialisée avant le Bluetooth** : le contrôleur y range sa
  calibration PHY.
- **`nimble_port_init()` n'allume pas le host.** Sans
  `nimble_port_freertos_init()`, le firmware boote, n'affiche aucune erreur, et
  n'annonce rien.
- **Un périphérique NimBLE cesse d'annoncer dès qu'il est connecté.** Sans
  réarmement dans `BLE_GAP_EVENT_DISCONNECT`, la carte disparaît du scanner
  après la première connexion.
- **Ne jamais déclarer le CCCD (`0x2902`) à la main** : NimBLE l'ajoute
  automatiquement derrière toute caractéristique notifiable.
- **`assert(rc == 0)` + `-Werror` casse le build release** : avec `NDEBUG`,
  `assert` disparaît, `rc` devient une variable inutilisée. D'où les
  `if (rc != 0) { ESP_LOGE(...); return; }` explicites.
- **`conn_handle` ≠ `LinkId`.** NimBLE redonne un `conn_handle` dès qu'il est
  libéré ; un `LinkId` ne doit jamais l'être. Les confondre attribuerait une
  trame en retard au pair suivant (voir `04-apprentissages.md`).
- **Scan « pour toujours » + filtre de doublons = pair revenu invisible.** Le
  scan tourne par passes de 10 s, relancées à `DISC_COMPLETE`.
- **`ble_gap_connect()` exige que le scan soit arrêté** (`BLE_HS_EBUSY` sinon) :
  `ble_gap_disc_cancel()` d'abord, relance du scan après la connexion.
- **`captures/` est dans le `.gitignore`** du dépôt : une capture d'écran rangée
  là disparaîtrait en silence. Les captures vont dans `docs/suivi/modules/img/`.

---

## À quoi ça sert

Le relais ESP32 est la partie « infrastructure » de dengon : une carte sur
secteur, posée en hauteur, toujours allumée, qui relaie les messages des
téléphones alentour, garde les enveloppes destinées aux absents et remonte ses
journaux au dashboard.

Depuis l'US-220, la carte sait **parler BLE avec ses voisins dans les deux
sens** : elle s'annonce, elle scanne, elle se connecte aux autres nœuds dengon
(ou accepte leurs connexions), et elle échange avec eux des trames d'octets.
Le transport **ne comprend pas** ce qu'il transporte : c'est voulu.

Depuis l'US-308, la logique vient de `dengon-core` (Rust, module `relay`), lié
en `libdengon_core.a`. Le relais :
- décode chaque trame et la déduplique ;
- relaie ce que l'émetteur autorise (`RELAY_OK`, TTL − 1) ;
- échange son inventaire avec chaque voisin identifié et lui pousse ce qui
  lui manque ;
- garde les enveloppes scellées et les remet à qui les réclame ;
- consigne tout dans un journal chaîné et signé, écrit sur la flash
  (littlefs), qui survit aux redémarrages.

Le C ne garde que la radio, le stockage et l'ordonnancement (4 tâches
FreeRTOS).

## Structure

```
firmware/dengon-relay/
  CMakeLists.txt        — projet ESP-IDF ; son `project()` nomme les artefacts
  sdkconfig.defaults    — LA configuration versionnée (host, 4 rôles, MTU, partitions)
  README.md             — pointeur vers cette fiche
  main/                 — l'application : tout ce qui touche NimBLE
    CMakeLists.txt      — composant `main` ; -Wall -Wextra -Werror sur NOTRE code
    Kconfig.projbuild   — menu « dengon — relais » : démo on/off, période
    main.c              — app_main : NVS, relais (identité, journal), transport,
                          tâches, console — ou la démo US-220 si activée
    dengon_relay_app.{h,c} — US-308 : handle dengon-core sous mutex, tâches
                          route / inventory / courier / ledger, relay.boot,
                          auto-test Noise
    dengon_console.{h,c}— US-308 : commandes série `ledger`, `restart`, `relay`
    dengon_transport.h  — API C publique = miroir du trait Rust `Transport`
    transport_nimble.c  — glue NimBLE : annonce, scan, connexion, découverte
                          GATT, abonnement, émission/réception, événements GAP
    dengon_gatt.{h,c}   — les 3 UUID et la table GATT ; les écritures sur
                          CHAR_RX partent au transport
    dengon_peer_id.{h,c}— peerID en cache : le vrai (US-308, `_set`) ou le
                          bouchon SHA-256 de la MAC (démo)
    dengon_demo.{h,c}   — tâche de démo : poll + trames de test (preuve 2 cartes),
                          compilée seulement si CONFIG_DENGON_TRANSPORT_DEMO
  partitions.csv        — US-308 : nvs, phy, factory 2 Mo, littlefs 256 Ko
  dependencies.lock     — versions figées des composants gérés (littlefs)
  tools/build_core.sh   — produit libdengon_core.a (cargo +esp, xtensa)
  tools/dump_ledger.py  — capture série (`ledger`) → .bin pour dengon-verify
  components/
    dengon_core_ffi/    — US-308 : lie libdengon_core.a + header cbindgen
                          (crates/dengon-core-ffi/include/dengon_core.h)
    dengon_store/       — US-308 : le `Store` du relais
      dengon_ledger_file.c   — C PUR : relecture, réparation de fin tronquée,
                               append + fsync, anneau de 2 fichiers
      dengon_store.c         — littlefs (/lfs), secrets + curseur en NVS,
                               ancre de reprise au boot (cible matérielle)
      idf_component.yml      — joltwallet/littlefs (hors cible linux)
      test_apps/             — Unity : test_ledger_file.c (linux + esp32),
                               test_store_cible.c (esp32 seulement)
    dengon_transport_core/          — C PUR, sans NimBLE, testable sur l'hôte
      include/dengon_transport_core.h — types miroirs du contrat + API du cœur
      include/dengon_adv.h            — manufacturer data, règle anti-boucle
      dengon_transport_core.c         — table de liens, file d'événements, send
      dengon_adv.c
      test_apps/                      — projet Unity (cible linux ET esp32)
        main/banc.{h,c}               — miroir C de `BancDEssai`
        main/test_conformite.c        — les 12 cas de conformance.rs, portés 1:1
        main/test_specifique.c        — 14 cas propres au C (MTU, file, quota…)
        main/test_adv.c               — 6 cas (manufacturer data, anti-boucle)
```

`sdkconfig`, `build/`, `build-esp32/` et `managed_components/` ne sont pas
versionnés.

**Pourquoi deux moitiés.** Toute la *sémantique* du contrat (quels événements,
dans quel ordre, quelles erreurs) est dans `dengon_transport_core`, qui ne
connaît que des `conn_handle` et des codes HCI bruts : il compile pour la cible
`linux` d'ESP-IDF et passe ses tests en CI, sans carte. `transport_nimble.c` ne
fait que **traduire** les événements NimBLE en appels au cœur. Ce qui ne peut
se vérifier qu'avec deux cartes est ainsi réduit au strict minimum.

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `dengon_transport_start()` | `main/transport_nimble.c:745` | Miroir de `Transport::start`. Démarre NimBLE ; annonce et scan partent à la synchro du contrôleur. Borne `max_connections` à 3. |
| `dengon_transport_poll()` | `main/transport_nimble.c:824` | Miroir de `poll` : retire les événements, ne bloque jamais, 0 avant `start`. |
| `dengon_transport_send()` | `main/transport_nimble.c:875` | Miroir de `send` : route validée par le cœur **sous verrou**, puis émission **hors verrou** (write sans réponse ou notification selon le rôle). |
| `dengon_transport_broadcast()` | `main/transport_nimble.c:893` | Miroir de `broadcast` : au mieux, `OK` sans aucun pair. |
| `dengon_tc_t` | `components/…/include/dengon_transport_core.h` | L'état complet : démarré ?, table de liens, file FIFO de 32 événements, compteurs de pertes. Allocation statique. |
| `dengon_tc_link_open()` | `components/…/dengon_transport_core.c:146` | Ouvre un lien à la connexion GAP, **non annoncé**, `LinkId` tiré d'un compteur monotone. Refuse au-delà du quota. |
| `dengon_tc_link_announce()` | `…/dengon_transport_core.c:242` | Émet `PeerConnected` quand le lien est utilisable dans les deux sens. |
| `dengon_tc_on_rx()` | `…/dengon_transport_core.c:250` | Copie la trame, `FrameReceived` en file. Annonce paresseusement le lien si besoin. |
| `dengon_tc_link_close()` | `…/dengon_transport_core.c:293` | `PeerDisconnected` **seulement si** le lien avait été annoncé ; libère le `conn_handle`. |
| `dengon_tc_route_for_send()` | `…/dengon_transport_core.c:379` | `NOT_STARTED` / `UNKNOWN_PEER` / `FRAME_TOO_LARGE` (> MTU-3), sinon la route. |
| `q_push()` | `…/dengon_transport_core.c:32` | File unique (ordre par lien garanti) avec **réserve** pour le cycle de vie : les trames sont jetées avant un `PeerDisconnected`. |
| `dengon_tc_map_hci_reason()` | `…/dengon_transport_core.c:439` | `0x13/0x14/0x15` → Propre, `0x16` → Locale, tout le reste → Brutale. |
| `dengon_adv_parse_mfg()` | `…/dengon_adv.c:19` | Lit `peerID[0..4]` et `flags` dans le manufacturer data d'un pair, **en sautant le Company ID**. |
| `dengon_adv_should_initiate()` | `…/dengon_adv.c:40` | Règle anti-boucle : le plus petit peerID initie ; égalité → adresse BLE. |
| `on_disc()` | `main/transport_nimble.c:301` | Résultat de scan : filtre UUID + Company ID, déjà connecté ?, quota, anti-boucle, puis `ble_gap_connect`. |
| `on_connect()` / `on_disconnect()` | `main/transport_nimble.c:535` / `:599` | Ouvrent / ferment le lien dans le cœur, puis **réarment annonce et scan**. |
| `on_mtu` → `on_svc` → `on_chr` → `on_dsc` → `on_subscribed` | `main/transport_nimble.c:506` → `:389` | Chaîne de découverte côté central (une procédure ATT à la fois). |
| `dengon_relay_app_init()` | `main/dengon_relay_app.c` | US-308 : monte littlefs, relit ou génère les secrets, calcule l'ancre de reprise, crée le relais Rust, journalise `relay.boot`. **Avant** la radio. |
| `route_task()` | `main/dengon_relay_app.c` | Poll transport → `dengon_relay_link_up/_down/_on_frame`, puis `poll_routing` (relais jitterés), sous mutex ; émission hors mutex. Période 10 ms. |
| `inventory_task()` / `courier_task()` | `main/dengon_relay_app.c` | `poll_inventory` (1 s) / `poll_courier` + bilan de santé (30 s). |
| `ledger_task()` | `main/dengon_relay_app.c` | Retire les entrées produites, les ajoute à `ledger.bin` (fsync), **puis** committe le curseur NVS recalculé depuis la dernière entrée écrite. |
| `dengon_ledger_file_recover()` | `components/dengon_store/dengon_ledger_file.c` | Relit `ledger.bin` en flux, garde la dernière entrée complète, **tronque** ce qui suit (écriture coupée). |
| `dengon_store_boot_anchor()` | `components/dengon_store/dengon_store.c` | Ancre de reprise : dernière entrée du fichier, sinon curseur NVS (fichier vide après rotation), sinon genèse. |
| `dengon_store_load_secrets()` | `components/dengon_store/dengon_store.c` | Secrets X25519 + Ed25519 en NVS ; générés une fois par `esp_fill_random` sous `bootloader_random_enable()`. |
| `demo_task()` | `main/dengon_demo.c:120` | Poll, journalise, diffuse une trame de 64 o toutes les 2 s, sonde MTU-3 / MTU-2 à chaque lien, bouton BOOT = tout fermer. |

## Flux principal (exemple)

Deux cartes A (peerID `1a2b…`) et B (peerID `7f00…`) sont allumées :

1. `app_main()` initialise NVS, calcule le peerID bouchon, appelle
   `dengon_transport_start()`. Le cœur passe à « démarré » (quota borné à 3),
   NimBLE démarre ; à la synchro, `on_sync()` lance **annonce** et **scan**.
2. A voit l'annonce de B : UUID `dengon` présent, Company ID `0xFFFF`, préfixe
   `7f00…`. `1a2b… < 7f00…` : **c'est à A d'initier**. B, qui voit A, se tait.
3. A coupe son scan, `ble_gap_connect()`. À `CONNECT`, A ouvre un lien
   **central** (link#1), B un lien **périphérique** (son link#1 à lui — les
   `LinkId` sont locaux à chaque carte).
4. A enchaîne : échange MTU (517) → découverte du service → CHAR_RX et
   CHAR_TX de B → CCCD de CHAR_TX → écriture `0x0001`. B reçoit l'abonnement
   (`SUBSCRIBE`) : **PeerConnected** chez B. L'écriture confirmée :
   **PeerConnected** chez A.
5. A écrit sur le CHAR_RX de B (write sans réponse) ; B notifie sur son
   CHAR_TX. Chaque trame reçue devient un **FrameReceived** intact.
6. On débranche B : au *supervision timeout*, A reçoit `DISCONNECT` avec HCI
   `0x08` → **PeerDisconnected{Brutale}** après les trames déjà reçues. A
   **réannonce et rescanne** ; B rebranchée est retrouvée et obtient un
   `LinkId` **neuf** (link#2), même si NimBLE lui redonne le même
   `conn_handle`.

## Flux US-308 : un message traverse le relais

1. **Boot** : NVS → `dengon_relay_app_init()`. littlefs est monté ;
   `ledger.bin` est relu (une fin tronquée par une coupure est retirée) et
   l'ancre en est tirée. Les secrets sont relus de NVS, ou générés au premier
   démarrage. `dengon_relay_new(…, ancre)` crée le relais, dont le `peerID`
   est dérivé de sa clé statique. `relay.boot` est journalisé. Le transport
   démarre avec ce `peerID`, puis vient l'auto-test Noise sur l'aléa
   matériel, et enfin les 4 tâches et la console.
2. Un téléphone P1 se connecte : `route_task` voit `PeerConnected` et appelle
   `dengon_relay_link_up`. Le relais met en file son `ANNOUNCE` signé (TTL 1),
   que `route_task` émet.
3. P1 envoie son `ANNOUNCE` : le relais le vérifie, lie le lien à ce `peerID`
   (et apprend l'heure si la sienne est inconnue), journalise
   `peer.announce_seen`, puis émet `INVENTORY` et, s'il détient des
   enveloppes, `ENVELOPE_OFFER`.
4. P1 envoie un paquet `RELAY_OK` pour P2 (hors de portée) : le routeur
   l'accepte et programme un relais jitteré. 10 à 220 ms plus tard,
   `poll_routing` le rend avec TTL − 1 vers tous les autres liens.
   `pkt.relayed` est journalisé.
5. `ledger_task`, notifiée, écrit les nouvelles entrées dans `ledger.bin`
   (fsync), puis le curseur en NVS.
6. `esp_restart()` : au boot suivant, l'ancre est relue et la prochaine entrée
   porte `seq` + 1 et le `prev_hash` de la dernière. La commande `ledger`
   suivie de `dengon-verify` montre une chaîne intacte.

## Flux US-309 : un événement part au dashboard

Mise en service (une fois par relais et par base de dashboard) :

1. Récupérer la racine Caddy du VPS dans `main/certs/dashboard_root.pem`
   (fichier non versionné, `main/certs/README.md`), compiler, flasher.
2. Console : `dash id` → `node_id` (`relay-` + 3 premiers octets du
   `peerID`) et clé publique Ed25519. `tools/register_relay.py --node-id …
   --pub-sign …` poste `POST /api/nodes` et affiche `dash token <jwt>` à
   coller dans la console (jeton en NVS, espace `dengon_net`).
3. `wifi <ssid> <mdp>` (NVS aussi). Le Wi-Fi se connecte ; SNTP met l'heure.

En fonctionnement :

1. Un événement arrive au journal (`pkt.relayed`, `relay.health`…).
   `ledger_task` le retire (`dengon_relay_pop_ledger`), l'écrit dans
   littlefs **et** le copie dans le buffer ring (`dengon_ship_push`).
   Les événements produits dans un contexte à petite pile (`relay.wifi_up`
   depuis la tâche d'événements ESP-IDF) passent par une file
   (`dengon_relay_app_record`) que `ledger_task` vide : la signature du
   journal n'y tiendrait pas.
2. La tâche `dengon_ship` attend le Wi-Fi, lit au plus 16 entrées dans le
   ring (`dengon_ring_peek`, sans les retirer), et demande à dengon-core le
   corps signé (`dengon_relay_build_batch`, sous le mutex du relais : la clé
   ne sort jamais du handle).
3. `POST CONFIG_DENGON_DASH_URL/ingest/batch` en HTTPS, racine épinglée,
   `Authorization: Bearer`. Une session TLS par batch (keep-alive coupé :
   les ~40 Ko de TLS sont rendus au tas entre deux envois).
4. `dengon_ship_decide(statut)` : 2xx → `dengon_ring_commit` ; 400/413 →
   retiré quand même et compté (sinon il bloquerait la file) ; 401/403 →
   gardé, attente de 60 s ou d'un nouveau jeton ; réseau/TLS/5xx → gardé,
   backoff 1 s → 60 s avec gigue.
5. Hors ligne, le ring (12 Kio) se remplit puis écrase ses plus anciennes
   entrées, comptées (`logs_dropped` de `relay.health`). Au retour du
   réseau il se vide dans l'ordre, 1 batch/s. Un `commit` après écrasement
   ne retire que les entrées encore présentes (identifiants croissants).

## Dépendances

- **Internes :** `dengon_transport_core`, `dengon_core_ffi`
  (`libdengon_core.a` + header), `dengon_store`, `dengon_ship_core` (US-309 :
  ring + politique, C pur).
- **Externes ajoutées par US-309 :** `esp_wifi`, `esp_netif` (+ SNTP),
  `esp_event`, `esp_http_client` (mbedTLS/esp-tls).
- **Gérées (registre ESP-IDF) :** `joltwallet/littlefs` 1.22.3 (figée dans
  `dependencies.lock`).
- **Externes (composants ESP-IDF) :** `bt` (NimBLE), `nvs_flash` (calibration
  radio), `esp_hw_support` (MAC eFuse), `mbedtls` (SHA-256 du bouchon),
  `esp_driver_gpio` (bouton BOOT de la démo), `esp_rom` (CRC32 de la démo),
  `console`, `esp_timer`, `esp_app_format`, `bootloader_support`
  (`bootloader_random_enable`), `spi_flash` (`relay.boot`), `unity` (tests).

## Décisions d'implémentation

- **1 trame = 1 PDU ATT, pas de fragmentation BLE** : `send` au-delà de
  MTU-3 rend `FRAME_TOO_LARGE`. La fragmentation protocole (US-202) découpe déjà
  à cette taille ; la règle n°3 du contrat (trames partielles) est donc tenue
  par construction. Écart consigné.
- **`PeerConnected` = lien utilisable dans les deux sens** (après abonnement),
  annoncé paresseusement à la première trame si le pair écrit sans s'abonner.
  Écart consigné.
- **`max_connections` borné à 3** (contrôleur) au lieu d'échouer. Écart
  consigné.
- **Anti-boucle sur 4 octets de peerID**, départage à l'adresse BLE. Écart
  consigné.
- **Codes HCI → motif** : seul ce qui est *annoncé* par le pair est « Propre ».
- **Aucun appel NimBLE sous le verrou du cœur** : évite tout interblocage avec
  le verrou interne du host.
- **Le MTU d'un lien n'est enregistré qu'à `BLE_GAP_EVENT_MTU`**, dans les
  deux rôles. `on_mtu` (central) ne fait qu'enchaîner la découverte : NimBLE
  émet l'événement GAP avant d'appeler ce callback (retour de revue PR #101).
- **Contre-pression** : la file jette des trames entrantes, jamais un
  événement de cycle de vie (docs/synthese/08 §4).
- **Démo désactivable** par Kconfig : `CONFIG_DENGON_TRANSPORT_DEMO`.
- Hérités d'US-114 : `-Werror` sur nos seuls composants, nom d'annonce en
  réponse de scan (paquet principal à 30/31 octets), MTU fixé deux fois.
- **US-308 — la logique reste en Rust.** Le firmware ne fait que brancher
  `dengon_core::relay`, testé sous `cargo test`. Aucune règle de routage n'est
  réécrite en C.
- **US-308 — un seul handle, un mutex.** Les 4 tâches ont des cadences
  différentes mais partagent l'état du relais. Les envois passent **hors
  mutex** (`dengon_transport_send` est thread-safe).
- **US-308 — le fichier fait foi, le curseur suit.** Entrées fsync-ées
  d'abord, curseur NVS ensuite, recalculé depuis la dernière entrée *écrite*
  (pas depuis le relais, qui a pu avancer entre-temps). Une coupure entre les
  deux est rattrapée au boot.
- **US-308 — anneau de 2 × 96 Ko.** Au-delà, `ledger.bin` devient
  `ledger.old`. Juste après une rotation, c'est le curseur NVS qui porte
  l'ancre.
- **US-308 — aléa.** Secrets tirés sous `bootloader_random_enable()`, radio
  éteinte (ESP-IDF interdit de le laisser actif ensuite). Noise est vérifié au
  boot, radio allumée, par `dengon_noise_selftest(esp_fill_random)`.
- **US-308 — heure.** `gettimeofday()` part de 1970 sans SNTP : le relais
  Rust apprend l'heure du premier `ANNOUNCE` authentique. Avant cela, il
  ignore les paquets (`sans_heure` dans les compteurs).
- **US-308 — démo US-220 à `n` par défaut.** Activée, elle **remplace** le
  relais (peerID bouchon, pas de dengon-core).

## Décisions d'implémentation — US-309

- **Source = le journal chaîné.** Le `seq` de l'événement envoyé est celui de
  l'entrée du journal : monotone, repris après `esp_restart()`, et la dédup
  du serveur par `event_id = SHA-256(node_id ‖ seq)` rend tout rejeu inoffensif
  (vérifié : un batch reposté rend 202 avec `new_event_count: 0`).
- **Ring en RAM, pas en littlefs.** Le critère de l'US est « sans dépasser la
  mémoire » : tampon statique fixe, jamais d'allocation. Le journal complet
  reste de toute façon sur littlefs. Conséquence : ce qui est dans le ring au
  moment d'un redémarrage n'est pas renvoyé. Écart consigné.
- **12 Kio par défaut** (`CONFIG_DENGON_SHIP_RING_BYTES`) : ~50 entrées. Le tas
  libre mesuré à l'US-308 (128 Ko, avant Wi-Fi) doit encore porter Wi-Fi
  (~50 Ko) et une session TLS (~35-40 Ko).
- **Signature en Rust, pas en C.** Le JSON canonique et Ed25519 existent déjà
  dans dengon-core ; les réécrire en C ferait deux implémentations à garder
  identiques octet pour octet.
- **Racine épinglée, pas le certificat feuille.** Caddy `tls internal`
  renouvelle la feuille toutes les 12 h et l'intermédiaire tous les 7 jours ;
  seule la racine est stable. Le serveur ne l'envoie pas : il faut la copier
  depuis le VPS. Le fichier est facultatif au build (sinon la CI casserait) :
  absent, l'export est coupé et `dash status` le dit.
- **IRAM** : Wi-Fi + BLE dépassaient l'IRAM de 1,6 Ko. Optimisations IRAM du
  Wi-Fi et de lwIP coupées (`sdkconfig.defaults` §8) : IRAM à 82 %.

## Tests

**US-309 — 10 tests Unity** dans `components/dengon_ship_core/test_apps/`
(cible `linux`, workflow `firmware`), exécutés le 2026-09-29 : **10 Tests 0
Failures**. Ring : FIFO, `peek` borné en nombre et en place, écrasement des
plus anciens compté, refus de l'entrée trop grosse, `commit` après
écrasement, et le scénario de l'US (1000 entrées hors ligne, la mémoire ne
dépasse jamais le tampon — sentinelles autour —, puis vidage dans l'ordre par
lots de 3). Politique : codes réels du dashboard, passagers, backoff, gigue.
Test app aussi compilée pour `esp32`. Firmware complet :
`idf.py build` propre (binaire 1,58 Mo, 21 % libre, IRAM 82 %, DRAM statique
66 %). Côté Rust (dengon-core / dengon-core-ffi), voir leurs fiches.
**Non exécuté** : rien de l'US-309 n'a tourné sur carte.

**Automatisés — 32 tests Unity**, dans `components/dengon_transport_core/test_apps/`,
exécutés sur l'hôte (cible `linux`) par le workflow `firmware` :

- **12 cas de conformité** portés 1:1 depuis `crates/dengon-ble/src/conformance.rs`
  (mêmes noms, mêmes messages). Le banc (`banc.c`) pilote le cœur exactement
  comme la glue NimBLE. `cas_link_id_jamais_reutilise` est **renforcé** : le
  banc redonne le même `conn_handle`, comme NimBLE.
- **14 cas propres au C** : limite MTU-3 exacte, plancher 23 → 20 octets, lien
  non annoncé invisible, `PeerConnected` paresseux, fermeture silencieuse d'un
  lien jamais annoncé, quota borné, file saturée qui garde la fermeture, poll
  par petits lots, trame vide, broadcast au mieux, route, mapping HCI.
- **6 cas d'annonce** : aller-retour du manufacturer data (octets identiques au
  format US-114), rejet d'un champ trop court ou d'un autre fabricant,
  anti-boucle (inférieur, égalité, soi-même).

Vérifications réellement exécutées le 2026-09-28 (image `espressif/idf:v5.5.5`
épinglée) :

- Tests hôte : **32 Tests 0 Failures**, code de sortie 0.
- **Test de mutation** : une purge de la file à la fermeture (le bogue « purge
  avant vidage » que redoute la rustdoc) fait échouer 2 cas
  (`cas_trame_recue_avant_coupure_est_livree`,
  `test_file_saturee_garde_la_fermeture`), code de sortie 1. Code restauré.
- Test app compilée aussi pour `esp32` (`build-esp32/`) : OK.
- `idf.py fullclean build` du firmware : `Project build complete`, **0 warning**
  dans `main` et `dengon_transport_core` (`-Werror`). Binaire 507 984 o
  (463,5 Ko en US-114), 68 % de la partition applicative libre.
- `idf.py size` : DRAM 22,6 % (96,5 Ko restants ; 20,7 % en US-114), IRAM 75,1 %.
- `sdkconfig` généré relu : `ROLE_CENTRAL`, `ROLE_OBSERVER`, `GATT_CLIENT` à
  `y`, `MAX_CONNECTIONS=3`.

**Sur carte réelle — 2026-09-28**, une seule ESP32 disponible
(ESP32-D0WD-V3, MAC `48:9d:31:00:83:dc`, puce USB CH340, rattachée à WSL par
`usbipd`) :

- **Tests Unity sur cible : `32 Tests 0 Failures`** — la même application que
  sur l'hôte, flashée (`build-esp32/`), lue sur le port série.
- **Firmware relais** : table GATT enregistrée, `max_connections=8 demandé,
  borné à 3`, annonce `dengon-relay-39e1`, scan relancé toutes les 10 s. Deux
  captures de 4 et 10 min : aucun reset, aucun `Guru Meditation`.
- **Rôle périphérique contre un vrai pair** — Pixel 8 Pro (Android 17) +
  nRF Connect 4.29.1, piloté par `adb` (`uiautomator` + `input tap`) pendant la
  lecture du port série :

| Étape côté téléphone | Observé sur la carte | Verdict |
|---|---|---|
| Table GATT lue | RX `WRITE NO RESPONSE`, TX `NOTIFY`, **un seul** CCCD (auto) | ✅ |
| CONNECT | `connexion établie conn=0, rôle périphérique` + **réannonce** (1/3 liens) | ✅ |
| Abonnement à `…0002` | `lien prêt (pair abonné) -> PeerConnected link#1` | ✅ |
| (MTU 23, non négocié) | sonde `20 o (MTU-3) -> ok`, `21 o (MTU-2) -> trame trop grande` ; le téléphone reçoit `44-47-4E-30…` = « DGN0 », motif intact | ✅ |
| Écrire `DEADBEEF` sur `…0001` | `FrameReceived link#1 : 4 o, crc32=7c9ca35a` — CRC recalculé sur PC : `0x7c9ca35a` | ✅ |
| Request MTU 517 | `ATT MTU négocié = 517 (trame max 514 o)` ; les diffusions de 64 o arrivent alors toutes les 2 s (sautées à MTU 23, « au mieux ») | ✅ |
| DISCONNECT | `HCI 0x13 -> Propre -> PeerDisconnected`, puis `send sur link#1 fermé -> pair inconnu` | ✅ |
| Reconnexion | NimBLE redonne **`conn=0`**, le lien reçoit **`link#2`** | ✅ |
| Bluetooth du téléphone coupé (`bluetooth_manager disable`) | `HCI 0x15 -> Propre` — Android **prévient** avant d'éteindre la radio : ce n'est pas une coupure brutale | ✅ (mapping) |
| **Pile Bluetooth tuée** (`am force-stop com.google.android.bluetooth`) | ~5 s plus tard (supervision timeout) : `HCI 0x08 -> Brutale -> PeerDisconnected link#1, motif Brutale`, un seul événement, puis `send … -> pair inconnu` | ✅ |
| Après la coupure brutale : nouveau scan + CONNECT | carte toujours annoncée, `PeerConnected link#2` | ✅ |

### US-308 (2026-09-29)

**Automatisés, sur l'hôte :**
- `cargo test -p dengon-core --lib relay::`, 12 tests : deux relais se lient
  par `ANNOUNCE` ; un message traverse deux relais (P1 → A → B → P2, TTL
  décrémenté, pas de second relais du doublon) ; heure apprise ; enveloppe
  déposée, offerte puis remise sur requête ; offre aux voisins déjà liés ;
  `ANNOUNCE` usurpé et requête mal signée refusés ; trames hostiles ; journal
  repris après redémarrage (chaîne `Ok`, signatures vérifiées) ; paquets
  ≤ 514 o.
- `cargo test -p dengon-core-ffi`, 5 tests d'API C : cycle de vie,
  `BUFFER_TOO_SMALL` sans perte, journal persisté puis repris, pointeurs nuls,
  auto-test Noise (aléa correct → OK, aléa constant → refus).
- Tests Unity du Store, cible `linux` : **5 Tests 0 Failures** (longueur
  d'entrée, fichier absent, ancre, fin tronquée réparée et idempotente,
  rotation).
- `idf.py build` du firmware complet : `Project build complete`. Binaire
  0xf27b0 o (53 % libre sur 2 Mo). `idf.py size` : IRAM 78,2 %, DRAM 33,7 %
  (22,6 % avant).
- Test app du Store compilée pour `esp32` : OK.
- Essai hôte jetable : journal produit via l'API C, redémarrage par l'ancre,
  export au format de la console, `tools/dump_ledger.py`, puis
  `dengon-verify --pubkey` → `{"verdict":"ok","entries":5,…,"signatures":"verified"}`.

**Sur carte réelle, 2026-09-29** (ESP32-D0WD-V3 et Pixel 8 Pro avec nRF
Connect, piloté par `adb.exe`) :
- Tests Unity du Store sur cible : **9 Tests 0 Failures**.
- Relais : auto-test Noise sur `esp_fill_random` OK à chaque boot ; 3
  `restart` → reprise `seq=1,2,3` ; `dengon-verify` → `ok`, 4 entrées,
  signatures vérifiées.
- **Débordement de pile** de `dengon_route` (6 Ko) au premier `link_up`,
  corrigé (16 Ko ; marge mesurée ensuite : 8,6 Ko libres).
- Après correctif : abonnement du téléphone → lien ouvert sans crash ; trame
  `DEADBEEF` → `pkt.rejected malformed` ; déconnexion propre. Journal de
  10 entrées **à travers un panic et un reflash** → `ok`, signatures
  vérifiées.
- nRF Connect reste en MTU 23 : l'`ANNOUNCE` (174 o) n'est pas émis vers lui.

**Reste non vérifié** : essai 2 cartes (une seule disponible), coupure
d'alimentation pendant une écriture, message qui traverse le relais depuis un
téléphone. Procédure :

| # | Action | Attendu |
|---|---|---|
| 1 | `tools/build_core.sh`, puis `idf.py erase-flash flash monitor` sur A | `relais-xxxx peerID=… (nouvelle identité), journal reprend à seq=0`, `clé de journal (dengon-verify --pubkey) = …`, `auto-test Noise XX sur esp_fill_random : OK` |
| 2 | `restart` dans la console de A, 3 fois | `identité relue`, `journal reprend à seq=N` croissant |
| 3 | `ledger` dans la console, capture (`idf.py monitor \| tee capture.log`) | bloc `DENGON-LEDGER-BEGIN…END` |
| 4 | `tools/dump_ledger.py capture.log -o l.bin` puis `cargo run -p dengon-verify -- --pubkey <clé> l.bin` | `{"verdict":"ok", …, "signatures":"verified"}` |
| 5 | Couper l'alimentation de A pendant du trafic, rallumer, refaire 3-4 | éventuel `journal : N o d'écriture interrompue retirés`, verdict `ok` |
| 6 | A et B flashées, allumées côte à côte | chez chacune `lien 1 ouvert`, puis `relay` → compteurs ; journal : `peer.announce_seen` de l'autre |
| 7 | Un message traverse le relais | **Bloqué côté client** : `api.rs` ne pose pas `RELAY_OK` et n'émet pas `ANNOUNCE` (écart US-308). Faisable en injectant des paquets forgés (outil à écrire) ou après l'US client correspondante. |

**Non vérifié — essai sur 2 cartes.** Une seule carte était disponible. Le
rôle **central** (scan qui trouve un pair dengon, règle anti-boucle, chaîne
MTU → service → caractéristiques → CCCD → abonnement, `NOTIFY_RX`) n'a
**jamais** tourné contre un vrai pair : un téléphone n'annonce pas le service
`dengon`. Le relais d'octets **entre deux cartes** et la coupure d'une carte
par l'autre restent à démontrer avec la procédure ci-dessous.

### Procédure d'essai sur 2 cartes (critères n°1, n°3, n°4)

Flasher la même image sur A et B (`idf.py flash monitor`, un terminal par
carte). Noter le peerID de chacune (`dengon-peer` au boot).

| # | Action | Attendu dans les moniteurs |
|---|---|---|
| a | Allumer A et B | Une seule carte (plus petit peerID) : `pair … vu … : connexion`. Les deux : `ATT MTU négocié = 517`, puis `PeerConnected link#1`. |
| b | Attendre 10 s | Sondes : `trame de 514 o (MTU-3) -> ok`, `trame de 515 o (MTU-2) -> trame trop grande`. Toutes les 2 s, `FrameReceived … 64 o de <peerID d'en face> … motif intact` des deux côtés. `FrameReceived … 514 o … motif intact`. |
| c | **Débrancher B** (alimentation, pas `reset`) | Sur A, en quelques secondes : `déconnexion (HCI 0x08 -> Brutale)`, `PeerDisconnected link#1, motif Brutale`, `send sur link#1 fermé -> pair inconnu`, `annonce en cours`. Mesurer le délai. nRF Connect voit A de nouveau. |
| d | Rebrancher B | Reconnexion ; sur A, `PeerConnected link#2` (**LinkId neuf**). |
| e | Appuyer sur BOOT de A | A : `motif Locale` ; B : `motif Propre`. Puis reconnexion automatique. |

## Limites connues / TODO

- **US-309 pas exécuté sur carte.** Restent à vérifier : connexion Wi-Fi avec
  BLE actif (coexistence), tas libre avec une session TLS ouverte, que
  mbedTLS accepte le certificat au nom d'une IP (sinon
  `CONFIG_DENGON_DASH_SKIP_CN_CHECK`), 202 contre le VPS, coupure réseau.
- **`rssi_avg` de `relay.health` = RSSI Wi-Fi** du point d'accès (-120 hors
  connexion) : le transport ne remonte pas le RSSI des voisins BLE.
- **Jeton de 24 h sans renouvellement** (limite du dashboard US-216) : il
  faut réenregistrer le relais après chaque purge de la base de démo.

- **US-308 exécutée sur une seule carte** : l'essai à 2 cartes reste à faire.
  La branche dépend de la PR #108 (US-307), non mergée, dont la CI n'a jamais
  tourné.
- **Un téléphone ne peut pas encore faire relayer un message** : le client
  (`api.rs`) ne pose jamais `RELAY_OK` et n'émet ni `ANNOUNCE`, ni
  `INVENTORY`, ni `ENVELOPE_REQUEST`. Écart consigné, hors périmètre US-308.
- **Pas de fragmentation ni de réassemblage côté relais** : les fragments sont
  relayés comme des paquets ordinaires. Une enveloppe fragmentée n'entre pas
  au courrier.
- **NVS non chiffré** (la conception dit « chiffré par eFuse ») : les secrets
  du relais sont lisibles par qui dumpe la flash.
- **Pas de SNTP** : heure apprise des `ANNOUNCE` (US-309 pour le Wi-Fi).
- `peer.connected` / `peer.disconnected` ne sont pas journalisés (le `peerID`
  n'est connu qu'à l'`ANNOUNCE`) ; `peer.announce_seen` en tient lieu.

- **Essai 2 cartes à faire** (voir ci-dessus) — condition de clôture de l'US.
  Le rôle central n'a jamais tourné contre un vrai pair.
- **nRF Connect ne négocie pas le MTU** tant qu'on ne le demande pas : un
  téléphone reste à 23 (20 o utiles), et les diffusions de 64 o de la démo
  lui sont sautées. L'app Android (US-213) devra demander 517 elle-même.
- **La suite Rust elle-même** n'a pas tourné contre ce code : c'est son
  portage C qui a tourné. L'adaptateur Rust au-dessus de `dengon_transport.h`
  est l'US-307.
- **3 liens au plus** (contrôleur) ; au-delà, annonce et scan suspendus.
- **Pas de fragmentation BLE** : une trame > MTU-3 est refusée — suppose que
  l'appelant fragmente (c'est le cas de `dengon-core`).
- **Anti-boucle unilatérale** : si le nœud au plus petit peerID a son quota
  plein ou son scan coupé, les deux ne se connectent jamais.
- **RSSI absent côté périphérique** (`PeerConnected` sans RSSI).
- Pas de Wi-Fi, pas de HTTPS (US-309).
- `CONFIG_BT_NIMBLE_SECURITY_ENABLE` toujours à `y` (voir US-114).
- Le workflow `firmware` n'est **pas** dans les checks requis de `main`.
- US-114 : critères n°2 et n°3 (capture nRF Connect) toujours non démontrés.

## Pour l'oral

Deux cartes qui se parlent en Bluetooth Low Energy, ce n'est pas symétrique :
pour chaque liaison, l'une est « centrale » (elle se connecte) et l'autre
« périphérique » (elle s'annonce). Or dengon veut que **chaque** nœud soit les
deux. Si deux cartes s'aperçoivent en même temps, elles se connecteraient
chacune à l'autre — deux liaisons pour rien, sur trois possibles. La règle :
**celle dont l'identifiant est le plus petit se connecte, l'autre attend**.

Le point le plus intéressant à raconter est la **coupure brutale** : quand on
arrache l'alimentation d'une carte, l'autre ne reçoit aucun « au revoir ». Elle
s'en aperçoit seule, au bout de quelques secondes de silence radio (le
*supervision timeout*). À ce moment, elle doit : prévenir le cœur une seule
fois, **sans perdre les messages déjà reçus**, refuser proprement d'écrire vers
la carte morte, et surtout **recommencer à s'annoncer** — sinon elle devient
invisible pour toujours. Toute cette logique est isolée dans un morceau de C
pur, testé automatiquement sur un PC, avec exactement les mêmes 12 scénarios
que ceux écrits en Rust pour le contrat.

**US-308, en une image.** La carte est un facteur qui ne sait pas lire. Elle
reçoit des paquets fermés et regarde seulement l'enveloppe extérieure : l'a-t-il
déjà vu ? Combien de sauts lui reste-t-il ? Est-il destiné à quelqu'un
d'autre ? Si oui, elle le fait suivre. Elle garde les lettres scellées pour les
absents et ne les rend qu'à celui qui prouve, par une étiquette du jour, qu'elles
sont pour lui. Et elle tient un **registre signé** où chaque ligne contient
l'empreinte de la précédente. Le point démontrable : on débranche la carte en
plein travail, on la rallume, et l'outil `dengon-verify` confirme qu'aucune
ligne n'a été perdue ni réécrite. C'est possible parce que le registre est
écrit sur la flash *avant* le marque-page, et qu'au redémarrage la carte
reprend à la dernière ligne complète.
