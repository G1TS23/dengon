# dengon — Glossaire, bibliographie & annexes

> Fait partie du **contexte global** de `docs/synthese/` — point d'entrée :
> [`00-contexte-global.md`](00-contexte-global.md). Questions encore ouvertes :
> [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).

---

## 1. Glossaire

| Terme | Définition |
| --- | --- |
| **dengon (伝言)** | « message que l'on confie à quelqu'un pour qu'il le transmette ». Nom de travail du projet. |
| **BLE** | Bluetooth Low Energy. Radio courte portée, basse consommation, base du réseau. |
| **Maillage / mesh** | Réseau où chaque nœud relaie les messages des autres, sans point central. |
| **Nœud** | Un appareil qui fait tourner dengon (téléphone ou ESP32). |
| **Voisin** | Un nœud actuellement à portée Bluetooth directe. |
| **Saut / hop** | Un passage d'un nœud à un voisin. |
| **DTN** | *Delay-Tolerant Network*. Fonctionne même sans chemin complet à un instant donné : on stocke et on retransmet plus tard. |
| **Store-carry-forward** | Garder un message, le transporter physiquement, le retransmettre à la prochaine rencontre. |
| **Bundle Protocol v7 (RFC 9171)** | Standard IETF du DTN. Notions réutilisées : `bundle`, `lifetime`, `custody transfer`. |
| **Routage épidémique / gossip** | Propagation « de proche en proche » : à chaque rencontre, deux nœuds échangent ce qu'ils n'ont pas en commun. |
| **Managed flooding** | Diffusion à tous les voisins + relais, bornée par TTL + cache anti-doublon (routage de Bluetooth Mesh, de Bitchat, de Meshtastic). |
| **Spray-and-Wait** | Ne diffuser qu'un nombre limité **L** de copies, puis attendre. Prévu pour le budget de copies des enveloppes (**cible v2**). |
| **TTL (time to live)** | Compteur de sauts d'un paquet ; décrémenté à chaque relais, jeté à 0. TTL initial = **7** (`TTL_DEFAULT`). |
| **Déduplication / seen-set / table « déjà vu »** | Mémoire des messages déjà vus (par `msgID`) pour ne pas les relayer en boucle. |
| **Jitter de relais** | Petit délai aléatoire avant de relayer, pour que les doublons s'annulent (« écouter avant de rediffuser »). `RELAY_JITTER_MS = 10..=220`. |
| **Clamp de densité** | `ttl' = min(ttl-1, TTL_CLAMP_DENSE=5)` si ≥ 6 voisins — limite l'inondation dans les zones denses. |
| **Anti-inondation** | Un voisin qui dépasse `FLOOD_MAX_PER_MIN_PEER = 20` nouveaux `msgID`/min est temporairement ignoré. |
| **peerID** | Identifiant court d'un nœud = `SHA-256(pub_static)[0..8]`, **8 octets** (décision A-8). Stable, pseudonyme. |
| **fingerprint / empreinte** | `SHA-256(pub_static ‖ pub_sign)`, 32 octets. Base du code de vérification. |
| **msgID** | Identifiant d'un paquet L3 = `SHA-256(sender_id ‖ timestamp_ms ‖ type ‖ payload)` (32 o). Dédup + suivi. Tracé **haché** vers le dashboard (`msg_log_id = SHA-256(msgID)[0..16]`). |
| **msg_uuid** | Identifiant d'un **message applicatif**, stable de bout en bout (16 o, UUIDv4). C'est lui que suit la machine à états des statuts (le `msgID` change si le paquet est re-scellé). |
| **id_message** (olivier) | Identifiant de message dans le brouillon `olivier/format-trame` (16 o aléatoires). **Remplacé** par le couple `msgID` + `msg_uuid` de `powl/03` (A-9). |
| **AppFrame** | Contenu applicatif L4 (à l'intérieur de Noise) : `Message`, `Ack`, `ReadReceipt` (v2), `Profile` (post-MVP). |
| **Noise (XX / X)** | Cadre standard pour un canal chiffré. `XX` = session authentifiée bidirectionnelle (forward secrecy) ; `X` = message one-shot scellé vers une clé connue (enveloppe scellée, pas de FS). |
| **Forward secrecy** | Voler une clé aujourd'hui ne permet pas de déchiffrer les messages d'hier. Assurée sur la session `XX`, pas sur les enveloppes `X`. |
| **Ed25519 / X25519** | Ed25519 = signatures ; X25519 = accord de clés. Même courbe (Curve25519), usages différents. |
| **AEAD** | Chiffrement authentifié (ChaCha20-Poly1305) : chiffre **et** protège l'intégrité en une opération. |
| **XChaCha20-Poly1305** | Variante d'AEAD à nonce long (192 bits). Utilisée pour le chiffrement **champ par champ** de la base locale (B-3). |
| **Enveloppe scellée** (`SEALED_ENVELOPE`) | Message chiffré pour un destinataire absent, laissé sur des relais jusqu'à son retour. Noise `X`. |
| **recipient_tag** | Tag anonyme **tournant** (`HMAC-SHA256(pub_static_dest, "dengon-tag" ‖ day_u32)[0..16]`, change chaque jour) désignant le destinataire d'une enveloppe sans révéler qui. |
| **Budget de copies** (`copy_budget`) | Nombre max de copies d'une enveloppe en circulation (Spray-and-Wait). `COPY_BUDGET_INIT = 4`. **Cible v2** ; au MVP, flood borné + dédup. |
| **Outbox / file de retransmission** | File locale des messages envoyés-mais-pas-confirmés-livrés ; rejouée à chaque reconnexion. Taille max ~50 (ESP32) / ~300 (téléphone). |
| **Inventaire** (`INVENTORY`) | Paquet listant les `msgID` détenus, échangé entre voisins qui se rencontrent, pour n'envoyer que ce qui manque à l'autre. Mécanisme de réconciliation **du MVP**. |
| **Fragmentation (L2)** | Découpage d'un paquet > MTU BLE en fragments (`frag_id`, `index`, `total`), réassemblés par le récepteur. `FRAG_SIZE = 440`. Pas de checksum par fragment (C-6). |
| **GATT / GAP** | Couches BLE : GATT = services & caractéristiques (données) ; GAP = découverte & rôles (central/peripheral). |
| **Central / Peripheral** | Rôles BLE : central scanne et se connecte ; peripheral s'annonce et accepte des connexions. Un nœud mesh doit être **les deux à la fois**. |
| **MTU** | Taille max d'un paquet applicatif BLE (~20 à 512 octets ; négocier `ATT_MTU = 517`, repli 23). Charge utile réelle par écriture à mesurer au Spike C. |
| **ANNOUNCE** | Paquet de présence (`0x01`) : `peerID`, clés publiques, pseudo, `ledger_height`, `caps`. |
| **GOSSIP_FILTER / _PULL / _PUSH** | Paquets de réconciliation par filtre compact (GCS). **Cible v2** ; au MVP on utilise `INVENTORY` (liste brute). |
| **GCS (Golomb-Coded Set)** | Filtre probabiliste compact (~20–30 % plus petit qu'un Bloom filter à même taux de faux-positifs). Paramètre `p = 1/64`. Cible v2. |
| **LOG_ATTEST** | Paquet (`0x0A`) diffusé périodiquement : `{ ledger_root, height, node_pub_sign }`. Capté par les relais → VPS. |
| **TOFU** | *Trust On First Use* : faire confiance à la première clé vue pour un contact, alerter si elle change. |
| **Code de vérification / safety number** | Chaîne de 60 chiffres identique des deux côtés, comparée hors bande, pour détecter un intercepteur au premier contact (C-12). |
| **Journal chaîné (hash-chain / ledger)** | Liste d'événements où chaque entrée contient le hash de la précédente : impossible d'en modifier une sans casser la chaîne. Local à chaque nœud. |
| **ledger_root** | `entry_hash` de la dernière entrée = résumé de tout l'historique d'un nœud. |
| **Attestation** | Diffusion par un nœud du résumé (racine) de son journal ; d'autres la rapportent au dashboard, rendant le mensonge détectable. |
| **Fork (de journal)** | Un nœud présente deux historiques différents pour la même hauteur → signe de triche. |
| **Statuts** | `en attente` (`QUEUED`) → `parti` (`IN_FLIGHT`) → `distribué` (`DELIVERED`) (+ `échec/expiré` = `EXPIRED`, `annulé` = `CANCELLED`). `lu` (`READ`) = **v2** (A-10). |
| **ACK** | Accusé signé : « reçu par l'appareil destinataire ». Voyage dans la session Noise ou en enveloppe scellée. |
| **conv_seq** | Compteur par conversation (par expéditeur) → détection de trous, ordre causal, anti-rejeu (C-7). |
| **conv_hash** | `SHA-256(min(peerA,peerB) ‖ max(peerA,peerB))[0..8]` : groupe une conversation sans savoir qui elle implique. |
| **Padding** | Complétion PKCS#7 des paquets chiffrés vers `PAD_BUCKETS = [256, 512, 1024, 2048]`. |
| **Relais / `dengon-relay`** | Nœud fixe (ESP32-WROOM) branché au secteur : densifie le mesh, cache, dépose des enveloppes, remonte les logs en HTTPS. Ne déchiffre rien. |
| **`dengon-core`** | Bibliothèque Rust contenant toute la logique (protocole, crypto, stockage, synchro, journal). **Seule implémentation**, partagée par l'app, le nœud CLI et le firmware (A-2). |
| **`dengon-node`** | Nœud en ligne de commande sans UI : pour les tests et comme nœud fixe. |
| **`dengon-sim`** | Simulateur multi-nœuds (transport en mémoire, partitions, churn). |
| **`dengon-ffi`** | Bindings UniFFI (génère Kotlin ; Swift en v2). |
| **`dengon-verify`** | Petit binaire Rust du workspace qui vérifie les journaux chaînés (`verify_chain`) ; appelé en sous-processus par le dashboard FastAPI (A-5). |
| **Transport (trait)** | Interface qui cache la radio : `dengon-core` envoie/reçoit des octets sans savoir si Android, un PC ou un ESP32 est derrière. Unique couture entre le cœur et les plateformes. |
| **UniFFI** | Outil (Mozilla) qui génère automatiquement le « pont » pour appeler du Rust depuis Kotlin (et Swift). Ce qui rend l'ouverture iOS peu coûteuse. |
| **Observabilité** | Capacité à comprendre ce que fait le système depuis l'extérieur, via les logs/traces. Ici : le dashboard. |
| **Dashboard / VPS** | Serveur qui **observe** le réseau (parcours des messages, santé). Ne transporte aucun message, ne voit aucun contenu. Stack : FastAPI + SSE + SQLite (A-5). |
| **SSE (Server-Sent Events)** | Canal temps réel **sens unique** serveur → navigateur (plus simple qu'un WebSocket). Utilisé par le dashboard. |
| **HTTPS POST par batch** | Mode de remontée des logs des nœuds vers le dashboard (A-6) : `POST /ingest/batch`, JSON canonique signé Ed25519, auth par jeton/JWT. Remplace MQTT. |
| **MQTT** | Protocole publish/subscribe léger de l'IoT. **Envisagé puis écarté** pour dengon (pas de broker à opérer — A-6). |
| **TimescaleDB** | Extension PostgreSQL pour données horodatées. **Envisagée puis écartée** pour le dashboard (SQLite suffit — A-5). |
| **ESP-IDF / NimBLE** | ESP-IDF = SDK officiel ESP32. NimBLE = pile Bluetooth légère (host BLE) du firmware (~40 Ko de RAM de moins que Bluedroid). |
| **PSRAM** | RAM supplémentaire sur certains ESP32 (WROVER). Le projet utilise des **WROOM sans PSRAM** → quotas réduits (A-4). |
| **eFuse / Secure Boot / flash encryption** | Mécanismes de sécurisation matérielle de l'ESP32 (clé de relais en NVS chiffrée, firmware signé). |
| **E2EE** | Chiffrement de bout en bout (seuls l'expéditeur et le destinataire lisent le contenu). |
| **Merkle (arbre de)** | Résumé d'un lot de données par une racine ; preuve d'inclusion en `log₂(n)` hachages. Optionnel pour les accusés groupés / l'intégrité des fragments. |

---

## 2. Bibliographie complète

> Base : `oswin/06-sources-references.md` (consultée le 8 septembre 2026),
> complétée par `powl/01 §1.5` et `olivier/etude-stack.md`. ⭐ = plus utile pour
> démarrer. Revérifier chaque lien à la rédaction finale et privilégier les
> sources primaires (RFC, spécifs officielles, articles académiques) pour les
> points sensibles.

### 2.1 Applications de messagerie mesh (études de cas)

- ⭐ **Bitchat** (la référence à imiter — BLE mesh + TTL + store-and-forward +
  crypto moderne) :
  - dev.to – Offline messaging reinvented with Bitchat — https://dev.to/grenishrai/offline-messaging-reinvented-with-bitchat-5011
  - TechTarget – What is Bitchat — https://www.techtarget.com/whatis/feature/What-is-Bitchat
  - TechRadar – how Bitchat works — https://www.techradar.com/phones/bitchat-is-a-new-private-bluetooth-messaging-app-that-doesnt-need-the-internet-heres-how-it-works
  - CNBC – Jack Dorsey launches a Bluetooth messaging rival — https://www.cnbc.com/2025/07/07/jack-dorsey-whatsapp-bluetooth.html
  - BeInCrypto – Bitchat expliqué — https://beincrypto.com/learn/bitchat-bluetooth-bitcoin-app/
  - `WHITEPAPER.md` — github.com/permissionlesstech/bitchat
- ⭐ **Bridgefy** (le contre-exemple de sécurité à étudier) :
  - « Breaking Bridgefy » — version abrégée (PDF) — https://martinralbrecht.wordpress.com/wp-content/uploads/2020/08/bridgefy-abridged.pdf
  - Article complet — eprint IACR 2021/214 (PDF) — https://eprint.iacr.org/2021/214.pdf
  - Springer – Mesh Messaging in Large-Scale Protests: Breaking Bridgefy — https://link.springer.com/chapter/10.1007/978-3-030-75539-3_16
  - Royal Holloway – communiqué — https://www.royalholloway.ac.uk/research-and-education/subjects/information-security/news/using-messaging-service-bridgefy-could-have-dire-consequences-for-users-if-privacy-protection-issues-aren-t-fixed/
  - Bridgefy SDK Android — https://github.com/bridgefy/sdk-android
- **Briar** — code.briarproject.org, audit Cure53 ; BLE + Wi-Fi Direct + Tor —
  https://havenmessenger.com/blog/posts/mesh-networking-briar/
- *Survey of Mesh Networking Messengers* — TUM NET-2021-05-1

### 2.2 Bluetooth & Bluetooth Mesh

- ⭐ Bluetooth SIG – Mesh Networking Primer — https://www.bluetooth.com/bluetooth-mesh-networking-primer/
- ⭐ Novel Bits – Bluetooth Mesh: the ultimate guide — https://novelbits.io/bluetooth-mesh-networking-the-ultimate-guide/
- Bluetooth SIG – Directed Forwarding — https://www.bluetooth.com/mesh-directed-forwarding/
- Bluetooth SIG – Mesh FAQ — https://www.bluetooth.com/learn-about-bluetooth/topology-options/le-mesh/mesh-faq/
- MokoSmart – What is Bluetooth Mesh & how it works — https://www.mokosmart.com/what-is-bluetooth-mesh-how-it-works/
- MathWorks – Bluetooth Mesh Flooding in WSN — https://www.mathworks.com/help/bluetooth/ug/bluetooth-mesh-flooding-in-wireless-sensor-networks.html
- Google Patents – Managed flooding for Bluetooth mesh (US20200314735A1) — https://patents.google.com/patent/US20200314735A1/en
- Meshtastic – « managed flood routing » — https://meshtastic.org/blog/why-meshtastic-uses-managed-flood-routing/ et https://meshtastic.org/docs/overview/mesh-algo/
- Argenox – Android 5.0 BLE improvements — https://argenox.com/blog/android-5-0-lollipop-brings-ble-improvements
- Argenox – pourquoi le BLE Mesh peine à percer — https://argenox.com/blog/10-reasons-why-ble-mesh-has-struggled-to-gain-traction

### 2.3 Réseaux tolérants aux délais (DTN) & store-and-forward

- ⭐ RFC 9171 – Bundle Protocol Version 7 (texte intégral) — https://www.rfc-editor.org/rfc/rfc9171.html
- RFC 9171 – page d'information RFC Editor — https://www.rfc-editor.org/info/rfc9171/
- EmergentMind – Delay/Disruption Tolerant Network protocols — https://www.emergentmind.com/topics/delay-disruption-tolerant-network-dtn-protocols
- DTN7 – implémentation open source du Bundle Protocol — https://dtn7.github.io/
- Vahdat, Becker – *Epidemic Routing for Partially-Connected Ad Hoc Networks*, 2000
- *A Secure Epidemic Routing using Blockchain in Opportunistic IoT* — Springer, 2020
- Mots-clés : « epidemic routing », « spray-and-wait », « PRoPHET DTN routing ».

### 2.4 Cryptographie & sécurité des messages

- ⭐ Signal – The Double Ratchet Algorithm (spécification) — https://signal.org/docs/specifications/doubleratchet/
- Signal Protocol – Wikipedia — https://en.wikipedia.org/wiki/Signal_Protocol
- Noise Protocol Framework — https://noiseprotocol.org/ (utilisé par Bitchat, WireGuard…)
- OMEMO – Wikipedia — https://en.wikipedia.org/wiki/OMEMO
- positive-intentions – Adapting the Signal Protocol for P2P — https://positive-intentions.com/blog/p2p-signal-protocol/
- Bibliothèque de référence : **libsodium/NaCl** (X25519, Ed25519, XChaCha20-Poly1305) — C/Arduino, JavaScript, Python.
- Crates Rust retenus : `ed25519-dalek` v2, `x25519-dalek`, `snow`, `chacha20poly1305`, `sha2`, `hmac`, `hkdf`, `getrandom`, `rusqlite`.

### 2.5 Blockchain, hash chains & arbres de Merkle

- Medium – Blockchain, Hash & Merkle tree : immutability & integrity — https://medium.com/@zlhk100/blockchain-hash-and-merkle-tree-data-immutability-and-integrity-append-only-database-eff7b621b9c3
- GeeksforGeeks – Blockchain Merkle Trees — https://www.geeksforgeeks.org/blockchain-merkle-trees/
- HackerNoon – Merkle Trees & cryptographic accumulators — https://hackernoon.com/merkle-trees-and-cryptographic-accumulators-the-mathematical-backbone-of-blockchain-integrity
- DEV – Merkle tree root for data integrity — https://dev.to/bloxbytes/understanding-the-concept-of-merkle-tree-root-in-blockchain-for-data-integrity-2hp0

### 2.6 Arduino / ESP32 & dashboard IoT

- ⭐ Random Nerd Tutorials – ESP32 MQTT Publish/Subscribe (Arduino IDE) — https://randomnerdtutorials.com/esp32-mqtt-publish-subscribe-arduino-ide/
- ⭐ Hackster – Connect ESP32 to ThingsBoard over Wi-Fi — https://www.hackster.io/norvi/connect-esp32-to-thingsboard-over-wi-fi-visualize-iot-data-eae2ed
- ThingsBoard – client SDK Arduino/ESP32 — https://github.com/thingsboard/thingsboard-client-sdk
- Zbotic – ThingsBoard IoT platform avec ESP32 — https://zbotic.in/thingsboard-iot-platform-with-esp32-open-source-dashboard/
- GitHub – ESP32 ThingsBoard IoT Dashboard (exemple) — https://github.com/hubamatyas/ESP32-Thingsboard-IoT-Dashboard
- FlowFuse – Interacting with ESP32 using Node-RED and MQTT (2026) — https://flowfuse.com/blog/2024/11/esp32-with-node-red/
- oh2mp/esp32_ble2mqtt – passerelle BLE → MQTT — https://github.com/oh2mp/esp32_ble2mqtt
- Theengs OpenMQTTGateway — https://docs.openmqttgateway.com/
- Datasheet ESP32-WROOM-32E (Espressif) — https://www.espressif.com/sites/default/files/documentation/esp32-wroom-32e_esp32-wroom-32ue_datasheet_en.pdf
- Datasheet HTML — https://documentation.espressif.com/esp32-wroom-32e_esp32-wroom-32ue_datasheet_en.html
- Grid Connect – module 4MB Flash — https://www.gridconnect.com/products/esp32-wroom-32e-combo-wi-fi-bt-ble-module
- Espressif – *ESP-BLE-MESH Architecture* — https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/esp-ble-mesh/ble-mesh-index.html
- ESP-IDF – NimBLE vs Bluedroid (empreinte mémoire) — https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/ble/overview.html
- NimBLE-Arduino (empreinte réduite) — https://osrtos.com/projects/nimble-arduino/
- thingshost – ThingsBoard vs Grafana (2026) — https://thingshost.de/en/blog/thingsboard-vs-grafana
- wz-it – plateformes IoT open source comparées — https://wz-it.com/en/knowledge/iot/open-source-iot-platforms-compared/
- wz-it – Node-RED MQTT dashboard — https://wz-it.com/en/knowledge/iot/node-red-mqtt-dashboard/
- Steve's Internet Guide – IoT MQTT dashboards — http://www.steves-internet-guide.com/iot-mqtt-dashboards/
- rweather – AES accéléré ESP32 — https://rweather.github.io/arduinolibs/AESEsp32_8cpp_source.html
- Arduino Cryptography Library – Ed25519 (rweather) — https://rweather.github.io/arduinolibs/classEd25519.html
- libsodium sur ESP32 (forum) — https://esp32.com/viewtopic.php?t=9243

### 2.7 App mobile & bibliothèques BLE

- flutter_blue_plus — https://pub.dev/packages/flutter_blue_plus et https://github.com/chipweinberger/flutter_blue_plus
- bluetooth_low_energy (Flutter, central + peripheral) — https://pub.dev/packages/bluetooth_low_energy
- ble_peripheral (Flutter) — https://pub.dev/packages/ble_peripheral et https://github.com/rohitsangwan01/ble_peripheral
- react-native-ble-plx (central only) — https://github.com/dotintent/react-native-ble-plx/blob/master/README.md
- react-native-ble-plx – mode arrière-plan iOS — https://github.com/dotintent/react-native-ble-plx/wiki/Background-mode-(iOS)
- react-native-peripheral — https://github.com/petrbela/react-native-peripheral
- react-native-ble-advertiser (non maintenu) — https://www.npmjs.com/package/react-native-ble-advertiser
- iOS « overflow area » pour l'annonce en arrière-plan — https://github.com/davidgyoung/ios-overflow-area et https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/CoreBluetoothBackgroundProcessingForIOSApps/PerformingTasksWhileYourAppIsInTheBackground.html
- Android 14 – types de services de fond obligatoires — https://developer.android.com/about/versions/14/changes/fgs-types-required et https://developer.android.com/develop/background-work/services/fgs/service-types
- Android 15 – restrictions scan BLE en arrière-plan — https://bleadvertiserapp.medium.com/android-15-broke-your-ble-app-new-permission-rules-3d8cb3c9ba86
- Android – communiquer en arrière-plan (doc officielle) — https://developer.android.com/develop/connectivity/bluetooth/ble/background
- Android Open Source Project – BLE advertising — https://source.android.com/docs/core/connect/bluetooth/ble_advertising
- liste de modèles sans mode périphérique (Corona-Warn-App) — https://github.com/corona-warn-app/cwa-app-android/issues/688
- flutter_sodium / libsodium pour Flutter — https://github.com/firstfloorsoftware/flutter_sodium
- pub.dev – sodium_libs — https://pub.dev/packages/sodium_libs ; pub.dev – sodium — https://pub.dev/packages/sodium
- Rust partagé mobile via UniFFI — https://github.com/ivnsch/rust_android_ios et https://appstimes.in/rust-in-mobile-development-2026-is-it-ready-to-replace-kotlin-and-swift-for-core-logic/
- Medium – BLE avec Flutter + Arduino — https://medium.com/@danielwolf.dev/get-started-with-bluetooth-low-energy-using-flutter-arduino-bdf5d790edc

### 2.8 Comment citer dans le rapport (exemples)

- Norme mesh : *Bluetooth SIG, « Bluetooth Mesh Networking Primer », bluetooth.com.*
- DTN : *S. Burleigh et al., « RFC 9171 – Bundle Protocol Version 7 », IETF, 2022.*
- Sécurité (contre-exemple) : *M. Albrecht, J. Blasco, R. B. Jensen, L. Mareková, « Mesh Messaging in Large-Scale Protests: Breaking Bridgefy », CT-RSA, 2021.*
- E2EE : *M. Marlinspike, T. Perrin, « The Double Ratchet Algorithm », Signal, 2016.*

---

## 3. Correspondance fichier synthèse ↔ fichiers sources

| Fichier de `docs/synthese/` | `docs/powl/` | `docs/oswin/` | `docs/olivier/` |
| --- | --- | --- | --- |
| `00-contexte-global.md` | `README.md`, `01 §Récap` | `README.md`, `00` | `README.md`, `CONTEXT.md`, `mise-en-commun.md` |
| `02-probleme-et-besoins.md` | `00-overview.md` | `00-vue-ensemble-architecture.md` | `CONTEXT.md`, `analyse-besoins.md`, `decisions-v1.md`, `mise-en-commun.md` |
| `03-etat-de-lart.md` | `01-benchmarks.md §1` | `00`, `01-bluetooth-ble-mesh.md`, `02 §4`, `03-store-and-forward-dtn.md`, `04-blockchain-analyse-critique.md` | `analyse-besoins.md §6`, `etude-stack.md §2.3, §5` |
| `04-architecture.md` | `02-architecture.md` | `00 §3-4` | `architecture.md` |
| `05-protocole-et-trame.md` | `03-network-protocol.md` **(spec retenue)** | `01 §4`, `03 §4-5`, `02 §3` | `protocole.md` v0.3, `format-trame.md` v0.1 (remplacé, cf. Annexe A) |
| `06-securite.md` | `04-security.md` **(doc de référence)** | `02-securite-messages.md` | `analyse-besoins.md §5`, `protocole.md §10` |
| `07-cycle-de-vie-et-statuts.md` | `05-message-lifecycle.md` | `03 §4` | `protocole.md §7` |
| `08-relais-esp32.md` | `06-relay-esp32.md` | `05-arduino-passerelle-dashboard.md`, `07 §2` | `architecture.md §3` |
| `09-dashboard-et-donnees.md` | `07-dashboard.md`, `08-observability-events.md` **(source de vérité événements)**, `09-data-model.md` **(source de vérité schémas)** | `05 §3-5` | `dashboard.md` v0.2 (partiellement remplacé, cf. Annexe B) |
| `10-benchmarks-mvp-tests.md` | `01-benchmarks.md`, `10-mvp-scope-roadmap.md`, `11-testing-strategy.md` | `07 §7`, `08-feuille-de-route-plan-dev.md`, `09-benchmark-technos.md` | `decisions-v1.md`, `etude-stack.md` v0.1, `mise-en-commun.md §5` |
| `11-glossaire-biblio-annexes.md` | termes disséminés `00`–`11`, `01 §1.5` | `00 §6`, `06-sources-references.md` | `protocole.md §2`, `format-trame.md`, `dashboard.md`, `etude-stack.md §Sources` |

**Vérification de complétude** : les 13 fichiers de `docs/powl/`, les 11 de
`docs/oswin/` et les 10 de `docs/olivier/` (README, CONTEXT, analyse-besoins,
decisions-v1, protocole, format-trame, architecture, dashboard, etude-stack,
mise-en-commun) sont tous référencés par au moins une ligne ci-dessus.

---

## Annexe A — Format de trame `olivier/format-trame.md` v0.1 (pour mémoire)

> **Remplacé par `powl/03`** (décision A-12, voir
> [`05-protocole-et-trame.md`](05-protocole-et-trame.md)). Conservé ici comme
> support pédagogique et source de **vecteurs de test** : la structure
> comportementale (fragment BLE, PDU, corps par type) reste un bon banc d'essai.

**Conventions** : ordre des octets **big-endian** ; entiers non signés sauf
mention ; champ « réservé » = 0 à l'émission, ignoré à la réception. Deux
niveaux : **PDU dengon** (unité complète, chiffrée/relayée/dédupliquée) et
**Fragment BLE** (un morceau de PDU envoyé en un envoi Bluetooth).

**Fragment BLE — en-tête = 24 octets**

| Décalage | Taille | Champ | Description |
| ---: | ---: | --- | --- |
| 0 | 1 | `version_proto` | **= 1** |
| 1 | 1 | `drapeaux` | bit 0 = corps chiffré ; bits 1-7 réservés |
| 2 | 16 | `id_message` | identifiant unique et stable (128 bits aléatoires). Dédup + corrélation de l'accusé. Jamais modifié par un relais |
| 18 | 2 | `index_fragment` | numéro du fragment, à partir de 0 |
| 20 | 2 | `nombre_fragments` | total de fragments du PDU (≥ 1) |
| 22 | 2 | `longueur_charge` | octets utiles dans ce fragment |
| 24 | `longueur_charge` | `charge_fragment` | tranche du PDU dengon |

Avec une taille utile BLE négociée d'environ **180 octets**, ~**156 octets** de
charge par fragment (à calibrer). Recollage : concaténer les `charge_fragment`
dans l'ordre des `index_fragment`. Délai max d'attente : ~30 s.

**PDU dengon — en-tête commun = 44 octets**

| Décalage | Taille | Champ | Description |
| ---: | ---: | --- | --- |
| 0 | 1 | `type_pdu` | **1** = DONNÉES, **2** = ACCUSÉ, **3** = INVENTAIRE |
| 1 | 1 | `sauts_restants` | TTL. **7** à l'émission. −1 par relais. À 0, on ne relaie plus |
| 2 | 16 | `empreinte_expediteur` | empreinte de clé publique de l'émetteur du PDU |
| 18 | 16 | `empreinte_destinataire` | empreinte de clé publique du destinataire du PDU |
| 34 | 8 | `horodatage_envoi` | ms depuis 1970-01-01 UTC. Best-effort : affichage + expiration (~24 h) |
| 42 | 1 | `version_contenu` | **= 1** |
| 43 | 1 | réservé | 0 |
| 44 | … | `corps` | dépend de `type_pdu` |

> Adressage : pour un PDU DONNÉES, `empreinte_expediteur` = auteur du message,
> `empreinte_destinataire` = lecteur. Pour un PDU ACCUSÉ, c'est **l'inverse**.

**Corps DONNÉES (`type_pdu` = 1)** — bit « chiffré » = 1

| Décalage | Taille | Champ |
| ---: | ---: | --- |
| 0 | 24 | `nonce` |
| 24 | 16 | `etiquette_auth` |
| 40 | M | `contenu_chiffre` (texte UTF-8 ≤ ~500 caractères une fois déchiffré) |

Primitive envisagée par `olivier` : libsodium `crypto_box`. → **remplacé** :
`powl` utilise Noise (voir [`06-securite.md`](06-securite.md)).

**Corps ACCUSÉ (`type_pdu` = 2)** — a son propre `id_message`, chiffré pour
l'auteur d'origine. Contenu **en clair** après déchiffrement :

| Décalage | Taille | Champ |
| ---: | ---: | --- |
| 0 | 16 | `id_message_confirme` |
| 16 | 1 | `statut` (**3** = DISTRIBUÉ ; **4** = LU, réservé v2) |
| 17 | 8 | `horodatage_reception` |
| 25 | 64 | `signature_destinataire` |

**Corps INVENTAIRE (`type_pdu` = 3)** — non chiffré, corps =
`nombre_entrees(2) ‖ ids(16 × nombre_entrees)`. À réception, le nœud envoie à ce
voisin les messages dont l'`id_message` n'apparaît pas dans la liste reçue. Une
file pleine côté téléphone (~300 messages) = ~4,8 Ko d'identifiants, ~30
fragments.

**Constantes du brouillon `olivier` (v0.3)** : `version_proto` = 1 ;
`version_contenu` = 1 ; TTL initial = 7 ; `id_message` 16 o aléatoires ; texte
≤ ~500 caractères ; charge utile fragment ~156 o ; réassemblage ~30 s ;
expiration ~24 h ; « écouter avant de rediffuser » ~50-500 ms ; anti-inondation
~20 nouveaux `id_message`/min/voisin ; file ~50 (ESP32) / ~300 (téléphone).

**Exemple de tailles — message de 500 caractères** :

```text
texte en clair              ~700 octets
+ nonce (24) + étiquette (16)   40 octets
= contenu chiffré           ~740 octets
+ en-tête PDU                 44 octets
= PDU DONNÉES               ~784 octets
découpage en fragments de ~156 octets utiles :  ⌈784 / 156⌉ = 6 fragments
```

**Proposition de format `oswin` (`oswin/02 §3`)** — pour mémoire :

```text
EN-TÊTE EN CLAIR (lisible par les relais, pour router)
  - id_message        (aléatoire, unique)
  - ttl               (nb de sauts restants)
  - expire_le         (horodatage d'expiration)
  - dest_pubkey_hash  (à qui, sous forme pseudonyme)
  - exp_pubkey_ephem  (clé publique éphémère de l'émetteur)
CHARGE UTILE CHIFFRÉE (AES-256-GCM) — illisible par les relais
  - texte du message
  - numéro de séquence (anti-rejeu)
SIGNATURE Ed25519 sur (en-tête + charge utile)
```

Points d'attention (valables pour `powl/03` aussi) : l'en-tête **doit** être
couvert par l'authentification pour qu'un relais ne puisse pas trafiquer le TTL
ou le destinataire ; numéro de séquence + ID unique bloquent le rejeu ; garder
l'en-tête **minimal** (chaque champ en clair est une fuite de métadonnées).

---

## Annexe B — Dashboard `olivier/dashboard.md` v0.2 (pour mémoire)

> Le principe du **code anonyme différent par message** est **remplacé** par un
> `node_id` pseudonyme **stable** (décision C-4 : A-7 impose une identité de nœud
> stable pour vérifier le journal chaîné par appareil). Le reste (tolérance aux
> événements partiels, statut déduit, maquettes) reste pertinent et est repris
> dans [`09-dashboard-et-donnees.md`](09-dashboard-et-donnees.md).

**Principe d'origine** : bonus non bloquant ; alimenté opportunément (seuls les
nœuds avec du Wi-Fi envoient) ; **anonymisé** — un nœud se désignait par un code
anonyme **différent pour chaque message** (on reconstitue le parcours d'un
message mais on ne relie pas entre eux les messages d'un même appareil) ; mise à
jour automatique de la page ; hébergement = VPS Debian.

**Affichait** : liste des messages suivis (id raccourci, statut, heure de
création, dernière activité) ; détail d'un message (parcours = suite de nœuds en
codes, horodatage de chaque étape, statut) ; compteurs globaux (messages en
circulation, distribués/expirés, taux de distribution). « Nombre d'appareils
actifs » = impossible à déduire avec un code par message → deux options ouvertes
(battement anonyme séparé, ou retirer le compteur). **Avec le `node_id` stable
retenu, ce compteur se dérive naturellement des événements `peer.*` /
`relay.health`.**

**Ne montrait JAMAIS** : contenu, clé publique / empreinte / pseudo réel, qui a
écrit à qui, position physique.

**Maquette d'écrans** :

```text
┌───────────────────────────── dengon · suivi ─────────────────────────────┐
│ Messages suivis                              nœuds actifs : 6            │
│ id        statut           créé        dernière activité                │
│ 7f3a…     En circulation   10:02       10:04   (3 étapes)   ▶           │
│ b1c8…     Distribué        09:51       09:58   (4 étapes)   ▶           │
│ 42de…     Expiré           hier 22:10  hier 22:41            ▶          │
└─────────────────────────────────────────────────────────────────────────┘
Détail du message 7f3a… :
   [ node-K ] créé 10:02
        ▼ relayé 10:03 (sauts restants 6)
   [ node-M ]
        ▼ relayé 10:04 (sauts restants 5)
   [ node-P ]        … en attente de la suite
```

**Replis si le temps manque** (toujours valables) : (1) page rafraîchie à la
main (abandon du canal temps réel) ; (2) maquette statique avec données
d'exemple ; (3) mode démo qui rejoue un scénario enregistré (seulement s'il
reste du temps).

**Outils clés en main envisagés (`oswin/05 §4`)** : ThingsBoard (plateforme IoT,
dashboards prêts, MQTT natif, SDK ESP32) ou Node-RED (flux visuel + dashboard,
pédagogique). **Écartés** (A-5) : mal adaptés à la vérification de journal
chaîné signé et à la reconstruction de parcours par corrélation d'événements
partiels.
