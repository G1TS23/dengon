# Module : `firmware-relay` (`firmware/dengon-relay/`)

**Rôle en une phrase :** le firmware du relais ESP32 — un transport BLE NimBLE
à rôle double (annonce + scan, serveur + client GATT) qui échange des **octets
opaques** avec les autres nœuds, conforme au contrat `Transport` d'US-105
(US-220), sur le squelette d'US-114.
**Correspond à la conception :** [`docs/synthese/08-relais-esp32.md`](../../synthese/08-relais-esp32.md)
§2-4, [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md)
§3 et §5, [`docs/powl/03-network-protocol.md`](../../powl/03-network-protocol.md)
§2 et §6 (UUID, rôle double, anti-boucle, MTU) — décision C-2 ; contrat
[`crates/dengon-ble/src/transport.rs`](../../../crates/dengon-ble/src/transport.rs).
**Dernière mise à jour :** 2026-09-28
**État :** partiel — transport BLE complet ; testé sur l'hôte, sur carte
(Unity) et en rôle périphérique contre un téléphone ; **essai sur 2 cartes
(rôle central) pas encore fait** ; aucune logique dengon (US-307/308).

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

# Explorer la configuration (écrit dans sdkconfig, PAS dans sdkconfig.defaults)
docker run --rm -it -u "$(id -u):$(id -g)" -e HOME=/tmp \
  -v "$PWD:/repo" -w /repo/firmware/dengon-relay "$IDF" idf.py menuconfig
```

`-u "$(id -u):$(id -g)"` et `-e HOME=/tmp` ne sont pas décoratifs : voir le
piège n°2 plus bas.

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
Elle **ne comprend pas** ce qu'elle transporte — c'est voulu : le décodage, la
déduplication et le routage viendront de `dengon-core` par FFI (US-307), branché
sur ce transport en US-308. Cette US dérisque la radio embarquée avant d'y
mettre de la logique.

## Structure

```
firmware/dengon-relay/
  CMakeLists.txt        — projet ESP-IDF ; son `project()` nomme les artefacts
  sdkconfig.defaults    — LA configuration versionnée (host, 4 rôles, MTU, partitions)
  README.md             — pointeur vers cette fiche
  main/                 — l'application : tout ce qui touche NimBLE
    CMakeLists.txt      — composant `main` ; -Wall -Wextra -Werror sur NOTRE code
    Kconfig.projbuild   — menu « dengon — relais » : démo on/off, période
    main.c              — app_main : NVS, peerID, dengon_transport_start(), démo
    dengon_transport.h  — API C publique = miroir du trait Rust `Transport`
    transport_nimble.c  — glue NimBLE : annonce, scan, connexion, découverte
                          GATT, abonnement, émission/réception, événements GAP
    dengon_gatt.{h,c}   — les 3 UUID et la table GATT ; les écritures sur
                          CHAR_RX partent au transport
    dengon_peer_id.{h,c}— bouchon d'identité (SHA-256 de la MAC eFuse)
    dengon_demo.{h,c}   — tâche de démo : poll + trames de test (preuve 2 cartes)
  components/
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

## Dépendances

- **Internes :** `dengon_transport_core` (composant du projet). L'US-307
  ajoutera `components/dengon_core_ffi/libdengon_core.a`.
- **Externes (composants ESP-IDF) :** `bt` (NimBLE), `nvs_flash` (calibration
  radio), `esp_hw_support` (MAC eFuse), `mbedtls` (SHA-256 du bouchon),
  `esp_driver_gpio` (bouton BOOT de la démo), `esp_rom` (CRC32 de la démo),
  `unity` (tests).

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
  réponse de scan (paquet principal à 30/31 octets), partitionnement
  `SINGLE_APP_LARGE`, MTU fixé deux fois.

## Tests

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
- Le `peerID` reste un **bouchon** sur la MAC eFuse (US-307).
- Pas de Wi-Fi, pas de HTTPS, pas de journal chaîné, pas de littlefs.
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
