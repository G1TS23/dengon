# dengon — Architecture cible

> Fait partie du **contexte global** de `docs/synthese/` — point d'entrée :
> [`00-contexte-global.md`](00-contexte-global.md). Questions encore ouvertes :
> [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).
> Ce document décrit l'**architecture retenue** : cœur Rust unique `dengon-core`
> (A-2), app Android Kotlin natif via UniFFI (A-1), iOS possible en v2 sans
> retoucher le cœur.

---

## 1. Décomposition en briques

1. **Format de message** (« paquet ») : en-tête clair (ID, TTL, expéditeur
   pseudonyme, destinataire, horodatage) + charge utile **chiffrée**.
2. **Transport BLE** : découverte des voisins, envoi/réception.
3. **Logique de relais** : anti-boucle, décrément du TTL, retransmission aux
   nouveaux voisins.
4. **File de stockage** avec expiration (TTL temporel) et purge sur ACK.
5. **Couche cryptographique** : échange de clés, chiffrement authentifié,
   signature.
6. **Accusé de réception (ACK)** qui remonte le mesh pour libérer le stockage.
7. **Passerelle Arduino** + **dashboard**.
8. **Chaînage par hash / Merkle** pour l'intégrité et les preuves (journal
   chaîné signé — retenu, voir [`06-securite.md`](06-securite.md)).

## 2. Vue composants — `dengon-core`

`dengon-core` (Rust) contient **tout sauf l'I/O radio et l'I/O réseau IP**. Il
produit/consomme des `Vec<u8>` transportés par un `Transport` ; le *log shipper*
(HTTPS POST, relais uniquement) est un consommateur externe du flux
d'événements. C'est la **seule implémentation du protocole** (A-2), partagée par
l'app Android, le nœud CLI et le firmware ESP32.

| Module | Responsabilité | Dépendances clés |
| --- | --- | --- |
| `protocol` | sérialisation binaire des paquets (voir [`05-protocole-et-trame.md`](05-protocole-et-trame.md)), types, flags, fragmentation/réassemblage | — (`no_std` compatible) |
| `crypto` | Ed25519 (sign/verify), Noise `XX` & `X` (`snow`), scellage d'enveloppes, `recipient_tag`, padding | `ed25519-dalek`, `snow`, `x25519-dalek`, `chacha20poly1305`, `hmac`, `sha2` |
| `identity` | génération/chargement de l'identité, encodage QR, dérivation du code de vérification 60 chiffres | `crypto`, `qrcode` (côté app) |
| `store` | persistance : messages, conversations, contacts, outbox, enveloppes, seen-set, journal ; chiffrement **XChaCha20-Poly1305 champ par champ** des colonnes sensibles (B-3) | `rusqlite` (natif) / abstraction pour ESP32 |
| `sync` | routage (flood + TTL + jitter + clamp densité + quotas), réconciliation par **échange d'inventaire** (GCS = cible v2), rejeu d'outbox, **machine à états des statuts**, collecte d'enveloppes | `protocol`, `store`, `crypto` |
| `ledger` | journal append-only chaîné : `append(event) -> Entry`, `verify_chain()`, `export(range)` | `crypto`, `sha2` |
| `observability` | catalogue d'événements typés, JSON canonique, redaction (hachage `msgID`) | `serde`, `ledger` |
| `api` (façade) | surface publique UI + UniFFI : `send_message`, `poll_events`, `on_peer_connected(transport)`, `mark_read`, … | tous les autres |

**Frontières `no_std`** : pour l'ESP32, `protocol`, `sync` (routage/dedup/
inventaire), `ledger` doivent compiler en `no_std` + `alloc`. `store` est
derrière un trait `Store` (impl `rusqlite` natif, impl NVS/flash sur ESP32).
`crypto` : **tout en Rust si le Spike A le permet ; sinon `trait Crypto` +
mbedTLS limité au handshake Noise `XX` de lien BLE, `sha2` + `ed25519-dalek`
restant en Rust partout** (B-1).

Le binaire **`dengon-verify`** (crate à part) réutilise `ledger::verify_chain`
et les structs `observability` ; il est appelé en sous-processus par le
dashboard Python pour vérifier les journaux chaînés (voir A-5 et
[`09-dashboard-et-donnees.md`](09-dashboard-et-donnees.md)).

## 3. Le trait `Transport`

```rust
/// Abstraction d'un lien BLE. Implémentée nativement par plateforme.
pub trait Transport: Send {
    /// Démarre l'annonce du service `dengon` + le scan des pairs.
    fn start(&mut self, cfg: TransportConfig) -> Result<()>;

    /// Événements remontés vers le core (thread-safe, non bloquant).
    ///  - PeerConnected { peer_link_id, rssi }
    ///  - PeerDisconnected { peer_link_id }
    ///  - FrameReceived { peer_link_id, bytes }   // 1 trame BLE (déjà réassemblée L2)
    fn poll(&mut self) -> Vec<TransportEvent>;

    /// Envoie une trame applicative à un pair connecté.
    /// La fragmentation BLE (MTU) est gérée par l'impl ; la fragmentation
    /// protocole (paquets > MTU) est gérée par `protocol`.
    fn send(&mut self, peer_link_id: LinkId, bytes: &[u8]) -> Result<()>;

    /// Diffusion best-effort à tous les pairs connectés (pour ANNOUNCE, gossip).
    fn broadcast(&mut self, bytes: &[u8]) -> Result<()>;
}
```

| Impl | Fichier | Techno |
| --- | --- | --- |
| Android (MVP) | `android/.../ble/AndroidTransport.kt` + pont JNI dans `dengon-ffi` | `BluetoothGattServer`, `BluetoothLeScanner`, `BluetoothLeAdvertiser` + foreground service |
| iOS (v2) | `ios/.../CoreBluetoothTransport.swift` | CoreBluetooth (bindings Swift générés par UniFFI depuis le **même** `dengon-core`) |
| Desktop / CLI | `crates/dengon-ble/src/btleplug_transport.rs` | `btleplug` (BlueZ / CoreBluetooth / WinRT) |
| ESP32 | `firmware/dengon-relay/src/transport_nimble.c` (+ FFI vers la lib core) | NimBLE (ESP-IDF) |

C'est **l'unique couture** entre le cœur et les plateformes : seuls la radio
(`Transport`) et l'UI sont spécifiques ; le protocole, la crypto, le stockage et
le journal sont partagés. C'est aussi ce qui rend l'**ouverture iOS** peu
coûteuse (un shell SwiftUI + une impl `Transport` CoreBluetooth, sans toucher au
cœur — A-11).

**Rôle GATT** : chaque nœud est **serveur ET client**. Service `dengon` (UUID
fixe, voir C-2), 2 caractéristiques : `RX` (write-without-response, pair →
nœud), `TX` (notify, nœud → pair).

## 4. Découpage en couches — vue fonctionnelle

De la radio vers l'utilisateur :

1. **Application** : conversations, contacts, statuts, réglages (mode éco),
   notifications — *téléphone uniquement*.
2. **Sécurité** : clés, chiffrement/déchiffrement, signatures, vérification de
   contact — *téléphone déchiffre ; ESP32 ne déchiffre pas*.
3. **Moteur dengon (protocole)** : circulation, déduplication, TTL, file de
   retransmission, réassemblage, accusés — *commun téléphone + ESP32*
   (`dengon-core`).
4. **Transport BLE** : découverte des voisins, connexion, envoi/réception de
   trames — *commun via le trait `Transport`, impl différente par plateforme*.
5. **Stockage local** : messages, contacts, clés, file, table « déjà vu » —
   *téléphone : base chiffrée champ par champ ; ESP32 : NVS + PSRAM/flash*.

**Commun téléphone ↔ ESP32** : le **format de trame** et la **logique du moteur**
(objectif : même comportement des deux côtés, garanti par le cœur unique).
**ESP32** : pas d'UI, ne déchiffre pas, moteur allégé, forte contrainte mémoire
→ file plus petite, seen-set plus court. **Téléphone** : UI complète,
déchiffrement/affichage, stockage riche, notifications, service de fond avec
notification permanente.

**Chemin d'un message** : Alice (app) saisit → Sécurité chiffre pour Bob, signe
→ Moteur crée la trame (ID, `peerID`, TTL, horodatage), statut « En attente » →
Transport BLE pousse aux voisins → « Parti ». ESP32 relais : Transport BLE
reçoit → Moteur : pas destinataire, « déjà vu » ? non ; TTL 7→6 ; met en file ;
réémet. Bob (app) : reçoit → Moteur : destinataire → réassemble → Sécurité :
déchiffre, vérifie signature → Application : affiche, déclenche l'ACCUSÉ →
Moteur : crée la trame ACCUSÉ (chiffrée pour Alice) et la diffuse. ACCUSÉ :
remonte Bob → ESP32 (qui le voit passer → retire le message de sa file) → Alice
→ statut « Distribué ».

## 5. Découpage du dépôt

```text
dengon/
├── crates/
│   ├── dengon-core/          # lib Rust — LE protocole
│   │   ├── src/{protocol,crypto,identity,store,sync,ledger,observability,api}.rs
│   │   └── tests/            # tests unitaires + property tests
│   ├── dengon-ble/           # trait Transport + impl btleplug
│   ├── dengon-node/          # binaire CLI : nœud headless (tests, PC fixe, bootstrap)
│   │   └── src/main.rs       # `dengon-node run --name alice --db ./alice.db`
│   ├── dengon-sim/           # simulateur multi-nœuds (transport in-memory, partitions)
│   ├── dengon-verify/        # binaire : vérif de journal chaîné, appelé par le dashboard
│   └── dengon-ffi/           # bindings UniFFI (génère Kotlin ; Swift en v2)
├── android/                  # app Kotlin + Jetpack Compose
│   └── app/src/main/{java,kotlin}/…/{ble,ui,service}/
├── firmware/
│   └── dengon-relay/         # ESP-IDF (C) + NimBLE + client HTTPS + lib core statique
│       ├── main/
│       └── components/dengon_core_ffi/
├── dashboard/
│   ├── api/                  # FastAPI (Python) : ingest HTTPS + SSE + SQLite ; appelle dengon-verify
│   └── web/                  # page web légère (statique)
├── docs/                     # powl/ oswin/ olivier/ synthese/ suivi/
└── Cargo.toml                # workspace Rust (crates/*)
```

*(La structure `app/ firmware/ dashboard/serveur/ outils/` proposée par
`olivier/mise-en-commun` n'a pas été retenue — voir A-14.)*

## 6. Déploiement

Terrain (hors ligne) : `dengon-app` (Android), `dengon-node` (PC fixe),
`dengon-relay` (ESP32-WROOM-32E). VPS (Debian, déjà possédé) : app **FastAPI**
(HTTPS, SSE), base **SQLite**, page web statique, binaire `dengon-verify`. TLS
via un reverse-proxy léger (Caddy ou nginx) ou directement.

- Le VPS n'a **aucune** connexion sortante vers le terrain. Il **reçoit**
  seulement (endpoint HTTPS `POST /ingest/batch`).
- Perte du VPS = perte de l'observabilité, **zéro impact** sur la messagerie.
- Relais authentifiés par **jeton/JWT court** (liste blanche) + **signature
  Ed25519** des batchs d'événements (B-2). Pas de broker MQTT, pas de mTLS.

## 7. Choix transverses

| Sujet | Choix | Note |
| --- | --- | --- |
| Langage cœur | Rust (edition 2021, MSRV figée) | audit unique |
| Async | `tokio` côté `dengon-node` ; core = **sync + boucle d'événements** (portable ESP32) | le core n'impose pas de runtime |
| Sérialisation événements | JSON canonique (clés triées) pour la signature ; stockage tel quel | déterminisme de signature |
| Sérialisation paquets | binaire maison (`powl/03`, voir [`05-protocole-et-trame.md`](05-protocole-et-trame.md)) | compacité BLE |
| Base locale | SQLite (`rusqlite`, `WAL`) | colonnes sensibles chiffrées **XChaCha20-Poly1305 champ par champ** (B-3) — pas de SQLCipher (évite une dépendance native de plus) |
| ID de log | `msgID` haché (`SHA-256(msgID)[:16]`) avant tout envoi au VPS | anti-corrélation |
| Versionnement protocole | champ `version` dans chaque paquet + négociation à l'ANNOUNCE | montée de version progressive |
| Dashboard | Python / FastAPI + SSE + SQLite (A-5) ; vérif de journal via le binaire Rust `dengon-verify` | pas de backend Rust complet (B-5) |
