# Module : `firmware-relay` (`firmware/dengon-relay/`)

**Rôle en une phrase :** le firmware du relais ESP32 — pour l'instant, un
squelette ESP-IDF + NimBLE qui annonce le service BLE `dengon` et déclare sa
table GATT, sans encore relayer le moindre message (US-114).
**Correspond à la conception :** [`docs/synthese/08-relais-esp32.md`](../../synthese/08-relais-esp32.md)
§2-3 (matériel et pile), [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md)
§3 et §5, [`docs/powl/03-network-protocol.md`](../../powl/03-network-protocol.md)
§2 et §6 (UUID, rôle BLE, MTU) — décision C-2.
**Dernière mise à jour :** 2026-09-16
**État :** esquisse (annonce et table GATT ; aucune logique de relais)

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
- **`captures/` est dans le `.gitignore`** du dépôt : une capture d'écran rangée
  là disparaîtrait en silence. Les captures vont dans `docs/suivi/modules/img/`.

---

## À quoi ça sert

Le relais ESP32 est la partie « infrastructure » de dengon : une carte sur
secteur, posée en hauteur, toujours allumée, qui relaie les messages des
téléphones alentour, garde les enveloppes destinées aux absents et remonte ses
journaux au dashboard. Rien de tout cela n'est encore codé.

Cette US livre uniquement la **chaîne de build embarquée** et la preuve qu'elle
fonctionne : un firmware qui compile, flashe, démarre et se fait voir des autres
appareils BLE sous la bonne identité. C'est une US de dérisquage — elle existe
pour que l'US-220, qui écrira le vrai transport, ne découvre pas en même temps
les problèmes de toolchain et les problèmes de protocole.

## Structure

```
firmware/dengon-relay/
  CMakeLists.txt        — projet ESP-IDF ; son `project()` nomme les artefacts
  sdkconfig.defaults    — LA configuration versionnée (host, rôles, MTU, partitions)
  README.md             — pointeur vers cette fiche
  main/
    CMakeLists.txt      — composant `main` ; pose -Wall -Wextra -Werror sur NOTRE code
    main.c              — app_main, séquence d'init, annonce BLE, événements GAP
    dengon_gatt.{h,c}   — les 3 UUID du service `dengon` et la table GATT
    dengon_peer_id.{h,c}— bouchon d'identité (SHA-256 de la MAC eFuse)
```

`sdkconfig` (généré), `build/` et `managed_components/` ne sont pas versionnés.

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `dengon_svc_uuid` | `main/dengon_gatt.c:35` | UUID du service `dengon`, `6d656e67-…-0000`. Exporté car l'annonce doit le publier. |
| `dengon_chr_rx_uuid` | `main/dengon_gatt.c:40` | `…0001` — le pair y écrit ses paquets, sans réponse ATT. |
| `dengon_chr_tx_uuid` | `main/dengon_gatt.c:45` | `…0002` — le nœud y pousse ses paquets par notification. |
| `dengon_gatt_svcs[]` | `main/dengon_gatt.c:63` | La table GATT : un service primaire, deux caractéristiques, pas de CCCD déclaré. |
| `dengon_chr_access(...)` | `main/dengon_gatt.c:98` | Callback d'accès. Journalise les écritures reçues **et les jette** — le routage est l'US-220. |
| `dengon_gatt_register_cb(...)` | `main/dengon_gatt.c:122` | Imprime au démarrage les UUID tels que NimBLE les a enregistrés. C'est le garde-fou contre l'endianness. |
| `dengon_gatt_init()` | `main/dengon_gatt.c:153` | `count_cfg` puis `add_svcs` — l'ordre est imposé. |
| `dengon_peer_id_init()` | `main/dengon_peer_id.c:17` | **Bouchon** : `SHA-256(MAC eFuse)[0..8]`. Pas une identité cryptographique. |
| `dengon_build_adv_payloads()` | `main/main.c:75` | Fabrique les 7 octets de manufacturer data et le nom `dengon-relay-XXXX`. |
| `dengon_gap_event(...)` | `main/main.c:96` | Événements GAP. Réarme l'annonce à la déconnexion, journalise le MTU négocié. |
| `dengon_advertise()` | `main/main.c:145` | Remplit le paquet d'annonce (30 o sur 31) et la réponse de scan, puis démarre. |
| `dengon_on_sync()` | `main/main.c:216` | Appelé quand le contrôleur est prêt. **Seul endroit** d'où l'on peut annoncer. |
| `app_main()` | `main/main.c:250` | La séquence d'initialisation, dont l'ordre est imposé par ESP-IDF. |

Constantes notables : `DENGON_ATT_PREFERRED_MTU` (`main/main.c:45`),
`DENGON_ADV_COMPANY_ID` (`:53`), les quatre `DENGON_ADV_F_*` (`:58`).

## Flux principal (exemple)

Ce que fait la carte entre la mise sous tension et l'affichage dans nRF Connect :

1. `app_main()` initialise **NVS** — le contrôleur Bluetooth y range sa
   calibration radio, rien ne marche sans.
2. `dengon_peer_id_init()` lit la MAC d'usine en eFuse et la hache : on obtient
   8 octets stables qui tiennent lieu d'identité provisoire.
3. `dengon_build_adv_payloads()` en tire les 7 octets de manufacturer data
   (`0xFFFF` ‖ 4 octets de peerID ‖ `0x05`) et le nom `dengon-relay-XXXX`.
4. `nimble_port_init()` allume le contrôleur et le host — mais le host **ne
   tourne pas encore**.
5. Les callbacks sont posés, la table GATT enregistrée, le MTU préféré fixé à
   517. `nimble_port_freertos_init()` lance enfin la tâche host.
6. Quand le contrôleur est synchronisé, `dengon_on_sync()` s'assure qu'une
   adresse existe, puis appelle `dengon_advertise()`.
7. La carte émet une annonce toutes les ~100 ms : Flags, UUID du service,
   manufacturer data. Un scanner qui la voit demande la réponse de scan et
   obtient le nom.
8. À la connexion, le pair lit la table GATT et voit les deux caractéristiques.
   S'il écrit dans `CHAR_RX`, `dengon_chr_access()` journalise et jette. À la
   déconnexion, l'annonce est **réarmée** — sinon la carte disparaîtrait.

## Dépendances

- **Internes :** aucune pour l'instant. L'US-307 ajoutera
  `components/dengon_core_ffi/libdengon_core.a`, le cœur Rust cross-compilé.
- **Externes (composants ESP-IDF) :** `bt` (NimBLE, host et contrôleur),
  `nvs_flash` (calibration radio, et plus tard la clé du relais),
  `esp_hw_support` (lecture de la MAC eFuse), `mbedtls` (SHA-256 du bouchon).

## Décisions d'implémentation

- **Rôles central et observateur désactivés** dans `sdkconfig.defaults`. Le
  périmètre de l'US est ainsi vérifiable par la machine : ce firmware est
  techniquement incapable de scanner, il ne peut pas déborder sur l'US-220.
- **`-Werror` sur le seul composant `main`**, jamais globalement : les
  composants d'ESP-IDF portent leurs propres avertissements assumés, et un
  `-Werror` global casserait leur compilation.
- **Nom d'annonce en réponse de scan.** Le paquet principal est saturé à
  30 octets sur 31 par ce que la conception impose. Voir
  [`03-ecarts-conception.md`](../03-ecarts-conception.md).
- **Partitionnement `SINGLE_APP_LARGE`** posé d'emblée. Le schéma par défaut
  n'accorde que 1 Mo à l'application ; rebasculer plus tard obligerait à
  réécrire toute la flash de chaque carte déjà déployée.
- **Le MTU est fixé deux fois**, en Kconfig et par `ble_att_set_preferred_mtu()`.
  Volontaire : la valeur reste visible dans le code même si `sdkconfig` est
  régénéré.
- Cinq écarts vs conception sont consignés dans
  [`03-ecarts-conception.md`](../03-ecarts-conception.md) (arborescence, nom
  d'annonce, Company ID, bitfield `flags`, bouchon `peerID`).

## Tests

**Aucun test automatisé.** Le seul niveau atteignable sans matériel en CI est la
compilation ; c'est ce que fait le workflow `firmware`. Les tests Unity sur cible
et les vecteurs de conformité relèvent de l'US-308 / US-312.

Vérifications réellement exécutées le 2026-09-16 :

- `idf.py build` → `Project build complete`, **zéro warning** avec
  `-Wall -Wextra -Werror` sur `main`. Binaire : 463,5 Ko, soit 69 % de la
  partition applicative encore libre.
- `idf.py size` → IRAM 75,1 %, DRAM 20,7 % (98,7 Ko restants), image totale
  474 461 octets. Point de départ du budget mémoire de
  [`08-relais-esp32.md`](../../synthese/08-relais-esp32.md) §5.
- Cohérence des UUID vérifiée par calcul : les trois tableaux little-endian,
  relus à l'envers, redonnent exactement les valeurs de `docs/powl/03` §2.
- Configuration effective relue dans le `sdkconfig` généré : `BT_NIMBLE_ENABLED=y`,
  aucun `BLUEDROID_ENABLED=y`, `ROLE_CENTRAL` et `ROLE_OBSERVER` absents (donc
  `n`), `ATT_PREFERRED_MTU=517`.

**Non vérifié, faute d'avoir pu flasher :** rien n'a tourné sur une vraie carte.
L'annonce BLE, le nom, le manufacturer data, le MTU négocié et l'affichage dans
nRF Connect **ne sont pas prouvés**. La cause est connue et documentée plus
haut : sous WSL2, aucun port série n'est visible tant que `usbipd-win` n'a pas
rattaché le périphérique, et cet outil n'est pas installé sur la machine
Windows. Les critères d'acceptation n°2 et n°3 de l'US-114 restent donc ouverts.

## Limites connues / TODO

- **Critères n°2 et n°3 de l'US non démontrés** : flash et capture nRF Connect à
  faire dès que le port série est rattaché.
- Le `peerID` est un **bouchon** sur la MAC eFuse. Ce n'est pas une identité
  cryptographique — levée en US-307.
- Aucune logique de relais : les paquets reçus sur `CHAR_RX` sont journalisés
  puis jetés (US-220).
- Pas de rôle central, donc pas de scan (US-220).
- Pas de `dengon_core_ffi` : le link de `libdengon_core.a` dans un projet
  ESP-IDF n'est **toujours pas prouvé** — le Spike A a compilé pour
  `xtensa-esp32-none-elf` alors qu'ESP-IDF vise `xtensa-esp32-espidf` (US-307).
- Pas de Wi-Fi, pas de HTTPS, pas de journal chaîné, pas de littlefs.
- `CONFIG_BT_NIMBLE_SECURITY_ENABLE` est laissé à `y` (valeur par défaut) : le
  couper ferait disparaître `ble_store_config_init()` du link. Optimisation de
  flash à traiter en US-308, avec la suppression de l'appel côté `main.c`.
- Le workflow `firmware` n'est **pas** dans les checks requis de `main`. À
  ajouter après le merge, en répétant la liste complète des contextes.

## Pour l'oral

Un ESP32 qui « se fait voir » en Bluetooth, ce n'est pas magique : il émet en
boucle un petit paquet de **31 octets maximum** qui dit *qui il est* et *ce
qu'il sait faire*. Tout le travail de cette étape a consisté à faire entrer dans
ces 31 octets ce que la conception exige — l'identifiant du service, qui fait
128 bits à lui seul, et une signature courte du nœud — puis à reléguer le nom
lisible dans un second paquet, envoyé seulement à qui le demande.

Le point intéressant à raconter : l'identifiant du service, `6d656e67-2d64-…`,
n'est pas un nombre au hasard. Lu comme du texte, c'est `meng-dengon-v1`. Et il
doit être écrit **à l'envers** dans le code, parce que la bibliothèque Bluetooth
range ces identifiants du dernier octet vers le premier. Se tromper de sens
compile parfaitement, démarre parfaitement, et produit un appareil que personne
ne reconnaît — le genre de bogue qu'aucun compilateur ne peut attraper, et
qu'on ne voit qu'en regardant la carte depuis un téléphone.
