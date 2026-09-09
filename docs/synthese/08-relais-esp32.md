# dengon — Relais ESP32

> Fait partie du **contexte global** de `docs/synthese/` — point d'entrée :
> [`00-contexte-global.md`](00-contexte-global.md). Questions encore ouvertes :
> [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).
>
> **Décisions retenues** : carte **ESP32-WROOM-32E** (Freenove, déjà en
> possession — A-4) ; transport vers le dashboard = **HTTPS POST par batch**,
> pas de MQTT (A-6) ; le relais est un **livrable MVP complet**, pas une simple
> preuve de concept (C-9).

---

## 1. Rôle

Infrastructure **fixe** : branché au secteur, posé en hauteur, toujours allumé.
Densifie le maillage et sert de **mémoire tampon** du réseau.

| Fonction | Détail |
| --- | --- |
| **Relayer** | reçoit des paquets BLE, applique le pipeline de routage (voir [`05-protocole-et-trame.md`](05-protocole-et-trame.md)), rediffuse |
| **Réconcilier** | échange d'inventaire des `msgID` récents avec ses voisins (fenêtre ~6 h) |
| **Déposer (courier)** | stocke les `SEALED_ENVELOPE` pour destinataires absents |
| **Attester** | tient son propre journal chaîné, diffuse `LOG_ATTEST` |
| **Remonter les logs** | Wi-Fi station → **HTTPS POST par batch** → VPS ; buffer en flash si Wi-Fi coupé |

**Ne déchiffre rien** : aucune clé utilisateur. Ne voit que des paquets chiffrés
+ des métadonnées de passage.

## 2. Matériel

**Carte retenue : ESP32-WROOM-32E (module Freenove)** — celle que l'équipe
possède déjà.

| Élément | Valeur | Pourquoi ça compte |
| --- | --- | --- |
| Processeur | **double cœur** Xtensa LX6, jusqu'à **240 MHz** | BLE **et** Wi-Fi en parallèle |
| Mémoire vive | **520 Ko SRAM** (pas de PSRAM) | limite la taille des files → quotas réduits (voir §5) |
| Mémoire flash | **4 Mo** | programme + file persistante (NVS / littlefs) |
| Wi-Fi | **802.11 b/g/n (2,4 GHz)** | passerelle vers le dashboard (HTTPS) |
| Bluetooth | **v4.2 : BR/EDR + BLE** | rôle de relais BLE dans le mesh |
| GPIO | ~34 broches | LED d'état, boutons |
| Alimentation | 5 V par USB / 3,3 V logique | alim USB murale suffit (fixe, pas de batterie) |

> Le Bluetooth et le Wi-Fi **partagent la même radio 2,4 GHz** → activer les deux
> en continu ralentit. Stratégie : le relais BLE prime ; le Wi-Fi est activé par
> **fenêtres** pour flusher les logs par petits lots, puis on revient en BLE.
> La carte Freenove est programmable avec l'IDE Arduino ou PlatformIO ; le
> firmware dengon vise **ESP-IDF** (voir §3).

**Antenne** : PCB ou U.FL externe (U.FL recommandé en déploiement réel).
**Stockage persistant** : flash interne (partition NVS + partition `littlefs`
pour le buffer de logs). **Pas de** LoRa, écran, batterie (hors MVP).

> **Option WROVER (non retenue)** : le module ESP32-WROVER-E ajoute **8 Mo de
> PSRAM**, ce qui permettrait des buffers ~8× plus grands et une meilleure
> coexistence BLE + Wi-Fi. À réévaluer **seulement si** un test de charge
> terrain montre une saturation mémoire ou un débit BLE+Wi-Fi insuffisant sur
> les WROOM (fiche A-4).

## 3. Stack firmware

```text
ESP-IDF v5.x
├── NimBLE (host BLE)                    → rôle GATT double (serveur + client)
├── esp-wifi (station)                   → connexion au réseau du site (par fenêtres)
├── esp_http_client (TLS)                → POST des batchs de logs vers le VPS
├── nvs_flash (chiffré, eFuse)           → clé de relais, config, curseur de journal
├── littlefs                             → buffer de logs hors-ligne (ring)
└── components/dengon_core_ffi/
        └── libdengon_core.a  (Rust, no_std+alloc, cross-compilé xtensa)
             → protocol, routing/dedup, inventaire, ledger, observability
        └── crypto : Rust si Spike A OK ; sinon trait Crypto + mbedTLS pour
                     le seul handshake Noise XX de lien (sha2 + ed25519 restent Rust)
```

**Réutilisation de `dengon-core`** : `protocol`, `sync::routing`,
`sync::inventory`, `ledger`, `observability` compilent en `no_std` + `alloc` →
archivés en `libdengon_core.a`, exposés au C via un header `dengon_core.h`
(généré par `cbindgen`). `store` : implémentation `Store` spécifique ESP32
(index en SRAM, persistance NVS pour le curseur de journal et les enveloppes
critiques). **Spike A obligatoire au Lot 0** pour trancher le curseur Rust/C de
la crypto (B-1).

**Coexistence BLE + Wi-Fi** : Wi-Fi en **mode station uniquement** ; débit
combiné modeste, acceptable (le relais BLE prime, les logs sont bufferisés et
envoyés par petits lots pendant des fenêtres Wi-Fi).

## 4. Architecture logicielle interne

Tâches FreeRTOS : `ble_rx_task` (reçoit trames GATT) → `route_task`
(dengon_core : dedup, TTL, relais) → `inventory_task` (échange d'inventaire,
push du manquant), `courier_task` (stock enveloppes), `ledger_task` (append +
LOG_ATTEST périodique), `ship_task` (buffer → HTTPS POST) ; `wifi_task`
(reconnexion, NTP). Store : SRAM (caches) / NVS (clé, curseur) / littlefs (log
ring). Files inter-tâches `xQueue` bornées (backpressure : si `route_task`
sature, on **droppe des paquets entrants** — jamais des enveloppes déjà
acceptées).

## 5. Budgets mémoire (cible WROOM, 520 Ko SRAM, pas de PSRAM)

| Poste | Taille | Note |
| --- | --- | --- |
| Cache de réconciliation (paquets récents) | ~64 Ko SRAM | ~120 paquets, éviction LRU + 6 h |
| Enveloppes scellées stockées | index SRAM + N plus récentes en NVS/littlefs | plafond `ENVELOPE_STORE_MAX = 64` |
| Seen-set (dedup) | ~48 Ko | 1024 × (32 o hash + méta) |
| Réassemblage fragments | ~32 Ko | `FRAG_MAX_CONCURRENT` réduit |
| Buffer de logs hors-ligne | littlefs, ~256 Ko partition | ring : écrase le plus ancien si plein |
| Pile NimBLE + Wi-Fi | ~64 Ko SRAM | |

Les quotas sont ~8× ceux d'une cible WROVER (`ENVELOPE_STORE_MAX` : 512 → 64).
Si un test de charge sature ces valeurs, envisager une WROVER (§2).

## 6. Connexion au VPS

**Provisioning** : à la fabrication, `idf.py` flashe le firmware, active **flash
encryption** + **secure boot** ; premier boot → génère la paire Ed25519 en NVS
chiffrée ; config Wi-Fi + URL de l'endpoint via fichier de conf flashé ou
provisioning BLE ; l'opérateur enregistre `relay_id` + `pub_sign` dans le
dashboard (liste blanche) et récupère un **jeton/JWT court** pour l'endpoint.

**Remontée des logs (HTTPS POST par batch)** :

| Paramètre | Valeur |
| --- | --- |
| Endpoint | `https://<vps>/ingest/batch` |
| Auth | `Authorization: Bearer <JWT court>` (liste blanche) + **signature Ed25519** du batch |
| Payload | JSON canonique — batch de N événements (`pkt.*`, `envelope.*`, `attest.*`, `relay.health`…) |
| Santé | `relay.health` toutes les 60 s (uptime, RSSI moyen, tailles de buffers, version fw) — dans le même flux |
| Fréquence | fenêtres Wi-Fi ; batchs accumulés entre deux fenêtres |
| Dédup | le VPS déduplique sur `event_id` (`SHA-256(node ‖ seq)`) → un POST rejoué ne crée pas de doublon |
| Hors-ligne | accumulation dans le ring littlefs ; flush FIFO à la reconnexion, débit limité |

**Horloge** : NTP au boot + resync toutes les heures. Les événements portent
`ts_ms` local ; le VPS ajoute `ingested_ms` et signale les dérives
(`node.clock_skew`).

## 7. Comportement en cas de panne

| Panne | Comportement |
| --- | --- |
| Wi-Fi coupé | relais BLE continue normalement ; logs bufferisés ; retry Wi-Fi backoff |
| Endpoint injoignable | idem ; buffer ring |
| Buffer de logs plein | on écrase les plus anciens logs (le routage prime) ; compteur `logs_dropped` remonté ensuite |
| SRAM saturée | éviction LRU du cache de réconciliation ; refus de **nouvelles** enveloppes (existantes protégées) ; `ledger.append("relay.overloaded")` |
| Redémarrage | recharge clé + curseur de journal depuis NVS ; le journal **reprend à `seq` suivant** (continuité de chaîne préservée) ; caches volatils repartent vides |
| Reset d'usine | nouvelle identité de relais → doit être ré-enregistré au dashboard |

## 8. Ce que le relais journalise

`pkt.seen`, `pkt.relayed`, `pkt.dropped` (raison), `pkt.rejected` (sig
invalide) ; `envelope.stored`, `envelope.handoff`, `envelope.delivered`,
`envelope.expired` ; `peer.connected`, `peer.disconnected` (avec RSSI, `peerID`
du pair) ; `attest.emitted` (sa propre racine), `attest.observed` (racine d'un
autre nœud) ; `relay.boot`, `relay.overloaded`, `relay.wifi_up`,
`relay.wifi_down` ; `node.clock_skew`. **Jamais** : contenu, `msg_uuid`,
`recipient` identifiable ; le `msgID` est **haché** avant émission.

## 9. Rôle passerelle BLE → IP

L'ESP32 relie le monde Bluetooth (le mesh) au monde Internet (le dashboard) : il
reçoit les paquets en BLE, **extrait les métadonnées** (id haché, sauts,
horodatage, état), les POST en HTTPS via Wi-Fi vers le dashboard.
**Règle de sécurité** : la passerelle remonte des métadonnées de supervision,
**pas** le texte des messages, sinon on casse le chiffrement de bout en bout.

**Scénario de démo minimal en 4 temps** : (1) A chiffre + signe + publie un
message en BLE ; (2) l'ESP32 (hors de portée de B) le reçoit, le **stocke**,
**POST ses métadonnées** → « message reçu au nœud Arduino, 1 saut » ; (3) B entre
dans la portée de l'ESP32 → retransmission → B déchiffre et répond par un ACK
signé ; (4) l'ACK remonte, l'ESP32 **purge** sa copie et POST « message livré,
purge ». Ce scénario démontre **les 6 points** du cahier des charges en une
manipulation.

**Pièges pratiques** : RAM limitée (files bornées, quota + purge) ; Wi-Fi + BLE
simultanés (radio 2,4 GHz partagée → fenêtres Wi-Fi) ; sécurité de la passerelle
(HTTPS + jeton + signature) ; alimentation (ESP32 en scan BLE continu + Wi-Fi
consomme → bonne alim USB murale).

> `oswin/05` proposait des dashboards clés en main **ThingsBoard** ou
> **Node-RED** (+ broker MQTT). **Écartés** (A-5) : mal adaptés à la vérification
> de journal chaîné et à la reconstruction de parcours par corrélation
> d'événements partiels. Conservé pour mémoire.
