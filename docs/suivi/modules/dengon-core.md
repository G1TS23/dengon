# Module : `dengon-core` (`crates/dengon-core/`)

**Rôle en une phrase :** la bibliothèque qui contient **tout le protocole** dengon, sans aucune entrée/sortie.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §2 et §5 (décision A-2) ; [`docs/synthese/05-protocole-et-trame.md`](../../synthese/05-protocole-et-trame.md) (format de trame) ; [`docs/synthese/06-securite.md`](../../synthese/06-securite.md) (crypto, identité §2) ; [`docs/synthese/09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md) §11.3 (QR, code de vérification).
**Dernière mise à jour :** 2026-09-28
**État :** en cours — squelette (US-104) + `protocol::{consts, types}` (US-108) + `ledger` (US-206) + `store` (US-207) + `crypto` : Ed25519 (US-203) + Noise `XX`/`X`, `recipient_tag`, padding (US-204) + `protocol::codec` (US-201) + `identity` : clés, QR, code de vérification, coffre (US-205) + `sync::status` (US-211) + `sync::routing` (US-209).

## À quoi ça sert

C'est le cœur du projet, et la raison pour laquelle il n'y a **qu'une seule**
implémentation du protocole : la même bibliothèque est utilisée par
l'application Android (via `dengon-ffi`), par le nœud en ligne de commande
`dengon-node` et par le firmware de l'ESP32.

La règle qui rend ce partage possible : `dengon-core` ne parle **ni à la radio
ni au réseau**. Il produit et consomme des `Vec<u8>`, que quelqu'un d'autre se
charge de transporter (le trait `Transport` de `dengon-ble`).

## Structure

```
dengon-core/
  src/
    lib.rs              — bascule no_std, extern crate alloc, PROTOCOL_VERSION, VERSION, pub mod store (feature std)
    crypto.rs           — Ed25519 sign/verify (US-203), CryptoError, ré-exports
    crypto/
      noise.rs         — Noise XX (Handshake → Session) et Noise X (seal / open) (US-204)
      pad.rs           — padding vers PAD_BUCKETS : len(u16 BE) ‖ données ‖ zéros (US-204)
      tag.rs           — recipient_tag = HMAC-SHA256(pub_static, "dengon-tag" ‖ day)[0..16], epoch_day (US-204)
      rng.rs           — CallerResolver : aléa de l'appelant injecté dans snow (privé) (US-204)
    identity.rs         — IdentityError, ré-exports (US-205)
    identity/
      keys.rs          — Identity (secrets + pseudo), PublicIdentity (carte de contact),
                         peer_id, fingerprint, peer_id_base32 (US-205)
      qr.rs            — PublicIdentity::to_qr / from_qr : "dengon:v1:<base64url>" (US-205)
      safety.rs        — SafetyNumber, safety_number, verification_code : 60 chiffres (US-205)
      vault.rs         — seal / unseal (XChaCha20-Poly1305), trait Vault, MemoryVault,
                         FileVault (std), load_or_create (US-205)
    ledger.rs           — journal chaîné append-only (US-206)
    store.rs            — persistance SQLite, chiffrement champ par champ (US-207)
    protocol/
      mod.rs           — re-exports du module protocol
      consts.rs        — TOUTES les constantes du protocole (synthese/05 §2)
      types.rs         — PacketType, Flags, Header, AppFrameKind, AckStatus,
                         alias PeerId / MsgId / Signature
      codec/
        mod.rs         — Packet, encode / decode L3, FrameRule, entrée de signature
        app.rs         — AppFrame (Message, Ack), encode / decode L4
    sync/
      mod.rs           — table des sous-modules sync (routing, status livrés ;
                         inventory, courier = US-210/212)
      routing.rs       — routeur sans-IO : TTL, dédup, jitter, clamp densité,
                         quotas, anti-inondation (US-209)
      status.rs        — Status, StatusEvent, next_status, StatusChange (US-211)
      status/outbox.rs — Outbox, OutboxStore, MemoryStore, OutboxRecord
      status/outbox/tests.rs — tests de l'outbox
  tests/
    vectors_v0.json        — vecteurs de conformité v0 du format de trame (US-108)
    protocol_vectors.rs    — contrôle structurel + décodage réel de ces vecteurs
    codec_proptest.rs      — property tests du codec (round-trip, aucun panic)
    crypto_vectors.rs      — contrôle + régénération des vecteurs crypto (US-204)
    vectors/crypto_v0.json — vecteurs crypto (padding, tags, transcript XX, enveloppe X)
    identity_vectors.rs    — vecteurs identity + scénario d'appairage A↔B (US-205)
    vectors/identity_v0.json — 2 identités : clés publiques, peerID, empreinte, QR, code
    routing_mock.rs        — sync::routing de bout en bout contre MockTransport
                             (+ codec de test provisoire)
```

`crypto.rs` et le dossier `crypto/` coexistent (disposition Rust 2018) : le
fichier Ed25519 de US-203 n'a pas été déplacé (pas de déplacement
de fichier pendant que les PR #78/#81/#82 sont empilées).

Modules encore absents : `sync::{inventory, courier}`,
`observability`, `api` (sprint 2).

## Concepts / types importants

| Type / fonction | Fichier | Ce que ça fait |
|---|---|---|
| `PROTOCOL_VERSION: u8` | `src/lib.rs` | **Alias** de `protocol::consts::PROTO_VERSION` ; gardé parce que les crates sœurs l'utilisent comme test de liaison (US-104). |
| `VERSION: &str` | `src/lib.rs` | Version de la crate, lue dans `Cargo.toml` à la compilation. |
| `protocol::consts::*` | `src/protocol/consts.rs` | ~35 constantes : `PROTO_VERSION`, TTL (`TTL_DEFAULT=7`, clamp densité), fragmentation, `MSG_TTL_S`, anti-inondation, `PAD_BUCKETS`, UUIDs GATT, tailles de champ d'en-tête… Transcription de `synthese/05` §2. |
| `PacketType` (enum `#[repr(u8)]`) | `src/protocol/types.rs` | 13 types `0x01`–`0x0D`. `from_u8` / `to_u8`, `is_mvp()` (les `GOSSIP_*` = v2), `is_always_signed()`, `is_addressed() -> Option<bool>` (`None` = hérite, `Fragment`). **`Inventory = 0x0D`** (numéro figé par cette US). |
| `Flags` (newtype `u8`) | `src/protocol/types.rs` | Bitfield `ADDRESSED / SIGNED / FRAGMENT / RELAY_OK / PADDED` + `RESERVED_MASK`. `from_bits_truncate` (masque), `from_bits_raw` (préserve, diagnostic), `contains`, `has_reserved` (diagnostic — les bits réservés sont **ignorés**, pas rejetés, à la réception). Pas de crate `bitflags` (surface figée, minuscule). |
| `Header` | `src/protocol/types.rs` | En-tête L3 **décodé** (champs, pas d'octets). `header_len()` (22 broadcast / 30 adressé), `wire_len()`, `flags_are_consistent()`. La conversion octets ⇄ `Header` est US-201. |
| `AppFrameKind`, `AckStatus` | `src/protocol/types.rs` | Frames L4 dans Noise (`Message`, `Ack`, `ReadReceipt` v2, `Profile` post-MVP) ; statut d'accusé (`Delivered=2`, `Read=3` v2). |
| `ledger::Entry` | `src/ledger.rs` | Une entrée du journal : `seq`, `ts_ms`, `event_name`, `payload_json`, `prev_hash`, `entry_hash`, `sig`. `entry_hash = SHA-256(seq‖ts_ms‖len‖event_name‖len‖payload_json‖prev_hash)`, longueurs préfixées pour lever toute ambiguïté de découpage. |
| `ledger::Ledger<S: Signer>` | `src/ledger.rs` | Le journal lui-même : `append()`, `verify_chain()`, `export(range)`, `entries()`. Paramétré par un `Signer` injecté, pas câblé sur une implémentation Ed25519 concrète. |
| `ledger::Signer` / `ledger::NullSigner` | `src/ledger.rs` | Trait de signature + bouchon nul, tant que `crypto` (US-203, même sprint) n'existe pas — voir « Décisions ». |
| `ledger::Verdict` | `src/ledger.rs` | `Ok`/`Broken`/`Fork`/`Gap`, renvoyé par `verify_chain()`. Réutilisé tel quel par `dengon-verify::main`, qui n'en a plus de copie locale. |
| `Entry::to_bytes`/`Entry::from_bytes` | `src/ledger.rs` | Sérialisation binaire simple d'une entrée — sert le test de reprise après redémarrage, pas un vrai backend de stockage (voir « Décisions »). |
| `store::Store<K: KeySource>` | `src/store.rs` | Connexion SQLite + migrations. `open()`/`open_in_memory()`, puis `set_identity`/`get_identity_private_keys`, `upsert_contact`, `insert_conversation`, `insert_message`/`get_message_body`, `set_noise_session`/`get_noise_session_state`. |
| `store::KeySource` / `store::FixedKeySource` | `src/store.rs` | Trait qui fournit la clé de chiffrement des champs sensibles + bouchon à clé fixe (tests uniquement) — voir « Décisions », même schéma que `ledger::Signer` (US-206). |
| `sync::routing::Router<L>` | `src/sync/routing.rs` | Routeur d'un nœud, **sans I/O**, générique sur l'identifiant de lien `L` (`dengon_ble::LinkId` côté appelant). `new(cfg, seed)`, `link_up`/`link_down`, `bind_peer(link, peerID, mono_ms)` (anti-inondation par pair, revue #85), `on_packet(from, &Header, &MsgId, Now) -> Decision`, `poll_due(mono_ms) -> Vec<RelayOrder>`, `next_deadline()`, `cancel(&MsgId)` (pour `status`/`courier` quand un ACK passe), `note_originated(&MsgId, ts, Now)` (messages émis / rejoués par l'outbox), `stats()`. |
| `sync::routing::Now` | `src/sync/routing.rs` | `{ wall_ms, mono_ms }` : horloge murale (comparée à `timestamp_ms` seulement) + horloge monotone (quotas, seen-set, jitter). Revue #85. |
| `sync::routing::Decision` | `src/sync/routing.rs:183` | `Reject(RejectReason)` / `Deliver` / `Store` (enveloppe à déposer) / `NoRelay(NoRelayReason)` / `RelayScheduled { at_ms, ttl }`. Tout sauf `Reject` = paquet **nouveau**. |
| `sync::routing::RejectReason` | `src/sync/routing.rs:149` | `BadVersion`, `Malformed`, `UnknownLink`, `ClockSkew`, `Expired`, `LinkQuota`, `Duplicate`, `FloodLimited` — prêt pour l'événement `pkt.rejected` (US-208). |
| `sync::routing::RelayOrder<L>` | `src/sync/routing.rs:205` | `msg_id`, `ttl` à écrire, `targets` = tous les voisins **sauf la source**, calculés à l'échéance. |
| `sync::routing::RoutingConfig` | `src/sync/routing.rs:98` | Réglages, `new(local_id)` = valeurs de `protocol::consts` + 3 valeurs propres au routeur : `LINK_MAX_PKT_PER_S = 50` (`:70`), `BROADCAST_TTL_MAX = 3` (`:76`), `DUP_CANCEL_THRESHOLD = 2` (`:87`). |
| `SeenSet`, `RateWindow`, `SplitMix64` (privés) | `src/sync/routing.rs:563`, `:536`, `:616` | Seen-set borné (cap + expiration), fenêtre glissante bornée par son quota, PRNG 64 bits seedé pour le jitter. |
| `store::encrypt_field`/`decrypt_field` (privées) | `src/store.rs` | XChaCha20-Poly1305, nonce aléatoire de 24 o préfixé au résultat stocké, AAD liée au contexte de ligne/colonne. |
| `crypto::SigningKey` | `src/crypto.rs` | Clé privée Ed25519, construite depuis une graine de 32 octets (`from_seed`). `sign` est déterministe. `Debug` masque le secret. |
| `crypto::VerifyingKey` | `src/crypto.rs` | Clé publique. `from_bytes` rejette un point invalide ; `verify` utilise `verify_strict`. |
| `crypto::CryptoError` | `src/crypto.rs` | `InvalidPublicKey` / `InvalidSignature` (Ed25519) ; `Noise` (tout échec Noise, causes volontairement confondues), `PayloadTooLarge`, `InvalidPadding`, `HandshakeNotFinished`, `PayloadNotAllowed` (US-204). Implémente `core::error::Error` (aussi en `no_std`). |
| `StaticKeypair` | `src/crypto/noise.rs` | Paire X25519 statique depuis un secret de 32 o (`from_secret`, clé publique calculée par `curve25519-dalek` ; `public`). **Sa** copie du secret est effacée au `Drop` ; les copies faites par `snow` ne le sont pas (écart). Masqué au `Debug`. |
| `Handshake` | `src/crypto/noise.rs` | Noise `XX` : `initiator`/`responder(clé, rng)`, `write_message`/`read_message` (payload **non paddé**, **interdit au message 1** → `PayloadNotAllowed`), `remote_static`, `into_session`. Handshake = 32 + 96 + 64 = 192 o. La doc du type donne la garantie de chaque message. **Un échec de lecture est définitif** : recommencer un handshake complet. |
| `Session` | `src/crypto/noise.rs` | Transport `XX` **sans état** (`snow::StatelessTransportState`) : `encrypt` → `nonce(u64 BE) ‖ chiffré(padded)`, soit bucket + 24 o ; `decrypt` accepte pertes et désordre, rejette altération, rejeu et nonce hors fenêtre ; `remote_static` authentifiée. |
| `ReplayWindow` (privé) | `src/crypto/noise.rs` | Fenêtre anti-rejeu de 64 nonces (bitmap, RFC 6479) ; mise à jour **après** authentification seulement. |
| `seal` / `open` / `Opened` | `src/crypto/noise.rs` | Noise `X` one-shot : enveloppe = bucket + 96 o ; clair opaque, dont l'appelant assemble `AppFrame ‖ sender_pub_static ‖ sig` (06 §3) → `AppFrame` ≤ 1950 o ; `open` révèle `sender_static` (transporté chiffré). Mauvaise clé → `Err(Noise)`, jamais de panic. |
| `pad` / `unpad` / `PAD_BUCKETS` | `src/crypto/pad.rs` | Padding vers `[256, 512, 1024, 2048]`, clair max 2046 o (`MAX_PADDED_PAYLOAD`). `unpad` strict (taille, longueur, zéros). |
| `recipient_tag` / `own_tags` / `epoch_day` | `src/crypto/tag.rs` | Tag anonyme du jour ; `own_tags` = J-1, J, J+1 à matcher contre les `ENVELOPE_OFFER`. |
| `CallerResolver` (privé) | `src/crypto/rng.rs` | Resolver `snow` : primitives par défaut + RNG `rand_core` de l'appelant, cédé une seule fois. |
| `protocol::Packet` | `src/protocol/codec/mod.rs` | Paquet L3 complet : `Header` + payload opaque + `Option<Signature>`. |
| `protocol::decode` / `encode` / `encode_into` | `src/protocol/codec/mod.rs` | Octets ⇄ paquet. `decode` vérifie la forme **et** les règles type ⇄ drapeaux (`FrameRule` : colonnes `ADDRESSED`/`SIGNED` de `synthese/05` §4, `FRAGMENT` ⇔ `0x09`). Ne panique jamais. Pas de TTL, d'anti-rejeu ni de filtre MVP (→ `sync::routing`). |
| `protocol::signing_input` / `received_signing_input` | `src/protocol/codec/mod.rs` | Octets à signer / à vérifier : en-tête + payload **avec l'octet `ttl` à 0**, car un relais le décrémente (revue #80). La réception travaille sur les octets **reçus**. À passer à `crypto::SigningKey::sign` / `VerifyingKey::verify`. |
| `protocol::AppFrame` (`Message`, `Ack`) | `src/protocol/codec/app.rs` | Frames L4 (le clair dans Noise), `encode_app_frame` / `decode_app_frame`. Kinds hors MVP refusés. |
| `impl ledger::Signer for crypto::SigningKey` | `src/crypto.rs` | Branche une vraie clé Ed25519 sur `Ledger<S>` à la place de `NullSigner`. `crypto::Signature`/`SIGNATURE_LEN` sont des réexports de `protocol`. |
| `Identity` | `src/identity/keys.rs` | Identité locale : `StaticKeypair` + `SigningKey` + pseudo. `generate(pseudo, rng)` tire 32 o de secret X25519 puis 32 o de graine Ed25519 ; `peer_id` = `SHA-256(pub_static)[0..8]` ; `fingerprint` = `SHA-256(pub_static ‖ pub_sign)` ; `public`. `Debug` sans secret. |
| `PublicIdentity` | `src/identity/keys.rs` | Carte de contact (pseudo + 2 clés publiques), mêmes `peer_id`/`fingerprint`. Pseudo validé : 1–255 octets UTF-8. |
| `to_qr` / `from_qr` | `src/identity/qr.rs` | `dengon:v1:` + base64url sans padding de `len ‖ pseudo ‖ pub_static ‖ pub_sign`. Décodage strict, canonique, une erreur par cause. |
| `SafetyNumber` / `safety_number` / `verification_code` | `src/identity/safety.rs` | `SHA-512(min(fp) ‖ max(fp))`, 12 groupes `u40_be(5 o) % 100000`. Symétrique. `Display` = `"75116 36485 …"`, `digits()` = 60 chiffres. |
| `Identity::seal` / `unseal` | `src/identity/vault.rs` | Blob `"DGID" ‖ 1 ‖ nonce 24 ‖ XChaCha20-Poly1305(secret ‖ graine ‖ len ‖ pseudo)`, en-tête en AAD, nonce de la RNG injectée. Erreurs `VaultFormat` / `VaultVersion` / `VaultDecrypt`. |
| `Vault`, `MemoryVault`, `FileVault`, `load_or_create` | `src/identity/vault.rs` | Rangement d'octets opaques. `FileVault` (std) écrit via `.tmp` (supprimé puis recréé en `create_new`, mode `0600`) + `rename`, puis `fsync` du répertoire sous Unix. `load_or_create` : déchiffre si présent, sinon génère + scelle + enregistre → `peerID` stable. |
| `IdentityError` | `src/identity.rs` | `InvalidPseudo`, `Qr{Prefix,Version,Encoding,Length,PublicKey}`, `Vault{Format,Version,Decrypt}`, `VaultIo(ErrorKind)` (std). |

## Flux principal (exemple)

Réception d'un paquet (partie codée aujourd'hui, US-201) :

1. le transport livre des octets (après réassemblage L2, US-202) ;
2. `protocol::decode(raw)` → `Packet` ou `DecodeError` (→ futur `pkt.rejected`) ;
3. si `SIGNED` : `VerifyingKey::verify(&received_signing_input(raw)?, sig)` ;
4. `sync::routing` (US-209) : dédup, TTL, anti-rejeu, anti-inondation ;
5. `NOISE_MSG` déchiffré → `decode_app_frame(clair)` → `Message` ou `Ack`.

Le flux complet visé : `04-architecture.md` §4.

Session live Noise `XX` entre Alice (initiatrice) et Bob :

1. `Handshake::initiator(&alice, rng)` / `Handshake::responder(&bob, rng)`.
2. Alice `write_message(b"")` → m1 (`e`) → Bob `read_message(m1)`.
3. Bob `write_message(..)` → m2 (`e, ee, s, es`) → Alice `read_message(m2)` :
   elle connaît la clé statique de Bob (`remote_static`).
4. Alice `write_message(..)` → m3 (`s, se`) → Bob `read_message(m3)`.
5. `into_session()` des deux côtés ; `encrypt`/`decrypt` : clair paddé au
   bucket puis chiffré ChaCha20-Poly1305, nonce explicite en tête (le
   `NOISE_MSG` traverse des relais qui peuvent perdre ou réordonner).

Enveloppe pour Bob absent : `seal(&alice, &bob_pub, clair, rng)` ; la couche
trame (à venir) y ajoutera `recipient_tag(&bob_pub, epoch_day(now)) ‖ epoch_day`
et la signature Ed25519 ; Bob fait `open(&bob, env)`.

Appairage (US-205, test `appairage_a_et_b`) : au premier lancement,
`load_or_create(&mut coffre, &clé, "alice", rng)` génère et scelle
l'identité. Au lancement suivant, il la relit : le `peerID` ne change pas.
Alice affiche `alice.public().to_qr()` ; Bob le scanne
(`PublicIdentity::from_qr`), et inversement. Chacun affiche ensuite
`verification_code(&moi.public(), &contact)` : le même code de 60 chiffres
des deux côtés.

`ledger`, lui, a déjà un flux exécutable :

```
Ledger::new(signer)
  .append("msg.queued", r#"{"msg_uuid":"..."}"#, ts_ms)   → seq=0, prev_hash=GENESIS
  .append("msg.handed_off", r#"{...}"#, ts_ms)             → seq=1, prev_hash=hash(entrée 0)
  ...
  .verify_chain()                                          → Verdict::Ok

# Reprise après redémarrage (simulée, voir tests) :
entries.iter().flat_map(Entry::to_bytes) → écrit quelque part (hors périmètre de ce module)
                                          → relu, Entry::from_bytes en boucle
Ledger::from_entries(entries_relues, signer).verify_chain() → Verdict::Ok
```

`store`, lui aussi, a un flux exécutable :

```
Store::open("dengon.db", key_source)
  → run_migrations() (schema_migrations, IF NOT EXISTS, une transaction par migration)
  → upsert_contact(peer_id, ...) → insert_conversation(conv_id, peer_id)
  → insert_message(msg_uuid, conv_id, ..., body="salut", ...)
       body chiffré (XChaCha20-Poly1305 + AAD) AVANT le INSERT — jamais en clair sur disque
  → get_message_body(msg_uuid) → déchiffre, renvoie "salut"
```

Le flux applicatif complet (Alice écrit → chiffrement → trame → diffusion BLE
→ relais → Bob → accusé) reste décrit dans
[`04-architecture.md`](../../synthese/04-architecture.md) §4 — `ledger` n'en
est qu'un maillon (la traçabilité) et `store` la persistance locale, pas le
chemin des messages.

Côté trame : US-108 livre les **types** (`PacketType`, `Flags`, `Header`), pas
encore le codec (US-201).

## Dépendances

- **Internes :** aucune. C'est la racine du graphe — les cinq autres crates
  dépendent d'elle, elle ne dépend de personne. C'est volontaire : une
  dépendance sortante de `dengon-core` serait une dépendance imposée au
  firmware ESP32.
- **Externes (runtime) :** `sha2` 0.10 (`default-features = false`, no_std)
  pour `ledger` et `recipient_tag` ; `ed25519-dalek` 2.2 (`default-features =
  false`, feature `zeroize`) ; **`snow` 0.10.0** (`>=0.10.0, <0.11`, jamais 0.9.x ;
  `default-features = false` + `default-resolver`, `use-curve25519`,
  `use-chacha20poly1305`, `use-sha2` — config du Spike A, **sans**
  `use-getrandom`) ; `hmac` 0.12 ; `rand_core` 0.6 (trait du RNG injecté) ;
  `zeroize` 1 (feature `alloc`, pour
  `Zeroizing<Vec<u8>>`) ; **`chacha20poly1305` 0.10** (`default-features =
  false`, `alloc` : XChaCha20 du coffre, US-205, et de `store`) ;
  **`data-encoding` 2** (`alloc` : base64url du QR, base32 du `peerID`,
  US-205). `protocol` n'utilise que `core`.
- **Externes (crates), `store` seulement (feature `std`) :** `rusqlite`
  (`features = ["bundled"]` — sqlite3 vendorisé en C, pas de dépendance
  système) ; la feature `std` ajoute `getrandom` à `chacha20poly1305` (pour
  `aead::OsRng`) : `getrandom` n'est donc tiré que par la feature `std`.
  Viendront encore `serde`.
- **Externes (dev) :** `rand_chacha` 0.3 (RNG déterministe des vecteurs),
  `dengon-ble` (US-209 : `MockTransport` pour `tests/routing_mock.rs` — cycle
  de dépendances *de dev* seulement, `dengon-ble` dépendant de `dengon-core` ;
  Cargo l'autorise), `proptest` 1 (property tests de `ledger`, `crypto`, du
  codec et de `sync::routing`), `serde_json` 1
  (lecture des vecteurs via `Value`, sans derive). N'affectent pas la
  compilation `no_std` (`cargo check` ne compile pas les dev-deps).

## Décisions d'implémentation

- **Bascule `no_std`** : `#![cfg_attr(not(feature = "std"), no_std)]` en
  `src/lib.rs`, avec une feature `std` activée par défaut. Retirer la feature
  active `#![no_std]` **sur la cible hôte**, ce qui suffit à détecter tout usage
  involontaire de `std` sans avoir à installer une cible bare metal. La CI le
  vérifie à chaque PR (`cargo check -p dengon-core --no-default-features`).
- **`extern crate alloc;` ajouté avec `ledger` (US-206)** : la note
  précédente de cette fiche disait « pas de `extern crate alloc;` tant qu'il
  n'est pas utilisé » (`unused_extern_crates` de `rust_2018_idioms` l'aurait
  fait échouer sous `-D warnings`) — `ledger` a maintenant besoin de
  `Vec`/`String` en `no_std`, donc la déclaration est ajoutée. Elle ne coûte
  rien en mode `std` (`alloc` y est déjà réexporté par la libstd), donc le
  code de `ledger` est identique dans les deux configurations.
- **Signature différée via le trait `Signer`** (US-206) : `crypto` (US-203)
  n'existe pas encore — même sprint, et la règle du projet interdit qu'une US
  dépende d'une autre US du même sprint. `Ledger<S: Signer>` est donc
  paramétré par un trait plutôt que câblé sur Ed25519 en dur ; `NullSigner`
  sert de bouchon pour les tests. Quand `crypto` arrivera, un vrai `Signer`
  se branche sans changer la forme de `Ledger`. `verify_chain()` ne vérifie
  donc **pas** la signature pour l'instant (vérifier une signature nulle ne
  prouverait rien) — écart consigné dans `03-ecarts-conception.md`.
- **Persistance hors périmètre de `ledger`** : le module ne fait aucune E/S.
  Le critère d'acceptation « reprise après redémarrage » est démontré par un
  aller-retour `Entry::to_bytes`/`from_bytes` en mémoire (sérialiser, tout
  détruire, désérialiser, revérifier) — la vraie écriture sur SQLite
  (`store`, US-207) ou littlefs (firmware, US-308) reste à câbler par la
  couche appelante.
- **`dengon-verify` réutilise `ledger::Verdict`** au lieu d'en garder une
  copie locale, comme annoncé par `04-architecture.md` §2 (« le binaire
  `dengon-verify` réutilise `ledger::verify_chain` ») — la duplication
  aurait fini par diverger silencieusement.
- **`verify_chain()` corrigé après auto-revue (2026-09-26)** : la version
  d'origine détectait `Fork`/`Gap` en ne comparant chaque `seq` qu'à celui
  de l'entrée immédiatement précédente dans le stockage. Une séquence
  `[0, 1, 2, 1]` (rejeu d'une entrée déjà vue, mais pas juste après
  l'original) était donc classée à tort `Gap` au lieu de `Fork`. Rien
  n'était accepté à tort (aucune entrée invalide ne passait comme `Ok`),
  mais le verdict précis était faux. Réécrit en deux passes : passe 1 sur
  un `BTreeSet<u64>` des `seq` (détecte `Fork`/`Gap` indépendamment de
  l'ordre de stockage), passe 2 = la marche de chaîne de hash d'origine
  (`Broken`). Voir `00-journal.md`, entrée du 2026-09-26 dédiée.
- **Arithmétique `seq` sécurisée contre le débordement `u64::MAX`** (retour
  de revue #75, OswinFreyr) : `append()` calculait `e.seq + 1` et
  `verify_chain()` calculait `max + 1`, tous deux en `u64` non vérifié — un
  export corrompu ou hostile portant `seq = u64::MAX` faisait paniquer le
  calcul en debug/test et reboucler silencieusement à 0 en release, un
  vérificateur qui panique ou ment sur une entrée hostile étant exactement
  ce qu'il ne faut pas. `append()` utilise maintenant `saturating_add`
  (infaillible par signature — au pire deux entrées consécutives
  partageraient `seq = u64::MAX`, détecté comme `Fork`) ; `verify_chain()`
  compare en arithmétique `u128`, qui ne peut pas déborder pour des
  opérandes `u64`. Test dédié :
  `seq_u64_max_ne_panique_pas_et_est_detecte`.
- **`store` reste `std`-only, volontairement** (US-207) : contrairement à
  `protocol`/`sync`/`ledger`, `store` n'a pas vocation à compiler en
  `no_std` — `rusqlite` vendorise sqlite3 en C, absent d'ESP32.
  `04-architecture.md` §2 le dit explicitement : « `store` est derrière un
  trait `Store` (impl `rusqlite` natif, impl NVS/flash sur ESP32) », donc
  l'implémentation ESP32 sera un module séparé, pas celui-ci.
  `std = ["dep:rusqlite", "dep:chacha20poly1305"]` dans `Cargo.toml` active
  `store` en même temps que `std`, pas de nouveau flag à retenir.
- **Clé de chiffrement différée derrière `KeySource`** (US-207), même
  schéma que `ledger::Signer` (US-206) : `identity` (US-205) — qui garde la
  vraie clé, idéalement en Keystore/Keychain — est dans le **même sprint**
  que `store`, et la règle du projet interdit la dépendance intra-sprint.
  `FixedKeySource` (clé fixe) sert de bouchon aux tests. Écart consigné
  dans `03-ecarts-conception.md`.
- **XChaCha20-Poly1305, nonce aléatoire par appel** : jamais le même nonce
  deux fois avec la même clé (condition de sécurité d'un AEAD en mode
  compteur/stream). Le nonce est stocké en clair, préfixé au texte chiffré
  — c'est la pratique normale, un nonce n'a pas besoin d'être secret,
  seulement unique.
- **Migrations calquées sur `dashboard/api/app/migrations.py`** (US-110,
  déjà revue) : `(version, nom, [instructions])`, `CREATE ... IF NOT
  EXISTS`, une transaction par migration, jamais rejouée une fois dans
  `schema_migrations`. Même discipline, langage différent.
- **Schéma repris tel quel de `docs/synthese/09-dashboard-et-donnees.md`
  §11.1** : les 11 tables sont créées par la migration initiale ; seules
  `identity`, `contacts`, `conversations`, `messages` et `noise_sessions`
  ont des méthodes CRUD pour l'instant (celles nécessaires pour prouver le
  chiffrement champ par champ) — `outbox`, `held_envelopes`, `seen_set`,
  `recon_cache`, `ledger`, `ship_cursor` attendent `sync` (US-209..212) et
  `ledger` (US-206) pour avoir un appelant.
- **AAD ajoutée au chiffrement champ par champ après auto-revue
  (2026-09-26)** : la version d'origine ne liait le texte chiffré à aucun
  contexte de ligne/colonne, donc un attaquant à écriture sur le fichier
  `.db` aurait pu copier le blob chiffré d'une ligne vers une autre (ex.
  substituer le corps d'un message par celui, chiffré, d'un autre message)
  sans que le déchiffrement échoue. `encrypt_field`/`decrypt_field`
  prennent maintenant un `aad` (`identity.priv_static`/`priv_sign` liés à
  une constante de colonne, `messages.body` à `msg_uuid`,
  `noise_sessions.state` à `peer_id`) — un texte chiffré présenté sous un
  mauvais contexte échoue explicitement. Voir `00-journal.md`, entrée
  dédiée du 2026-09-26.
- **AAD de `messages.body` élargie à toute la ligne** (retour de revue #76,
  OswinFreyr) : lier `messages.body` au seul `msg_uuid` protégeait contre
  une substitution de blob **entre lignes différentes**, mais pas contre
  une modification des **autres colonnes de la même ligne**
  (`author_peer_id`, `conv_id`, `direction`) — un attaquant à écriture sur
  le fichier `.db` pouvait réattribuer un message à un autre auteur/une
  autre conversation sans que le déchiffrement du corps échoue. L'AAD
  inclut maintenant un préfixe de domaine (`"messages.body"`) et ces trois
  colonnes, recalculée à la **lecture** depuis les valeurs réellement
  stockées (pas fournies par l'appelant) — toute incohérence entre le
  contenu chiffré et les métadonnées de sa ligne fait échouer le
  déchiffrement. Même préfixe de domaine ajouté à `noise_sessions.state`
  (`"noise_sessions.state"` + `peer_id`, déjà unique par ligne). Test dédié
  : `trafiquer_lauteur_dun_message_casse_le_dechiffrement`.
- **`upsert_contact` : rotation de clé publique trace `key_changed_at` et
  efface `verified_at`** (retour de revue #76, OswinFreyr) : la version
  d'origine écrasait `pub_static`/`pub_sign` sans toucher ces deux
  colonnes — un contact vérifié par l'utilisateur restait « vérifié »
  après qu'un pair ait annoncé le même `peer_id` avec d'autres clés
  (rotation légitime ou usurpation, `verified_at`/`key_changed_at` existent
  justement pour distinguer les deux). Le `ON CONFLICT DO UPDATE` compare
  maintenant les clés avant/après (`CASE WHEN ... THEN NULL/?6 ELSE
  contacts.verified_at/key_changed_at END`) : mêmes clés → statut
  inchangé, clés différentes → `verified_at` effacé et `key_changed_at`
  horodaté (nouveau paramètre `now_ms`). Au passage, `pseudo =
  COALESCE(excluded.pseudo, contacts.pseudo)` au lieu de `pseudo =
  excluded.pseudo` : un appel avec `pseudo=None` n'efface plus un pseudo
  déjà connu. Tests dédiés :
  `changer_les_cles_d_un_contact_efface_son_statut_verifie`,
  `upsert_contact_ne_vide_pas_un_pseudo_deja_connu`.
- **`protocol::{consts, types}` séparé de `protocol::codec`** (US-201) : permet
  à `sync::*` de démarrer sans la sérialisation. C'est l'objet même de l'US-108.
- **`no_std` garanti pour `protocol`** : n'importe que `core`
  (`core::ops::RangeInclusive`, `core::ops::BitOr`). Vérifié par
  `cargo check -p dengon-core --no-default-features`.
- **Bitfield maison** plutôt que la crate `bitflags` : 5 bits, API figée.
- **`Flags::from_bits_truncate` ignore les bits réservés ; `from_bits_raw` les
  préserve** (retour de revue #63, point de Paul) : avant, aucun constructeur
  public ne pouvait poser un bit 5-7, donc `has_reserved()` était inatteignable
  hors du module — un test devait lire `raw[3] & Flags::RESERVED_MASK`
  directement au lieu de l'API.
- **Bit réservé posé ⇒ `has_reserved() == true`, mais plus un motif de rejet**
  (retour de revue #63, point de Paul) : `synthese/05:80` — « ignoré à la
  réception », pas « non conforme ». `Header::flags_are_consistent()` ne
  vérifie plus l'absence de bits réservés ; sans ce correctif, un bit v1.1
  futur aurait fait jeter 100 % du trafic v1.1 par un nœud v1.0.
- **`is_always_signed()` ne compte plus `GossipPush`** (retour de revue #63,
  point de Paul) : `synthese/05:122` le dit non signé (son payload est une
  liste de paquets déjà signés individuellement).
- **`is_addressed() -> Option<bool>`**, pas `bool` (retour de revue #63,
  point de Paul) : `Fragment` **hérite** de l'adressage du paquet transporté
  (`synthese/05:123`) — ni oui ni non, un troisième cas qu'un booléen ne peut
  pas représenter. Avant, le test d'intégration devait court-circuiter
  `Fragment` avec un `if pt != Fragment` pour contourner l'API.
- **`Header` porte `recipient_id: Option<PeerId>`** et non un `PeerId` + booléen :
  rend l'invariant « présent ⇔ `ADDRESSED` » vérifiable (`flags_are_consistent`).
- **Vecteurs v0 dans `crates/dengon-core/tests/`** et non `contracts/packet/` :
  le dossier `contracts/` n'est pas encore sur `main` (PR #60). Déplacement
  prévu, consigné dans `03-ecarts-conception.md`.
- **`is_rejected()` (test) : garde de longueur `raw.len() < hdr`, pas
  `hdr + 2`** (retour de revue #63, point de Paul) : `hdr` (`HEADER_LEN_
  BROADCAST`/`_ADDRESSED`) inclut déjà les 2 octets de `payload_len`
  (`consts::tailles_den_tete_coherentes`). L'ancienne garde rejetait à tort
  tout paquet valide avec `payload_len ∈ {0, 1}`, et rendait 3 des 6 vecteurs
  `reject` détectables par cette règle de longueur plutôt que par celle
  qu'ils nomment (version/type/réservé) — vérifié en isolant chaque contrôle.
- **`reserved-flag-set` a quitté `reject` pour `accept`** (renommé
  `noise-msg-addressed-reserved-bit-ignored`, retour de revue #63, point de
  Paul) — c'est la conséquence directe du point précédent : un bit réservé
  posé n'est plus une raison de rejet. `bad-version`/`unknown-type` allongés
  de 2 octets pour rester rejetés par leur propre règle même sous une garde
  de longueur hypothétiquement encore buguée.
- **`announce-broadcast-signed`/`log-attest-broadcast-signed` : `RELAY_OK`
  ajouté** (`flags` `0x02`→`0x0a`, retour de revue #63, point de Paul) : ces
  deux paquets broadcast avaient `RELAY_OK` absent avec un TTL de 2-3 — ils
  seraient morts au premier saut (`synthese/05:203`, `RELAY_OK && ttl > 1 ?`),
  rendant le TTL non nul incohérent avec un relais impossible.
- **`cast_possible_truncation`/`cast_sign_loss`/`cast_possible_wrap` activés**
  dans `Cargo.toml` racine (retour de revue #63, point de Paul) : commentés
  « à activer avec `protocol` (US-108) » — c'est cette US. Un seul site
  touché (`i as u8` dans un test → `u8::try_from(i).unwrap()`).
- **Bascule `no_std`** : `#![cfg_attr(not(feature = "std"), no_std)]` dans
  `src/lib.rs`, avec une feature `std` activée par défaut. Retirer la feature
  active `#![no_std]` **sur la cible hôte**, ce qui suffit à détecter tout usage
  involontaire de `std` sans avoir à installer une cible bare metal. La CI le
  vérifie à chaque PR (`cargo check -p dengon-core --no-default-features`).
- **`extern crate alloc;`** ajouté par US-204 (`snow` et `crypto::noise`
  allouent des `Vec`). Tant qu'`alloc` n'était pas utilisé, il était absent :
  `unused_extern_crates` (groupe `rust_2018_idioms`) l'aurait refusé.
- **Aléa injecté** (US-204) : chaque fonction Noise qui tire une clé
  éphémère prend un `R: RngCore + CryptoRng + Send + Sync + 'static` par
  valeur. Pas de `getrandom` : sur ESP32, l'appelant fournira un RNG sur
  `esp_fill_random` (US-307/308).
- **Accès `pub(crate)` aux secrets** (US-205) : `StaticKeypair::secret()`
  (`crypto/noise.rs`) et `SigningKey::to_seed()` (`crypto.rs`, rendu
  dans un `Zeroizing`) servent uniquement à `Identity::seal`. Hors de la
  crate, un secret ne sort toujours pas.
- **Pas de `Clone` sur `Identity`** (rebase de #82) : `SigningKey` n'est plus
  `Clone` depuis la revue de #78, et aucun appelant ne clonait une identité.
- **Coffre sans `OsRng`** (US-205) : le nonce XChaCha20 de 24 octets vient
  de la RNG injectée, comme les éphémères Noise. `chacha20poly1305` est donc
  utilisé sans `getrandom` par `identity`, qui compile en `no_std` (seule
  la feature `std`, donc `store`, active `getrandom`).
- **Code de vérification sur 5 octets par groupe** (US-205, décision de
  Paul) : c'est un écart par rapport au `u16` de la conception, voir
  `03-ecarts-conception.md`.
- **Padding dans le chiffré** (US-204) : appliqué au clair, avant Noise, pour
  le transport et les enveloppes `X`. **Pas** pour le handshake (revue #81) :
  le message 1 part en clair et refuse tout payload, et avec des payloads vides
  les tailles sont fixées par le motif. Écart vs 06 §3, consigné.

- **`sync::routing` est sans I/O (US-209)** : pas d'appel à `Transport`
  (impossible : `dengon-ble` dépend de `dengon-core` et tire `std`), pas
  d'horloge (`now_ms` en argument), pas d'aléa système (graine au
  constructeur, PRNG SplitMix64 maison). Conséquences : `no_std`, testable
  sans radio, **déterministe à graine fixe**. Le relais est découpé en deux
  temps — `on_packet` programme, `poll_due` rend ce qui est échu — ce qui
  permet le « écouter avant de rediffuser » sans thread ni timer.
- **Ordre du pipeline** (`decide`, `src/sync/routing.rs:398`) : version →
  cohérence des drapeaux → lien connu → horloge (futur > 2 h / passé >
  24 h) → quota brut du lien (doublons compris) → dédup → anti-inondation
  (nouveaux `msgID` seulement) → insertion au seen-set → livraison locale →
  relais. Un `msgID` refusé par l'anti-inondation **n'entre pas** au
  seen-set, pour qu'un voisin honnête puisse le relivrer.
- **Seuil d'annulation pendant le jitter = 2 doublons, pas 1** : écart
  consigné (le seuil littéral affame un losange, prouvé par
  `seuil_litteral_affame_le_losange`).
- **Mémoire bornée sous flood** : les fenêtres glissantes ne retiennent que
  les événements *acceptés* (≤ quota), le seen-set est plafonné à
  `SEEN_SET_CAP`, et un `msgID` n'y est inséré qu'une fois (l'ordre
  d'insertion est l'ordre d'âge : une `VecDeque` suffit, pas de LRU fin).
  **Remplacé en revue #85** : l'échéance d'une entrée dépend maintenant de
  l'horodatage du paquet, le seen-set est indexé par échéance
  (`BTreeSet<(échéance, msgID)>`) et évince l'entrée qui expire le plus tôt.
- **Retours de revue #85 (OswinFreyr)** : (1) anti-inondation compté par
  `peerID` du voisin via `bind_peer`, conservé après `link_down` jusqu'à ce
  que la fenêtre se vide — une reconnexion (nouveau `LinkId`) ne rend plus
  de quota ; (2) un paquet plus vieux que l'horizon du seen-set (5 min)
  est accepté mais **pas relayé** (`NoRelay(Late)`), l'entrée vit jusqu'à
  `max(réception, horodatage) + SEEN_TTL_S` — un porteur de retour ne
  relance plus de flood, la dédup longue durée de `Deliver` revient au
  `store` ; (3) deux horloges (`Now`) ; (4) `note_originated` pour les
  messages émis localement. Trois écarts consignés.

## Tests

- `src/lib.rs`, module `tests` : deux tests fumigènes (version de crate,
  `PROTOCOL_VERSION`).
- **Codec (US-201)** : `src/protocol/codec/mod.rs` (14 tests : aller-retours,
  chaque `DecodeError` / `EncodeError`, bits réservés masqués, `ANNOUNCE` sans
  `SIGNED` refusé, règle `FRAGMENT`, entrée de signature identique après
  décrémentation du TTL), `codec/app.rs` (5 tests), `tests/codec_proptest.rs`
  (7 properties × 512 cas : `decode(encode(p)) == p`, préfixe refusé, octets
  arbitraires sans panic, même entrée de signature quel que soit le TTL reçu),
  `tests/protocol_vectors.rs` (+3 : vecteurs v0 passés au vrai décodeur).
  Couverture mesurée par le job CI `core` avant le rebase : `codec/mod.rs`
  95,7 %, `codec/app.rs` 100 %.
- `src/crypto.rs`, module `tests` : 16 tests — 3 KAT RFC 8032 §7.1 sur 4 vecteurs
  (clé publique, signature octet à octet, `verify`), aller-retour, déterminisme,
  négatifs (forgée, bit-flip exhaustif message + signature, tronqué, mauvaise
  clé, clé invalide, faible ordre), et `signe_le_journal_chaine` (un `Ledger`
  signé par `SigningKey`, signature vérifiée sur `entry_hash`).
- `src/ledger.rs`, module `tests` : 14 tests unitaires (chaîne vide valide,
  append→verify_chain toujours Ok, seq consécutives, trou détecté, doublon
  de seq détecté comme fork (adjacent **et** non adjacent — voir
  « Décisions »), **`seq = u64::MAX` détecté comme `Gap` sans paniquer**
  (nouveau, retour de revue #75 : `max + 1` en `u64` débordait — voir
  « Décisions »), entrée modifiée détectée comme broken, `prev_hash`
  incohérent détecté, export par plage, reprise après redémarrage par
  sérialisation, désérialisation d'un buffer tronqué sans panique) +
  **2 property tests** (`proptest`) : toute séquence d'appends reste
  vérifiable ; corrompre n'importe quelle entrée d'une séquence quelconque
  est toujours détecté comme `Broken`.
- `src/store.rs`, module `tests` : 13 tests — migrations rejouables sans
  erreur, round-trip identité/message/session Noise (chiffré puis
  déchiffré, on retrouve le texte d'origine), deux chiffrements du même
  texte donnent des octets différents (nonce aléatoire), déchiffrer avec la
  mauvaise clé échoue, déchiffrer une donnée modifiée échoue (garantie
  d'authenticité Poly1305), déchiffrer un buffer tronqué échoue sans
  paniquer, déchiffrer avec un mauvais contexte AAD échoue (protection
  anti-substitution entre lignes), **trafiquer `author_peer_id` d'un
  message casse son déchiffrement** (nouveau, retour de revue #76 : l'AAD
  ne liait `messages.body` qu'à `msg_uuid`, pas au reste de la ligne — voir
  « Décisions »), **changer les clés d'un contact efface son statut
  vérifié et trace le changement, un ré-appel avec les mêmes clés le
  conserve** (nouveau, retour de revue #76), **`upsert_contact` avec
  `pseudo=None` ne vide pas un pseudo déjà connu** (nouveau, retour de
  revue #76), et le test central du critère d'acceptation : **écrire un
  message connu sur un vrai fichier `.db`, puis `grep` binaire sur le
  fichier — le texte en clair n'y est pas**.
- `src/protocol/consts.rs` — 5 tests : valeurs de référence, cohérence des
  tailles d'en-tête, UUIDs GATT, sens des plages.
- `src/protocol/types.rs` — 8 tests : discriminants contigus `0x01`–`0x0D`,
  `from_u8` inverse de `to_u8`, périmètre MVP, bits de `Flags`, opérations
  (dont `from_bits_raw` vs `from_bits_truncate` sur un bit réservé),
  `Header::{header_len, wire_len, flags_are_consistent}`, **un bit réservé
  posé n'invalide plus `flags_are_consistent()`** (nouveau, retour de revue
  #63), `AppFrameKind`/`AckStatus`.
- `tests/protocol_vectors.rs` — 4 tests : les 8 vecteurs `accept` sont
  structurellement cohérents avec leurs `expect` (via `protocol::{consts,
  types}`) ; les 5 vecteurs `reject` violent chacun une règle du format ;
  `Inventory` a bien le type `0x0D` ; **les 8 vecteurs `accept` ont tous
  `ttl > 1` et portent tous `RELAY_OK`**
  (`accept_vectors_with_ttl_above_1_have_relay_ok`, nouveau — retour de revue
  #63, round 3, précisé au round 4 : le test couvre les 8 vecteurs
  `accept`, adressés (`ack-addressed`, `noise-msg-addressed`) compris — la
  règle réelle est « `ttl > 1` ⇒ `RELAY_OK` », quel que soit le type de
  paquet, pas seulement broadcast) garantit que cet invariant
  (`synthese/05:203`) reste vrai vecteur par vecteur, pas seulement pour
  les deux corrigés au round 2).
- `src/crypto/{noise,pad,tag,rng}.rs` (US-204) : handshake `XX` complet,
  authentification mutuelle, échange bidirectionnel, message de handshake
  altéré, hors séquence, session avant fin, chiffré altéré, **rejeu**, trop
  grand ; **perte tolérée**, **réordonnancement toléré** (puis rejeux
  refusés), nonce trop ancien, nonce forgé qui ne fait pas avancer la fenêtre,
  chiffré tronqué, nonces épuisés, nonce big-endian ; fenêtre anti-rejeu
  (glissement, grand saut) ; `X` aller-retour, expéditeur absent du clair, **mauvaise clé →
  erreur propre**, enveloppe altérée/tronquée/vide ; **deux messages de 2 et
  200 octets → trames de même taille** (session et enveloppe) ; clé publique
  X25519 = RFC 7748 §6.1 ; **payload refusé au message 1** (à l'écriture, et à
  la lecture d'un message 1 forgé directement dans `snow`) ; **tailles de
  handshake 32/96/64** ; payload de handshake trop grand ; padding aux bornes des buckets ; tags stables sur la
  journée, différents le lendemain et par destinataire, conformes à la formule.
- **Property tests** (`proptest`) : `unpad(pad(x)) == x`, `decrypt(encrypt(x))
  == x` (64 cas), `open(seal(x)) == x` (64 cas).
- `tests/crypto_vectors.rs` : `vecteurs_conformes` (recalcul == fichier),
  `vecteurs_rejouables` (Bob rejoue le handshake, déchiffre, ouvre
  l'enveloppe ; clés éphémères recalculées depuis `rng_seed` — flux ChaCha20
  standard — et vérifiées contre `e.pub` des messages ; elles ne sont pas
  écrites dans le fichier, GitGuardian les signalant comme secrets).
  Régénération volontaire :
  `cargo test -p dengon-core --test crypto_vectors -- --ignored generer_vecteurs`.
- `src/identity/*` (US-205), 27 tests unitaires + 8 property tests :
  - `peer_id` et empreinte recalculés à la main, génération déterministe
    pour une RNG donnée, pseudo vide ou de 256 octets refusé, base32
    (RFC 4648), `Debug` sans secret ;
  - format exact du QR, et chaque erreur de `from_qr` (préfixe, version,
    padding ou caractère interdit, longueur, pseudo vide ou non UTF-8, point
    Ed25519 invalide) ;
  - code recoupé à la main, affichage en 12 groupes ;
  - `seal`/`unseal` : aller-retour, deux scellements différents, pas de
    secret ni de pseudo en clair dans le blob, mauvaise clé, signature de
    fichier, version, blob tronqué, clair authentique mais mal formé ;
  - `load_or_create` stable et avec une mauvaise clé ;
  - `FileVault` : « grep binaire » du fichier, absence du `.tmp`, mode
    `0600`, réouverture, erreur d'E/S (sous-répertoire dédié, portable
    Windows), `.tmp` résiduel en `0644` remplacé (Unix).
- **Property tests `identity`** :
  - aller-retour QR dans les deux sens (`from_qr(to_qr(p)) == p` et
    `to_qr(from_qr(s)) == s`) ;
  - `from_qr` sans panique sur une chaîne arbitraire et sur des octets
    arbitraires ;
  - **symétrie** `safety_number(a, b) == safety_number(b, a)`, toujours 60
    chiffres, une empreinte différente donne un code différent ;
  - aller-retour `seal`/`unseal` pour tout pseudo, et un bit modifié
    n'importe où est refusé.
- `tests/identity_vectors.rs` :
  - `vecteurs_conformes` et `vecteurs_rejouables` ;
  - `appairage_a_et_b` : A et B affichent le même code, et une carte MITM le
    change.
  Tout le fichier (clés, `peerID`, base32, empreintes, QR, code) a été
  recalculé en Python (`cryptography`) depuis les graines ChaCha20 :
  identique.
- Commande : `cargo test -p dengon-core` → **141 unitaires + 3 (vecteurs identity) + 2 (vecteurs
  crypto) + 4 (vecteurs protocole) passés**, 2 ignorés (générateurs), 0 échec —
  2026-09-28, après rebase sur la tête revue de US-204 (#81), elle-même sur
  `main` (`ledger`, `store`).
  `cargo clippy --workspace --all-targets -- -D warnings` et `cargo fmt --all
  -- --check` verts. `cargo check -p dengon-core --no-default-features`
  (frontière `no_std`) vert.
- Couverture (`cargo llvm-cov -p dengon-core --all-features`, mesurée avant le
  rebase sur `ledger`/`store`) : **99,14 % des lignes** ; `crypto` : 98,3 à
  100 % selon le fichier.
- `src/sync/routing.rs`, module `tests` (US-209) : **30 tests unitaires**
  (une ligne de `synthese/10` §4.2 = au moins un test : dédup, TTL
  décrémenté, `ttl ≤ 1` → pas de relais, clamp à 6 voisins, `RELAY_OK`
  absent, abandon sur doublons pendant le jitter, anti-inondation au 21ᵉ
  `msgID` + libération après 60 s + autre voisin non pénalisé ; plus
  quota de lien, horloge, expiration à 24 h, enveloppe déposée, livraison
  locale, cibles calculées à l'échéance, seen-set borné / expiré, vecteur de
  référence SplitMix64) + **2 property tests** : chaque `msgID` relayé au
  plus une fois (et `ttl' < ttl`, clamp respecté, jitter dans
  `RELAY_JITTER_MS`) ; même graine + même séquence ⇒ même trace.
  Revue #85 : **+11 tests** (43 au total) — reconnexion sans regain de
  quota, report du quota au `bind_peer`, purge des pairs partis, porteur de
  retour non re-floodé, `Deliver` répété au-delà de l'horizon (assumé),
  horodatage en avance retenu tant qu'il est frais, recul de l'horloge
  murale sans gel des relais ni des quotas, seen-set en monotone,
  `note_originated` (doublon au retour, ne raccourcit pas une entrée).
- `tests/routing_mock.rs` (US-209) : **8 tests de bout en bout** — des
  `MockTransport` reliés par un « fil » de test : chaîne A–B–C–D (livré à
  D avec TTL 5), losange (1 seul exemplaire, ≤ 1 relais par nœud), seuil
  littéral qui affame le losange (régression documentée), portée bornée
  par TTL 3 sur une chaîne de 8, clamp de densité + exclusion de la
  source, **inondation** 1 voisin et 3 voisins, déterminisme de la trace.
  Chiffres mesurés (sortie `--nocapture`) : 1 voisin à **500 msg/s
  pendant 60 s** → 30 000 reçus, **20 relais, 40 trames émises**, 27 000
  refusés par le quota de lien, 2 980 par l'anti-inondation, seen-set = 20 ;
  3 voisins → **60 relais, 120 trames**.
- Commande (2026-09-28, US-209) : `cargo test -p dengon-core` → **86 passés**
  (74 lib + 4 `protocol_vectors` + 8 `routing_mock`).
- Couverture `cargo llvm-cov -p dengon-core --summary-only` (2026-09-28) :
  `sync/routing.rs` **99,16 % des lignes**, 98,17 % des régions ; total
  crate 97,69 % des lignes.
- Négatif vérifié en local : la garde de longueur `hdr + 2` réintroduite
  temporairement fait échouer `accept_vectors_are_structurally_consistent`
  sur le nouveau vecteur `noise-msg-addressed-reserved-bit-ignored` (30
  octets, exactement `hdr`) — confirme le « mirror bug » signalé par Paul
  (un paquet valide à `payload_len` faible rejeté à tort).
- Couverture de `ledger` non mesurée à l'époque (`cargo llvm-cov` pas encore
  posé) — depuis mesurée avec l'US-209 (voir ci-dessus), mais chaque branche de `verify_chain` (Ok/Broken/Fork/Gap) a un test
  dédié qui l'exerce explicitement.

## Limites connues / TODO

- Deux rangements de l'identité au repos coexistent (`identity::Vault` et
  `Store::set_identity`), aucun n'est appelé : à unifier (écart « Coffre
  d'identité »).
- `snow` 0.10.0 n'efface aucune clé : seules les copies détenues par
  `StaticKeypair` et `SigningKey` sont effacées (écart consigné).
- `crypto::open` n'a pas d'anti-rejeu : une enveloppe `X` réinjectée s'ouvre à
  nouveau ; dédupliquer par `msg_id` dans `sync` / `store`.
- `verify_chain()` ne vérifie pas la signature (voir « Décisions »). La
  brique existe depuis US-203 (`crypto::SigningKey` implémente
  `ledger::Signer`) mais la vérification n'est pas câblée.
- **`verify_chain()` ne peut pas re-vérifier un export partiel** (`seq` ne
  commençant pas à 0) — voir le docstring d'`export()` et l'écart consigné
  dans `03-ecarts-conception.md` (retour de revue #75). Pas encore
  bloquant : aucun appelant réel d'`export()` n'existe en dehors des tests.
- `ledger` n'est pas encore câblé sur `store` : la table `ledger` existe
  (migration initiale) mais aucune méthode d'accès — `to_bytes`/`from_bytes`
  prouvent le format, pas l'écriture disque réelle.
- Clé de chiffrement fixe (`FixedKeySource`) — pas de vraie dérivation
  depuis un Keystore/Keychain (voir « Décisions » — dépend d'`identity`,
  US-205).
- CRUD incomplet : `outbox`, `held_envelopes`, `seen_set`, `recon_cache`,
  `ledger`, `ship_cursor` ont leur table créée mais aucune méthode
  d'accès — pas d'appelant avant `sync`.
- Codec : les payloads **par type** (`ANNOUNCE`, `INVENTORY`, `FRAGMENT`…)
  restent opaques ; chaque module consommateur les décodera (US-202 pour
  `FRAGMENT`, `sync::*`).
- **Le TTL n'est pas protégé par la signature** (mis à 0 dans l'entrée de
  signature, revue #80) : un relais malveillant peut le remonter, borné par
  la dédup. Écart vs `powl/03`, voir `03-ecarts-conception.md`.
- Couverture ≥ 85 % non mesurée ni imposée (`cargo llvm-cov` pas encore
  posé) — mais chaque chemin d'erreur de `store` (`Encryption`,
  `Decryption`, `InvalidUtf8`, `Sqlite`) a un test dédié qui l'exerce.
- `no_std` vérifié sur cible hôte seulement ; la vraie cross-compilation
  `xtensa-esp32-none-elf` est l'objet du Spike A (US-101), déjà validé pour
  `protocol`/`crypto` mais pas encore rejoué pour `ledger` spécifiquement.
  `store` n'a jamais vocation à y compiler (voir « Décisions »).
- **`timestamp_ms` des vecteurs `accept` figé à une date fixe (2024-07-29),
  hors tolérance anti-rejeu `TIMESTAMP_TOLERANCE_MS` (±2 h)** — signalé
  hors-diff par Paul (revue PR #63) — **sans objet pour le codec** (US-201 :
  `decode` n'applique pas l'anti-rejeu, c'est `sync::routing`). Constat
  d'origine : un décodeur qui appliquerait l'anti-rejeu
  (US-201) rejetterait les 8 vecteurs `accept` tels quels. Pas corrigé dans
  cette session : la bonne solution (un `reference_now_ms` racine dans
  `vectors_v0.json`, lu par le futur décodeur au lieu de l'horloge système)
  relève du design du codec, pas d'un ajustement de constante — mieux traité
  avec US-201 qui en aura l'usage réel. Idem pour `expect.msg_id` (absent des
  vecteurs — seul endroit où une divergence d'endianness serait visible entre
  Rust/C/Python).
- L'objectif de couverture ≥ 85 % (`10-benchmarks-mvp-tests.md` §4.2) est
  mesuré à la main, pas imposé par la CI.
- `crypto::noise` ne signe pas l'enveloppe, ne compare pas `remote_static` au
  contact (TOFU/vérifié) et ne compte pas les messages pour re-négocier
  (`2^n`) : à faire par `identity` / `sync` / la couche trame.
- Pas de vecteurs Noise officiels (cacophony) : les vecteurs sont maison,
  mais **tout** le fichier (enveloppe `X`, handshake `XX` en 3 messages, 2
  chiffrés de transport) a été recalculé par une implémentation Python
  indépendante — identique octet par octet.
- Handshake relayé sur plusieurs sauts : une perte d'un `NOISE_HS` fait échouer
  le handshake, sans reprise automatique (à gérer par `sync`).
- Le mode `no_std` est vérifié sur cible hôte (CI) et sur `thumbv7em-none-eabi`
  (à la main, 2026-09-28). La cross-compilation `xtensa-esp32-none-elf` n'a été
  prouvée que par le Spike A (US-101), sur une crate jouet, pas sur `crypto`
  (ni sur `snow` tel qu'utilisé ici : toolchain `esp` absente du poste le
  2026-09-28).
- `identity` : aucun coffre plateforme branché (Keystore, Secret Service,
  NVS). La clé du coffre est fournie par l'appelant, et `FileVault` n'est
  qu'un fichier chiffré. `identity` ne décide pas non plus du TOFU :
  comparer une clé à un contact connu et lever `contact.key_changed`
  relève de `store` / `sync`. Enfin, les `ANNOUNCE` ne sont pas encore
  signés.
- `crypto` : pas de génération de clé (c'est `identity::Identity::generate`
  qui la fait), pas de séparation de domaine ; vecteur
  RFC 8032 « TEST 1024 » non repris.

## Sous-module `sync::status` (US-211)

Suit le **cycle de vie d'un message émis** — « En attente → Parti →
Distribué » (+ « Échec », « Annulé ») — et garde les messages non confirmés
dans une **outbox** qui survit au redémarrage. Conception :
[`synthese/07`](../../synthese/07-cycle-de-vie-et-statuts.md) §1-3, §7.
`READ` (« Lu ») est **absent** : reporté en v2 (A-10).

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `Status` (enum) | `src/sync/status.rs:69` | 5 statuts MVP. `rank()` (0 / 1 / 2 = terminal), `as_str()` (= `CHECK` SQL de `synthese/09`), `event_name()` (`msg.queued`…), `to_u8`/`from_u8`. |
| `StatusEvent` (enum) | `src/sync/status.rs:170` | `HandedOff`, `AckReceived(AckStatus)`, `TtlElapsed`, `Cancel`. |
| `fn next_status` | `src/sync/status.rs:186` | **La** fonction de transition, pure. `None` = l'événement ne change rien. |
| `StatusChange` | `src/sync/status.rs:203` | Changement appliqué (`from`, `to`, `at_ms`) à journaliser par l'appelant. |
| `RESEND_MAX = 8` | `src/sync/status.rs:65` | Remises max d'un message à un même pair. |
| `Outbox<S>` | `src/sync/status/outbox.rs:423` | `open` (relit le stockage), `enqueue`, `mark_handed_off`, `apply_ack`, `cancel`, `expire_due`, `replay_candidates`. |
| `trait OutboxStore` | `src/sync/status/outbox.rs:321` | Stockage clé `msg_uuid` → octets : `save`, `remove`, `load_all`. |
| `MemoryStore` | `src/sync/status/outbox.rs:352` | Implémentation en mémoire (tests, sim, bouchon avant US-207). |
| `OutboxRecord::encode` / `decode` | `src/sync/status/outbox.rs:148` / `:190` | Format binaire v1 versionné ; décodage sans panic. |

**Flux (Alice → Bob) :** `enqueue` → `QUEUED` (`msg.queued`). Un relais se
connecte : `replay_candidates(relais)` → envoi radio → `mark_handed_off` →
`IN_FLIGHT` (`msg.handed_off`). Crash puis redémarrage : `Outbox::open`
relit le stockage, le message est toujours `IN_FLIGHT` avec son compteur de
remises. L'Ack signé de Bob arrive → `apply_ack` → `DELIVERED`
(`msg.delivered`), le message sort de l'outbox. Sans Ack en 24 h :
`expire_due` → `EXPIRED`.

**Décisions :** stockage écrit **avant** la mémoire (un échec laisse l'état
intact) ; table des remises bornée à `ATTEMPT_PEERS_MAX = 32` pairs ;
paquet borné à `PACKET_MAX_BYTES` ; le filtre « destinataire ou bon relais »
reste à `sync::routing` (US-209). Écarts : `03-ecarts-conception.md`
(2026-09-28, US-211). Dépendance de dev ajoutée : `proptest`.

**Tests :** `src/sync/status.rs` (9 tests dont le property test
`aucune_regression_d_etat`) et `src/sync/status/outbox/tests.rs` (24 tests :
transitions, rejeu plafonné, expiration à l'échéance stricte, **rejeu après
redémarrage**, stockage défaillant, format d'enregistrement, et 2 property
tests : aller-retour de l'encodage + monotonie de l'outbox sur des suites
d'opérations aléatoires, redémarrages compris). Couverture : 100 % des
lignes (`cargo llvm-cov -p dengon-core`).

**Limites :** pas encore appelé (branchement `api`/FFI/sim à venir) ; pas
d'implémentation SQLite d'`OutboxStore` (après US-207) ; un `msg_uuid` déjà
terminé puis ré-enqueué repartirait en `QUEUED` (précondition documentée :
`msg_uuid` aléatoire sur 128 bits, jamais réutilisé).

- **`sync::routing`** : la signature des paquets est supposée vérifiée
  **en amont** (pas de `crypto` sur `main`) ; le décodage de
  `tests/routing_mock.rs` est un **codec de test provisoire** (sans
  signature), à remplacer par `protocol::codec` (US-201) ; le routeur ne
  voit pas les ACK (chiffrés dans Noise) — c'est `status`/`courier` qui
  appelleront `Router::cancel`. Pas de RSSI-gating (optionnel MVP).
  Pas encore branché dans `dengon-node` ni `dengon-sim`.

## Pour l'oral

Trois livrables dans cette crate à ce stade. US-108 fige le **vocabulaire du
protocole** : les 13 types de paquets, les 5 drapeaux, la forme de l'en-tête,
et une trentaine de constantes (durée de vie d'un message, seuils
d'anti-inondation, TTL de départ…) — rien ne « fonctionne » encore, mais
c'est le contrat sur lequel les trois implémentations (téléphone, nœud,
firmware) vont s'accorder, d'où les **vecteurs de conformité**. US-206
(`ledger`) est la première **logique exécutable** de la crate : un journal
chaîné append-only, vérifiable hors ligne, qui prouve qu'un appareil n'a pas
triché sur son historique — la pièce qui rend crédible « sécurité type
blockchain » sans blockchain.
US-207 (`store`) est la première **persistance réelle** : SQLite avec
chiffrement champ par champ des données sensibles (clés privées, corps des
messages, sessions Noise) — la preuve qu'un téléphone volé ne livre rien
en clair, critère central de `docs/synthese/06-securite.md`.

US-205 donne son **identité** à chaque appareil : deux paires de clés, dont
on tire un identifiant court (`peerID`). L'identité est rangée chiffrée dans
un coffre, ce qui la garde stable d'un redémarrage à l'autre. Pour ajouter
un contact, on scanne son QR (pseudo + clés publiques, aucun secret). Pour
s'assurer que personne ne s'est glissé au milieu de l'échange, les deux
téléphones affichent le même code de 60 chiffres, que l'on compare de visu.
Anecdote utile à l'oral : la formule du document de conception ne pouvait
jamais produire un groupe au-dessus de 65535. On l'a vu, corrigée, et
consignée.

US-209 (`sync::routing`) est le **cœur du maillage** : pour chaque paquet
reçu, décider si on le relaie, avec quel TTL, après quel délai, et à qui.
Message clé : sous un flot de 500 messages/seconde venant d'un voisin, un
nœud ne relaie **que 20 messages par minute** de ce voisin, et le trafic
honnête des autres passe toujours. Et une vraie trouvaille de conception : la
règle « abandonner au premier doublon » de la doc empêchait la livraison dans
un simple losange — mesuré, corrigé, documenté.
