# Benchmarks & décisions technologiques

Ce document justifie chaque choix structurant. Format : options comparées → verdict →
raison. Les décisions sont ensuite figées dans les documents de conception dédiés.

---

## §1 — « Le réseau ressemble à une blockchain » : qu'est-ce que c'est vraiment ?

### 1.1 Ce que le réseau est réellement

Un réseau de messagerie BLE maillé tolérant aux déconnexions est un **DTN**
(*Delay-Tolerant Network*) avec **routage épidémique / gossip** :

- **store-carry-forward** : un nœud garde un message en mémoire, le transporte
  physiquement en se déplaçant, et le retransmet quand il croise un autre nœud ;
- **routage épidémique** : quand deux nœuds se rencontrent, ils s'échangent tous les
  messages qu'ils n'ont pas en commun — le message se propage « comme une épidémie » ;
- **flooding contrôlé** : pour éviter la saturation, on borne la propagation par un
  **TTL** (nombre de sauts), une **fenêtre de déduplication**, et un **budget de
  copies** pour les envois ciblés.

C'est un domaine de recherche ancien (Vahdat & Becker, *Epidemic Routing*, 2000 ;
PRoPHET ; Spray-and-Wait). Les messageries réelles qui l'appliquent : **Briar**
(store-and-forward sur liens point-à-point), **Bridgefy** (mesh BLE, cf. la faille
analysée dans *Breaking Bridgefy*, IACR 2021/214), **bitchat** (mesh BLE, gossip,
Noise).

### 1.2 En quoi ça ressemble à une blockchain

| Trait blockchain | Présent ici ? | Comment |
| --- | --- | --- |
| Pas de serveur central, identité = clé publique | ✅ | Curve25519 + Ed25519 ; `peerID = SHA-256(pubkey)[:8]` |
| Propagation **gossip** P2P | ✅ | c'est le cœur du routage |
| Messages **signés** | ✅ | signature Ed25519 sur chaque paquet |
| **Anti-rejeu / anti-doublon** par hash de contenu | ✅ | `msgID = hash(sender, ts, payload)` + seen-set |
| **Registre append-only chaîné par hash** (tamper-evident) | ✅ | journal local par appareil : `hash_n = H(entry_n ‖ prev_hash)` |
| Réconciliation d'état entre pairs | ✅ | échange de filtres compacts (Bloom/GCS), pull du manquant |

### 1.3 Ce qu'on n'emprunte PAS à la blockchain — et pourquoi

| Trait blockchain | Retenu ? | Raison du rejet |
| --- | --- | --- |
| **Consensus global** (PoW / PoS / BFT) | ❌ | Exige une *liveness* réseau (quorum joignable) **incompatible avec l'offline-first**. Coût CPU/énergie prohibitif sur mobile et ESP32. |
| **Chaîne globale unique / ordre total** | ❌ | Aucun besoin d'ordonner globalement des messages privés indépendants. Un ordre **causal par conversation** suffit (numéro de séquence + horloge). |
| **Résistance Sybil / preuve de travail à l'entrée** | ❌ | Le problème (double-dépense) n'existe pas : on ne transfère pas de valeur, juste des messages idempotents. |
| **Réplication totale du registre chez tous** | ❌ | Chaque nœud n'a besoin que de **ses** conversations + un cache court des messages publics. Répliquer tout = fuite de métadonnées + coût stockage. |
| **Smart contracts / VM** | ❌ | Hors sujet. |

### 1.4 Formulation retenue

> **DTN à routage gossip + micro-registre local chaîné et signé par appareil,
> agrégé et audité par le dashboard.**

C'est l'interprétation *honnête* de « sécurité type blockchain » : on prend
l'infalsifiabilité (hash-chain) et la décentralisation (clés, gossip), on laisse le
consensus (inutile et nuisible ici).

**Le « registre » sert à l'audit, pas au routage** : le réseau route très bien sans
lui. Le journal chaîné donne au dashboard une trace qu'un relais malveillant ne peut
pas réécrire *a posteriori* sans que ça se voie (rupture de chaîne / fork détecté).

Détail d'implémentation du journal : `04-security.md` §Journal chaîné.

### 1.5 Références

- bitchat — `WHITEPAPER.md`, `github.com/permissionlesstech/bitchat`
- Briar — `code.briarproject.org`, audit Cure53
- *Mesh Messaging in Large-scale Protests: Breaking Bridgefy* — IACR ePrint 2021/214
- *Survey of Mesh Networking Messengers* — TUM NET-2021-05-1
- Vahdat, Becker — *Epidemic Routing for Partially-Connected Ad Hoc Networks*, 2000
- *A Secure Epidemic Routing using Blockchain in Opportunistic IoT* — Springer, 2020
- Espressif — *ESP-BLE-MESH Architecture*, docs.espressif.com

---

## §2 — Plateforme de l'application cliente

### 2.1 Contrainte technique déterminante

Un vrai nœud mesh doit être **simultanément** :

- **GATT peripheral** (annoncer un service, accepter des connexions entrantes) ;
- **GATT central** (scanner, se connecter à d'autres) ;
- **actif en arrière-plan** (l'utilisateur ne garde pas l'app au premier plan).

C'est ce triptyque qui élimine la plupart des options.

### 2.2 Comparatif

| Option | Peripheral + Central | Arrière-plan | Écosystème | Verdict |
| --- | --- | --- | --- | --- |
| **Android natif (Kotlin)** | ✅ `BluetoothGattServer` + `BluetoothLeScanner` | ✅ *foreground service* + `BLE scan` (restrictions gérables) | Play Store, ~70 % du parc mondial | ✅ **Cible n°1 (MVP)** |
| iOS natif (Swift) | ⚠️ central OK ; peripheral **fortement bridé** en fond (pas d'`advertising` de service custom hors foreground, local name masqué) | ⚠️ `bluetooth-central`/`peripheral` background modes mais throttlé | App Store | 🟡 Cible n°2, co-conçue, **hors MVP** |
| Flutter / React Native | ⚠️ dépend de plugins tiers ; peripheral partiel voire absent selon plugin | ⚠️ pire qu'en natif | 1 codebase | ❌ Rejeté pour un **produit** mesh (on hériterait des bugs des plugins sur la partie la plus critique) |
| Desktop / CLI (Rust + `btleplug`) | ✅ (Linux/BlueZ, macOS, Windows) | ✅ (process/daemon) | — | ✅ **Retenu comme `dengon-node`** (tests, PC fixe, bootstrap) |
| Web (Web Bluetooth) | ❌ central only, pas d'advertising, rien en fond | ❌ | navigateur | ❌ Rejeté |

### 2.3 Décision

**Cœur partagé en Rust + liaison BLE native par plateforme.**

```text
dengon-core (Rust)  ── tout sauf l'I/O radio ──┐
                                               ├── Android : JNI + BluetoothGattServer/Scanner   → dengon-app (Kotlin/Compose) via UniFFI
trait Transport ───────────────────────────────┼── Desktop/CLI : btleplug                        → dengon-node (Rust natif)
                                               ├── ESP32 : NimBLE                                → dengon-relay (voir §4)
                                               └── (iOS : CoreBluetooth via UniFFI/Swift, post-MVP)
```

**Pourquoi** : le protocole, la crypto et la logique de synchro sont écrits **une
seule fois**, testés une seule fois, audités une seule fois. Seule la couche
« envoyer/recevoir des octets par BLE » est spécifique à chaque plateforme, et elle
est petite et bien isolée derrière un trait.

**MVP livré** : `dengon-core` + `dengon-node` + `dengon-app` (Android).
Détails : `02-architecture.md`.

---

## §3 — Modèle de sécurité des messages

### 3.1 Comparatif

| Option | Confidentialité E2E | Forward secrecy | Post-compromise | Offline/async | Complexité | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| Signatures seules (Ed25519), contenu en clair | ❌ | ❌ | ❌ | trivial | ~0 | ❌ insuffisant |
| **Noise `XX` (session) + Noise `X` (scellé offline) + Ed25519 (paquet)** | ✅ | ✅ sur session live ; ❌ sur scellé | ⚠️ partiel | ✅ | moyenne | ✅ **Retenu** |
| Signal : X3DH + Double Ratchet | ✅ | ✅✅ | ✅ | ✅ mais état de ratchet lourd à gérer en multi-hop / multi-device | élevée | 🟡 v2 (upgrade du canal privé) |
| MLS (RFC 9420) | ✅ | ✅✅ | ✅ | ✅ | élevée | 🟡 seulement si les **groupes** deviennent centraux |
| Registre chiffré répliqué / « vraie » blockchain | n/a | n/a | ❌ liveness | ❌ | très élevée | ❌ cf. §1.3 |

### 3.2 Décision : Noise Protocol Framework (approche éprouvée par bitchat)

- **Identité** : `Curve25519` (clé statique, accord de clés Noise) + `Ed25519`
  (signature des paquets). `peerID = SHA-256(pub_curve25519)[0..8]`, stable.
- **Canal en direct** (pairs connectés) : **Noise `XX`**
  `Noise_XX_25519_ChaChaPoly_SHA256` → authentification mutuelle + forward secrecy.
  Les accusés (distribué, lu) circulent **chiffrés dans cette session**.
- **Message pour destinataire absent** : **enveloppe scellée Noise `X`**
  (`Noise_X_25519_...`, one-shot, vers la clé statique publique du destinataire).
  Pas de forward secrecy sur ces enveloppes (compromis assumé, documenté).
  Adressage par **tag tournant** `recipient_tag = HMAC(pub_static_dest, jour_UTC)`
  (16 o) : les relais ne savent pas *qui* est le destinataire.
- **Padding** des paquets chiffrés vers 256 / 512 / 1024 / 2048 o.
- **Vérification d'identité (QR + code)** :
  - QR = `dengon:v1:<base64url(pseudo ‖ pub_static ‖ pub_sign)>`
  - **code de vérification** = 60 chiffres décimaux, groupés par 5, dérivés de
    `SHA-512(min(pubA,pubB) ‖ max(pubA,pubB))` (indépendant de l'ordre) — même
    principe que le *safety number* de Signal.
  - **TOFU** (*Trust On First Use*) au premier contact ; statut « ✔ vérifié » après
    comparaison réussie du code.
- **Couche audit « type blockchain »** : journal append-only chaîné signé par
  appareil (cf. §1.4 et `04-security.md`).

Le VPS ne reçoit **jamais** de clé privée ni de contenu déchiffrable.
Détails et modèle de menace : `04-security.md`.

---

## §4 — Firmware du relais ESP32

### 4.1 Comparatif

| Option | Réutilisation `dengon-core` | Maturité BLE | BLE + Wi-Fi simultanés | Écosystème | Verdict |
| --- | --- | --- | --- | --- | --- |
| **ESP-IDF (C) + host NimBLE** | via `esp-idf-sys` (lib statique) ou port de la spec | ✅ production, doc riche | ✅ Wi-Fi station + BLE (coexistence supportée) | officiel Espressif | ✅ **Retenu (MVP)** |
| `esp-rs` (`esp-hal` + `esp-wifi` + `bleps`/`trouble`) | ✅ maximale (même langage) | 🟡 BLE encore jeune, API mouvantes | 🟡 en progrès | communautaire | 🟡 **cible d'évolution** |
| Arduino core + `NimBLE-Arduino` | ❌ faible | ✅ correcte | ✅ | large mais « maker » | ❌ moins maîtrisable pour un produit (couches d'abstraction opaques) |

### 4.2 Décision

- **ESP-IDF + NimBLE** (host léger : ~40 Ko RAM de moins que Bluedroid, init plus
  rapide).
- **Réutiliser `dengon-core`** pour les parties `no_std`-compatibles : parsing de
  paquets, TTL, déduplication, logique gossip, journal chaîné → compilées en
  **bibliothèque statique** (`target xtensa-esp32-none-elf` ou lien via
  `esp-idf-sys`).
- **Crypto** : si `dengon-core` complet ne cross-compile pas proprement, isoler
  Noise/Ed25519 derrière un trait et utiliser une lib C éprouvée (`libsodium` port
  ESP-IDF, ou `mbedTLS` déjà présent) côté firmware. **Décision finale au spike**
  (`10-mvp-scope-roadmap.md`, lot 0).
- **Constantes de protocole** (types de paquets, TTL, tailles) dans **un seul
  fichier** de la spec, généré ou recopié à l'identique côté C.

### 4.3 Carte

| Carte | RAM utile | PSRAM | Verdict |
| --- | --- | --- | --- |
| ESP32-WROOM-32 | ~300 Ko SRAM | ❌ | 🟡 prototype only (buffers limités, débit BLE+Wi-Fi réduit) |
| **ESP32-WROVER(-E)** | + 4-8 Mo PSRAM | ✅ | ✅ **Retenu** : buffers de messages + enveloppes scellées + coexistence radio |

Détails : `06-relay-esp32.md`.

---

## §5 — Stack du dashboard (VPS)

### 5.1 Ingestion des logs

| Option | Pour | Contre | Verdict |
| --- | --- | --- | --- |
| **MQTT over TLS** (Mosquitto/EMQX) | conçu pour flotte d'objets, QoS, reconnexion, léger sur ESP32 (lib officielle) | broker à opérer | ✅ **Retenu** (canal principal relais → VPS) |
| HTTPS POST par batch | simple, pas de broker | pas de push, gestion reconnexion à la main | 🟡 **fallback** pour clients opt-in |
| gRPC streaming | efficace | lourd sur ESP32 | ❌ |

### 5.2 Backend

| Option | Pour | Contre | Verdict |
| --- | --- | --- | --- |
| **Rust + Axum** | réutilise les **types d'événements** et la **vérif de hash-chain** de `dengon-core` ; un seul langage serveur ↔ core | équipe doit être à l'aise en Rust | ✅ **Retenu** |
| Node.js + TypeScript (Fastify) | rapide à écrire, même langage que le front | réimplémenter la vérif de chaîne / parsing | 🟡 acceptable si contrainte d'équipe |

### 5.3 Stockage

**PostgreSQL 16 + TimescaleDB** : hypertable pour le flux d'événements (haute
cardinalité temporelle), tables relationnelles classiques pour nœuds / messages /
statuts / contacts vérifiés. Un seul moteur à opérer.

### 5.4 Frontend

**React + TypeScript + Vite**, WebSocket pour le temps réel (carte réseau,
statuts qui progressent en direct). Lib graphe : `d3-force` / `cytoscape` pour la
topologie.

### 5.5 Déploiement

**Docker Compose** sur le VPS : `mosquitto`, `api` (Axum), `db` (Timescale), `web`
(nginx servant le build), `caddy` (reverse-proxy + TLS Let's Encrypt automatique).
Grafana **optionnel** pour les métriques système du VPS lui-même.

Détails : `07-dashboard.md`.

---

## Récapitulatif des décisions

| # | Sujet | Décision |
| --- | --- | --- |
| 1 | Nature du réseau | DTN gossip + journal chaîné signé (pas de consensus) |
| 2 | App cliente | `dengon-core` Rust partagé ; app **Android/Kotlin** (MVP) ; `dengon-node` CLI Rust ; iOS post-MVP |
| 3 | Crypto | Noise `XX` (session) + Noise `X` (scellé) + Ed25519 ; QR + code 60 chiffres ; TOFU |
| 4 | Firmware relais | ESP-IDF + NimBLE ; ESP32-**WROVER** ; réutilisation `dengon-core` (lib statique) |
| 5 | Dashboard | MQTT/TLS → Axum (Rust) → Postgres/TimescaleDB → React/TS ; Docker Compose + Caddy |
