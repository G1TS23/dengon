# dengon — Choix techniques, périmètre MVP & tests

> Fait partie du **contexte global** de `docs/synthese/` — point d'entrée :
> [`00-contexte-global.md`](00-contexte-global.md). Questions encore ouvertes :
> [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).
> Les tableaux comparatifs ci-dessous sont la **recherche qui a informé les
> décisions** ; la synthèse des décisions est dans la table de
> [`00-contexte-global.md`](00-contexte-global.md).

---

## 1. Décisions techniques retenues

| Sujet | Décision | Réf |
| --- | --- | --- |
| Nature du réseau | DTN gossip + **journal chaîné signé** par appareil, audité par le dashboard, sans consensus | A-7, A-15 |
| Cœur logique | **un seul `dengon-core` en Rust** (`no_std + alloc`), partagé app / CLI / firmware via UniFFI + lib C | A-2 |
| App mobile | **Android : Kotlin natif + Jetpack Compose** ; iOS (Swift/SwiftUI) en v2, même cœur | A-1, A-11 |
| Crypto | **Noise `XX` + Noise `X` + Ed25519** en Rust ; repli `trait Crypto` + mbedTLS limité au lien BLE ESP32 | A-3, B-1 |
| Base locale chiffrée | **XChaCha20-Poly1305 champ par champ** (pas SQLCipher) | B-3 |
| Format de trame | **`powl/03`** unique (12 types + `INVENTORY` MVP) | A-12 |
| Routage | socle flood + TTL 7 + seen-set + jitter + clamp densité + quotas + anti-inondation 20/min ; **échange d'inventaire** au MVP, GCS + Spray-and-Wait en cible v2 | A-13 |
| Carte relais | **ESP32-WROOM-32E** (Freenove, existant) ; WROVER si saturation constatée | A-4 |
| Firmware | **ESP-IDF (C) + NimBLE** + `libdengon_core.a` | — |
| Dashboard | **FastAPI (Python) + SSE + SQLite** + binaire `dengon-verify` (Rust) | A-5, B-5, C-8 |
| Transport nœud → dashboard | **HTTPS POST par batch** (signé), pas de MQTT | A-6 |
| Auth des nœuds | jeton/JWT court (liste blanche) + signature Ed25519 des batchs | B-2, C-5 |
| Statut « Lu » | **reporté en v2** | A-10 |
| Dépôt | workspace `crates/` de `powl/02` + `crates/dengon-verify`, `dashboard/` en Python | A-14 |

## 2. Comparatifs (recherche)

### 2.1 « Le réseau ressemble à une blockchain ? »

Voir [`03-etat-de-lart.md`](03-etat-de-lart.md) §9-10. Formulation retenue :
*DTN à routage gossip + micro-registre local chaîné et signé par appareil,
agrégé et audité par le dashboard.* Le registre sert à l'audit, pas au routage.

### 2.2 Plateforme de l'app cliente

Contrainte déterminante = un vrai nœud mesh doit être **simultanément** GATT
peripheral + GATT central + actif en arrière-plan.

| Option | Peripheral + Central | Arrière-plan | Verdict |
| --- | --- | --- | --- |
| **Android natif (Kotlin)** | ✅ `BluetoothGattServer` + `BluetoothLeScanner` + `BluetoothLeAdvertiser` | ✅ foreground service (restrictions gérables) | ✅ **Retenu — MVP** (~70 % du parc mondial) |
| iOS natif (Swift) | ⚠️ central OK ; peripheral **fortement bridé** en fond (*overflow area*) | ⚠️ throttlé | 🟡 **v2**, même cœur Rust |
| Flutter / React Native | ⚠️ dépend de plugins tiers ; peripheral partiel voire absent ; l'*advertiser* RN n'est plus maintenu | ⚠️ pire qu'en natif | ❌ Rejeté — un framework d'UI multiplateforme ne mutualise pas la couche BLE (fiche A-1) |
| Desktop / CLI (Rust + `btleplug`) | ✅ (Linux/BlueZ, macOS, Windows) | ✅ (process/daemon) | ✅ **Retenu comme `dengon-node`** |
| Web (Web Bluetooth) | ❌ central only, pas d'advertising, rien en fond | ❌ | ❌ Rejeté |

### 2.3 Modèle de sécurité des messages

| Option | Verdict |
| --- | --- |
| Signatures seules (Ed25519), contenu en clair | ❌ insuffisant |
| **Noise `XX` (session) + Noise `X` (scellé offline) + Ed25519 (paquet)** | ✅ **Retenu** (FS sur session live ; borné par `MSG_TTL_S` sur le scellé) |
| Signal : X3DH + Double Ratchet | 🟡 v2 (état de ratchet lourd en multi-hop) |
| `libsodium crypto_box` + mbedTLS (deux libs) | ❌ deux implémentations = risque de dérive (leçon Bridgefy) ; incompatible avec le cœur unique |
| Registre chiffré répliqué / « vraie » blockchain | ❌ (liveness, coût) |

### 2.4 Firmware du relais ESP32

| Option | Verdict |
| --- | --- |
| **ESP-IDF (C) + host NimBLE** | ✅ **Retenu (MVP)** (production, doc riche, coexistence Wi-Fi STA + BLE, ~40 Ko RAM de moins que Bluedroid) |
| `esp-rs` (`esp-hal` + `esp-wifi` + `bleps`/`trouble`) | 🟡 **cible d'évolution** (BLE encore jeune) |
| Arduino core + `NimBLE-Arduino` | 🟡 utile pour prototyper vite, moins bonne réutilisation du core |

Carte : **ESP32-WROOM-32E** ✅ retenu (existant, quotas réduits) ; WROVER + PSRAM
seulement si un test de charge sature les WROOM.

### 2.5 Stack du dashboard

| Option | Verdict |
| --- | --- |
| **FastAPI (Python) + SSE + SQLite + `dengon-verify`** | ✅ **Retenu** — léger, adapté à une démo de 5-8 appareils, base effacée par session |
| MQTT/Mosquitto → Axum (Rust) → PostgreSQL 16 + TimescaleDB → React, Docker Compose | ❌ sur-dimensionné pour le MVP (5 conteneurs, équipe débutante, ~3 semaines) |
| ThingsBoard / Node-RED + MQTT | ❌ « boîte noire » mal adaptée à la vérif de journal chaîné et à la reconstruction de parcours |

### 2.6 Benchmark couche par couche (`oswin/09` — recherche)

> Recherche `oswin` ; a informé les décisions. Les choix `oswin` (colonne
> « recommandé ») **ne sont pas** ceux du projet quand ils divergent (voir
> §1) — conservés ici comme matière de rapport.

- **App mobile** : Flutter recommandé par `oswin` (1 langage, build APK rapide) ;
  Kotlin natif si contrôle BLE maximal. → **Projet : Kotlin natif** (A-1).
- **Bibliothèque BLE** : `flutter_blue_plus` (central), `bluetooth_low_energy`
  (central + périphérique, moins éprouvé). → **Projet : API Kotlin native**
  (`BluetoothGattServer` / `BluetoothLeScanner` / `BluetoothLeAdvertiser`).
- **Firmware ESP32** : Arduino + PlatformIO recommandé par `oswin` ; ESP-IDF plus
  raide. → **Projet : ESP-IDF + NimBLE**.
- **Cryptographie** : mêmes primitives des deux côtés (X25519, ChaCha20-Poly1305,
  Ed25519), bibliothèque éprouvée. `oswin` : `sodium_libs` (app) + libsodium /
  rweather (ESP32). → **Projet : Noise + crates Rust, une implémentation**.
- **Broker MQTT** : Mosquitto local / HiveMQ Cloud. → **Projet : pas de MQTT**
  (HTTPS POST, A-6).
- **Dashboard** : Node-RED (pédagogique) / ThingsBoard (vitrine). → **Projet :
  FastAPI + SSE + SQLite**.
- **Persistance locale** : app = Hive/Isar ou SQLite ; ESP32 = NVS puis
  LittleFS. → **Projet : SQLite (app) / NVS + littlefs (ESP32)**.

**Points de vigilance transverses (valables)** : fixer le format de message
avant tout code ; tester le rôle périphérique BLE des téléphones dès la semaine
1 (Spike C) ; ne jamais coder sa propre crypto ni bricoler les nonces ; sécuriser
le transport (auth + TLS) ; versionner les choix (noter les versions exactes des
libs).

### 2.7 Étude de stack (`olivier/etude-stack` — recherche)

> Recherche `olivier` ; a informé A-1, A-2, A-5, A-13. Conservée comme matière.

- **Le point dur = le BLE de l'app mobile** : un téléphone doit être *chercheur*
  (scanner) **et** *visible/serveur* (annoncer + accepter des connexions) en
  même temps.

  | Option | Chercheur | Visible/serveur | Arrière-plan | Maintenue |
  | --- | :---: | :---: | :---: | :---: |
  | Flutter · flutter_blue_plus | ✅ | ❌ | partiel | ✅ très active |
  | Flutter · bluetooth_low_energy | ✅ | ✅ | oui, avec config | ✅ (moins éprouvée) |
  | RN · react-native-ble-plx | ✅ | ❌ | partiel | ✅ |
  | RN · react-native-ble-advertiser | ❌ | annonce seule | — | ❌ ~4 ans sans MAJ |
  | Natif Android (API Kotlin / lib Nordic) | ✅ | ✅ | contrôle total du service de fond | ✅ |

- **Contraintes d'arrière-plan Android** : **Android 14** — service de fond BLE
  doit déclarer `foregroundServiceType="connectedDevice"` + la permission
  `FOREGROUND_SERVICE_CONNECTED_DEVICE` ; **Android 15** — restrictions
  supplémentaires sur le scan BLE en fond, Doze profond différant les scans.
  → notification permanente incontournable ; **mode éco** + **relais ESP32
  fixes** prennent tout leur sens ; pour la démo, garder les écrans allumés.
- **Pourquoi l'iPhone est reporté** : en arrière-plan, un iPhone en rôle
  « visible » place ses identifiants de service dans une *overflow area*
  découvrable **uniquement par un autre appareil Apple** scannant cet
  identifiant précis. Comportement **système, non contournable** → **iOS v2**
  (A-11).
- **Firmware ESP32** : **NimBLE** (~40–100 Ko de moins que Bluedroid, ~50 % de
  flash en moins, BLE seul) → retenu. BLE Mesh natif (ESP-BLE-MESH, pile Zephyr)
  = modèle publish/subscribe, **pas** le modèle dengon → partir de **NimBLE
  brut**.
- **Un moteur ou deux ?** `olivier` recommandait deux implémentations bornées
  par une spec de trame stricte (simple, pas d'outillage, risque de dérive
  assumé). → **Projet : un seul cœur Rust** (A-2), l'outillage (UniFFI,
  cross-compile xtensa, `cbindgen`) est assumé comme investissement de semaine 1.
- **Inspiration Meshtastic « managed flood routing »** : limite de sauts = 7 ;
  un nœud ne rediffuse que si TTL > 0 **et** qu'il n'a pas déjà entendu ce
  paquet ; **« écouter avant de rediffuser »** (attente courte, si un voisin
  rediffuse déjà, s'abstenir ; les nœuds lointains rediffusent en premier). →
  intégré au pipeline de routage (voir [`05-protocole-et-trame.md`](05-protocole-et-trame.md) §6).
- **Serveur dashboard `olivier`** : petite API HTTP + base + canal temps réel +
  page statique ; FastAPI ou Express ; SQLite ; SSE. → **Projet : FastAPI**
  (C-8).

---

## 3. Périmètre MVP & feuille de route

### 3.1 Definition of Done du MVP

Le MVP est atteint quand, sur du **vrai matériel** :

1. Deux téléphones Android à portée échangent un message texte chiffré, avec
   passage des statuts **en attente → parti → distribué**.
2. Un 3ᵉ appareil hors de portée est joint via **au moins un relais ESP32**.
3. Un destinataire **éteint** reçoit son message à son retour (enveloppe
   scellée) ; l'expéditeur, **déconnecté** entre-temps, voit ses statuts se
   mettre à jour à sa reconnexion.
4. Le **dashboard FastAPI** (déployé sur le VPS) montre le parcours du message,
   la carte du réseau, l'état des relais, et **détecte une altération** de
   journal simulée (via `dengon-verify`).
5. Contact établi par **QR + code de vérification 60 chiffres** ; changement de
   clé détecté.
6. Sécurité : E2E vérifié (un relais capturé ne révèle aucun clair), `cargo
   audit` propre, revue crypto tierce passée.
7. CI verte : tests `dengon-core`, simulateur multi-nœuds, build des cibles
   (core, node, android, firmware), tests dashboard.

*(Le statut « Lu » et l'app iOS ne sont **pas** dans le DoD — A-10, A-11.)*

### 3.2 Lots de livraison

| Lot | Contenu | Jalon |
| --- | --- | --- |
| **Lot 0 — Fondations & spikes** | Workspace Rust, CI de base, conventions. **Spike A** : `dengon-core` (`protocol` + `crypto`) cross-compile-t-il pour `xtensa-esp32-none-elf` ? → tranche le curseur Rust/C de la crypto (A-2/A-3/B-1). **Spike B** : `btleplug` en rôle **peripheral** sur Linux → valider le double rôle GATT. **Spike C** : Android `BluetoothGattServer` + `BluetoothLeAdvertiser` + scan en foreground service, 2 appareils échangent 20 o (= « hello mesh ») → go/no-go de l'app Android native (A-1) + mesure du MTU réel (C-1). | Rapport de décision + squelette des crates |
| **Lot 1 — `dengon-core` : protocole & crypto** | `protocol` (encode/decode paquets, fragmentation L2, round-trip + property tests) ; `crypto` (Ed25519, Noise `XX`, Noise `X`, `recipient_tag`, padding, XChaCha20 champ par champ) ; `identity` (keypair, QR, code de vérification) ; `ledger` (append + `verify_chain` + export). | Vecteurs Noise, forge de signature rejetée, chaîne rompue détectée |
| **Lot 2 — `dengon-core` : store & sync** | `store` (impl SQLite) ; `sync::routing` (pipeline de routage) ; `sync::inventory` (échange d'inventaire, push du manquant) ; `sync::status` (machine à états MVP, outbox, rejeu) ; `sync::courier` (dépôt/collecte d'enveloppes) ; `observability` (catalogue + JSON canonique). | Tests unitaires par module |
| **Lot 3 — `dengon-sim` + `dengon-node` + `dengon-verify`** | `dengon-sim` (N instances de core reliées par transport en mémoire scriptable) ; scénarios rejoués ; `dengon-node` (CLI, transport `btleplug`) ; `dengon-verify` (binaire de vérif de journal). | Les 5 scénarios du DoD passent en simulation |
| **Lot 4 — `dengon-ffi` + app Android** | `dengon-ffi` (surface UniFFI, bindings Kotlin) ; `AndroidTransport` (Kotlin + JNI : GATT server + advertiser + scanner + foreground service) ; UI Compose (conversations, fil, saisie, statuts, écran QR + vérification, écran « réseau »). | 2 téléphones + 1 `dengon-node` → scénarios 1 et partiellement 2 |
| **Lot 5 — Firmware `dengon-relay`** | ESP-IDF + NimBLE, `libdengon_core.a` liée (selon Spike A) ; tâches route/inventory/courier/ledger/ship ; client HTTPS + buffer littlefs ; provisioning. | Scénarios 2 et 3 du DoD sur vrai matériel (WROOM) |
| **Lot 6 — Dashboard** | `api` FastAPI : ingest HTTPS + appel `dengon-verify` + REST + SSE ; migration SQLite ; `web` (page légère) ; déploiement sur le VPS derrière un reverse-proxy TLS ; script de purge par session. | Scénario 4 du DoD |
| **Lot 7 — Durcissement produit** | Quotas anti-DoS affinés ; revue crypto tierce ; `cargo deny`, SBOM ; OTA firmware signé, Secure Boot sur cartes de prod ; doc d'exploitation, runbook d'incident ; tests de charge (densité), tests de terrain. | — |

**Séquencement indicatif** : Lot 0 → Lot 1 → Lot 2 ; Lot 3 après Lot 2 ; Lot 4
et Lot 5 après Lot 3 ; Lot 6 après Lot 2 ; Lot 7 après Lot 5. Lots 5 et 6
parallélisables.

### 3.3 Explicitement hors MVP

| Fonction | Pourquoi reporté | Prévu par l'archi ? |
| --- | --- | --- |
| Statut **« Lu »** | réduit le périmètre v1 ; `ReadRcpt` / `status=lu` réservés | oui (A-10) |
| App **iOS** | BLE périphérique/fond bridé (*overflow area*), effort natif | oui (cœur Rust + UniFFI/Swift + `trait Transport`) (A-11) |
| **Gossip GCS** (`GOSSIP_FILTER/PULL/PUSH`) | l'échange d'inventaire brut suffit au MVP ; GCS = optimisation de bande passante | oui (types de paquets réservés) (A-13) |
| **Budget de copies / Spray-and-Wait** des enveloppes | flood borné + dédup suffit au MVP | oui (A-13) |
| **Groupes / canaux publics** | complexité (MLS ou signatures de canal), pas dans le brief | partiellement |
| **Pièces jointes** | débit BLE, fragmentation lourde | oui (fragmentation L2 déjà là) |
| **Double Ratchet** (Signal) | Noise suffit au MVP | oui (couche `crypto` isolée) |
| Firmware relais **en Rust pur** (`esp-rs`) | écosystème BLE encore jeune | oui (même `dengon-core`) |
| **Backhaul par le VPS** | choix explicite « observabilité seule » | non |
| Multi-appareil pour un même compte | synchro d'état complexe | non (v2) |

### 3.4 Risques & mitigations

| Risque | Impact | Mitigation |
| --- | --- | --- |
| `dengon-core` (crypto) ne cross-compile pas pour ESP32 | firmware plus coûteux | Spike A ; `trait Crypto` + mbedTLS pour le seul handshake de lien ; `sha2` + `ed25519` restent Rust (B-1) |
| BLE périphérique instable selon OEM Android | maillage dégradé sur certains téléphones | matrice d'appareils testés ; `dengon-node` comme relais mobile d'appoint ; relais ESP32 densifient |
| Coexistence BLE+Wi-Fi sur WROOM = débit faible | remontée de logs lente | fenêtres Wi-Fi ; batch HTTPS ; logs best-effort par nature ; WROVER en secours si saturation |
| Faux relais / pollution d'enveloppes | DoS stockage | signature obligatoire des `SEALED_ENVELOPE` ; quotas ; anti-inondation |
| Analyse de trafic par corrélation | fuite de métadonnées | padding, tags tournants ; limite documentée (adversaire global hors périmètre) |
| Dérive d'horloge des nœuds | statuts/latences faux | NTP relais ; fenêtre de tolérance ; alerte `clock_skew` |
| Périmètre qui gonfle (groupes, fichiers…) | MVP jamais fini | §3.2 fait foi ; tout ajout = après Lot 7 |

### 3.5 Feuille de route « débutants » par paliers (`oswin/08` — pour mémoire)

Approche progressive proposée par `oswin`, compatible avec les lots ci-dessus :

- **Palier 1** — le lien de base : téléphone central ↔ 1 ESP32 périphérique
  (envoi chiffré + affichage + dashboard).
- **Palier 2** — le relais et le stockage : 2ᵉ ESP32, store-carry-forward,
  multi-sauts, ACK, purge. **= MVP visé**.
- **Palier 3** — le mesh téléphone-à-téléphone (rôle périphérique BLE sur
  Android), à tenter une fois les paliers 1–2 solides.

Règle d'or : on ne commence un palier que lorsque le précédent est démontrable.
6 chantiers : (1) protocole & format ; (2) firmware ESP32 ; (3) app Android ;
(4) sécurité / crypto (transversal) ; (5) dashboard ; (6) intégration, tests &
doc.

### 3.6 Cadrage & planning (`olivier/decisions-v1` — pour mémoire)

> ⚠️ **Soutenance le 29/09/2026** — ~3 semaines à partir du 08/09/2026.

Cadrage v1 : objectif = **étude de conception + démo fonctionnelle** à parts
égales, périmètre resserré ; **Android + ESP32** (iPhone reporté) ; **1-à-1
uniquement** ; dashboard **inclus** mais minimal ; **protocole maison** ;
**chiffrement dès la v1** ; ajout de contact **en présentiel** (scan QR) ;
**« Lu » reporté en v2**.

Planning : conception ~1 semaine (figer la spec, priorité = circulation des
messages) ; démo ~1,5 semaine (démo réduite + dashboard minimal) ; rendu
~0,5 semaine (docs, rapport, slides, répétition).

**Ordre de repli si le délai dérape** : (1) hors-ligne + relais (le cœur) ;
(2) sécurité (relais aveugle) ; (3) dashboard. Le relais ESP32 reste dans le
MVP (C-9) et **n'est pas** ce qu'on sacrifie en premier.

Livrables attendus : spec protocole, doc sécurité, doc architecture, **+ rapport
écrit + support de présentation**.

### 3.7 Chantier « semaine 1 » proposé

| Tâche | Pourquoi maintenant | Proposition |
| --- | --- | --- |
| **Prototype « hello mesh »** (= Spike C) : 2 téléphones + 1 relais, 1 message qui passe | go / no-go de la techno mobile | Paul ou Tanguy |
| **Delta de doc sécurité** (sur la base de `powl/04-security.md`) | référencé par le protocole ; bloque la crypto | Paul ou Tanguy |
| **Monter le workspace `crates/`** + migration des docs | fin de la phase conception | Olivier |
| **Attribution des composants** (mobile / ESP32 / dashboard) | conditionne la phase démo | Réunion |
| **Plan de la démo** + scénario campus concret | cadre le développement | Réunion, puis Olivier rédige |

---

## 4. Stratégie de test & CI

> Source : `powl/11-testing-strategy.md`, complété par `oswin/08 §9`.

### 4.1 Pyramide

E2E terrain (manuel) — scénarios DoD sur vrai matériel ; Intégration —
`dengon-sim` (multi-nœuds), dashboard e2e, banc ESP32 ; Composant — chaque module
de `dengon-core`, `api` dashboard ; Unitaire + property — `protocol`, `crypto`,
`ledger`, status FSM. Principe : **le maximum de logique est dans `dengon-core`**,
testable sans radio ni réseau ; BLE et HTTP aux frontières, mockés.

### 4.2 `dengon-core` — tests unitaires & property

| Module | Tests clés |
| --- | --- |
| `protocol` | round-trip encode/decode (tout type) ; property `decode(encode(p)) == p` ; fragmentation/réassemblage avec MTU aléatoire ; rejet des paquets malformés / version inconnue |
| `crypto` | vecteurs Noise `XX`/`X` ; sign/verify Ed25519 ; forge rejetée (bit-flip → verify échoue) ; `recipient_tag` stable sur la journée, différent le lendemain ; padding → taille ∈ `PAD_BUCKETS` ; déchiffrement d'enveloppe avec mauvaise clé échoue proprement ; XChaCha20 champ par champ round-trip |
| `identity` | QR encode/decode ; code de vérification identique des deux côtés et ordre-indépendant ; sensible à un bit de clé |
| `ledger` | `verify_chain` OK sur chaîne valide ; détecte entrée modifiée, `seq` manquante, mauvaise signature ; `export(range)` cohérent ; reprise après « redémarrage » |
| `sync::routing` | dedup (même `msgID` 2× → 1 relais) ; TTL décrémenté, 0 → drop ; clamp densité ; `RELAY_OK=0` → pas de relais ; abandon si doublon pendant le jitter ; anti-inondation `FLOOD_MAX_PER_MIN_PEER` |
| `sync::inventory` | réconciliation A/B convergente ; push repasse par le pipeline (re-dedup) |
| `sync::status` | FSM exhaustive : toutes transitions valides, toutes invalides rejetées ; monotonie ; ACK en double idempotent |
| `sync::courier` | expiration ; collecte sur match de tag ; pas de fuite si tag ne matche pas |
| `observability` | chaque événement round-trip JSON canonique ; redaction : aucun `msg_uuid`/texte/recipient en clair ne peut être sérialisé (test négatif) ; `msg_log_id` bien haché |
| `store` (SQLite) | migrations ; idempotence `messages.insert(msg_uuid)` ; requêtes outbox ; chiffrement champ par champ actif |

Outils : `cargo test`, `proptest`, `cargo-nextest` en CI, `cargo llvm-cov`
(objectif ≥ 85 % sur `dengon-core`).

### 4.3 `dengon-sim` — intégration multi-nœuds

N instances de `dengon-core` reliées par un `Transport` **en mémoire** avec
modèle réseau scriptable (latence, perte, bande passante, **partition**,
**churn**, horloges désynchronisées). Scénarios versionnés
(`sim/scenarios/*.ron`) :

| Scénario | Vérifie |
| --- | --- |
| `direct.ron` | A→B connectés : livré, `delivered` |
| `multihop.ron` | A→C via 2 relais : livré, chemin attendu, TTL cohérent |
| `recipient_offline.ron` | C absent → enveloppe déposée → C revient → livré + ACK remonte |
| `sender_offline.ron` | A envoie puis part → à son retour, statuts rattrapés |
| `partition_merge.ron` | réseau coupé en 2, messages des deux côtés, fusion → convergence complète |
| `flood.ron` | 500 msg/s injectés → pas d'explosion mémoire, dedup efficace, TTL borne la charge |
| `dup_paths.ron` | même message par 3 chemins → 1 seul affiché, 1 seul ACK |
| `tamper.ron` | un nœud altère son journal → `verify_chain` échoue, dashboard alerte |
| `key_change.ron` | clé de B change → messages suspendus, alerte MITM |

Déterminisme : seed RNG fixe → rejouable. Exécuté à chaque PR.

### 4.4 Dashboard — tests

Unitaire (`api`, pytest) : parsing/validation de batch ; vérif signature
Ed25519 ; reconstruction de statut depuis une séquence d'événements ; appel de
`dengon-verify` (fixtures de chaînes valides / cassées / forkées) ; redaction
refusée. Intégration : base SQLite éphémère, POST des batchs réels → vérifier
projections, idempotence. API : golden tests sur les réponses REST ; SSE : un
event ingéré → push reçu. Front : tests de composants ; e2e Playwright sur les
écrans avec base seedée.

### 4.5 Firmware ESP32 — tests

Unitaire host (`libdengon_core` compilée pour l'hôte → mêmes tests que §4.2 pour
les modules embarqués) ; unitaire cible (Unity : `Store` NVS/littlefs, buffer
ring de logs wrap-around, curseur de journal persistant) ; intégration banc (2-3
ESP32 + 1 téléphone) ; résilience (coupure Wi-Fi 10 min ; `esp_restart()` →
journal reprend sans rupture de chaîne ; saturer SRAM → refus d'enveloppes mais
routage OK) ; conformité (vecteurs de paquets partagés avec `dengon-core` :
mêmes bytes in → mêmes décisions out).

### 4.6 E2E terrain — checklist de recette

Recette exécutée avant chaque jalon « produit » : (1) **Contact** (QR, comparer
le code 60 chiffres, marquer « vérifié ») ; (2) **Direct** (A→B à 5 m, statuts
observés, latence notée) ; (3) **Multi-saut** (B s'éloigne à 40 m avec un relais
ESP32 au milieu → livré) ; (4) **Destinataire absent** (Charlie éteint son BLE ;
A→Charlie ; après 5 min, Charlie rallume près d'un relais → reçoit ; ACK revient
à A) ; (5) **Expéditeur absent** (A envoie puis coupe le BLE 5 min ; A rallume →
statut « distribué ») ; (6) **Dashboard** (parcours de chaque message, carte
réseau, état des relais) ; (7) **Intégrité** (modifier une entrée de journal
d'un relais de test → dashboard lève `chain_broken` sous X minutes) ;
(8) **Sécurité** (capturer le trafic BLE avec nRF Sniffer → aucun clair ; VPS
sans `msgID` en clair ni contenu) ; (9) **Densité** (8-10 appareils dans une
salle → tous se voient, messages croisés livrés, pas d'effondrement). Résultats
consignés dans un rapport de recette daté.

### 4.7 CI

| Job | Déclencheur | Contenu |
| --- | --- | --- |
| `core` | PR, push | `cargo fmt --check`, `clippy -D warnings`, `nextest`, `llvm-cov`, `proptest` |
| `sim` | PR, push | tous les scénarios `dengon-sim` (seed fixe) |
| `audit` | PR, quotidien | `cargo audit`, `cargo deny check`, SBOM |
| `android` | PR touchant `android/` ou `dengon-ffi/` | build UniFFI, `./gradlew assembleDebug testDebugUnitTest` |
| `firmware` | PR touchant `firmware/` | `idf.py build`, tests host de `libdengon_core`, tests Unity |
| `dashboard` | PR touchant `dashboard/` | `pytest` (api, base SQLite éphémère), lint + build de la page, Playwright, build de `dengon-verify` |
| `cross-vectors` | PR, push | vecteurs de conformité partagés core ↔ firmware ↔ dashboard |
| `release` | tag | build artefacts (node binaries, APK, firmware .bin), signe, publie |

Blocage de merge : `core`, `sim`, `audit`, `cross-vectors` verts obligatoires.

### 4.8 Revue de sécurité

Avant le premier déploiement « produit » (fin Lot 7) : revue interne du modèle
de menace ; **audit crypto externe** de `dengon-core::crypto` + intégration
Noise ; test d'intrusion du dashboard (VPS) ; revue de la surface FFI (UniFFI)
et du firmware (gestion mémoire C).

### 4.9 Plan de test par brique (`oswin/08 §9`)

Crypto : chiffrer puis déchiffrer → identique ; signature vérifiée ; message
modifié → rejeté. Stockage : un message expiré est supprimé ; un ACK purge la
bonne entrée. Anti-doublon : un même message reçu 2× n'est relayé qu'une fois.
Passerelle : un événement ESP32 apparaît bien sur le dashboard.
