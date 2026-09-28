# Module : `dengon-core` (`crates/dengon-core/`)

**Rôle en une phrase :** la bibliothèque qui contient **tout le protocole** dengon, sans aucune entrée/sortie.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §2 et §5 (décision A-2) ; [`docs/synthese/05-protocole-et-trame.md`](../../synthese/05-protocole-et-trame.md) (format de trame).
**Dernière mise à jour :** 2026-09-28
**État :** esquisse — squelette (US-104) + `protocol::{consts, types}` (US-108) + `ledger` (US-206) + `store` (US-207) + `crypto` Ed25519 (US-203).

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
    crypto.rs           — Ed25519 sign/verify (US-203)
    ledger.rs           — journal chaîné append-only (US-206)
    store.rs            — persistance SQLite, chiffrement champ par champ (US-207)
    protocol/
      mod.rs           — re-exports du module protocol
      consts.rs        — TOUTES les constantes du protocole (synthese/05 §2)
      types.rs         — PacketType, Flags, Header, AppFrameKind, AckStatus,
                         alias PeerId / MsgId / Signature
  tests/
    vectors_v0.json    — vecteurs de conformité v0 (bytes -> champs / rejet)
    protocol_vectors.rs — contrôle structurel de ces vecteurs
```

Modules encore absents : `codec` (US-201), `identity`, `sync`,
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
| `store::encrypt_field`/`decrypt_field` (privées) | `src/store.rs` | XChaCha20-Poly1305, nonce aléatoire de 24 o préfixé au résultat stocké, AAD liée au contexte de ligne/colonne. |
| `crypto::SigningKey` | `src/crypto.rs` | Clé privée Ed25519, construite depuis une graine de 32 octets (`from_seed`). `sign` est déterministe. `Debug` masque le secret. |
| `crypto::VerifyingKey` | `src/crypto.rs` | Clé publique. `from_bytes` rejette un point invalide ; `verify` utilise `verify_strict`. |
| `crypto::CryptoError` | `src/crypto.rs` | `InvalidPublicKey` / `InvalidSignature` — on ne distingue pas les causes d'un échec de `verify`. |

## Flux principal (exemple)

Pas encore de flux protocole complet : US-108 livre les **types**, pas la
logique. `sync::routing` (US-209) s'écrira contre `PacketType` / `Flags` /
`Header` sans attendre le codec (US-201). Le flux visé :
`04-architecture.md` §4.

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
`store`, lui, a déjà un flux exécutable :

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

## Dépendances

- **Internes :** aucune. C'est la racine du graphe — les cinq autres crates
  dépendent d'elle, elle ne dépend de personne. C'est volontaire : une
  dépendance sortante de `dengon-core` serait une dépendance imposée au
  firmware ESP32.
- **Externes (runtime) :** `sha2` (`default-features = false`, no_std) pour
  `ledger` ; `ed25519-dalek` 2.2 (`default-features = false`, feature
  `zeroize`) pour `crypto`. `protocol` n'utilise que `core`.
- **Externes (crates), `store` seulement (feature `std`) :** `rusqlite`
  (`features = ["bundled"]` — sqlite3 vendorisé en C, pas de dépendance
  système) et `chacha20poly1305` (`features = ["getrandom"]`, pour
  `aead::OsRng`). Viendront encore `snow`, `x25519-dalek`, `serde`.
- **Externes (dev) :** `proptest` (property tests de `ledger`) ; `serde_json`
  — lecture de `tests/vectors_v0.json` via `Value` (pas de derive, donc
  `serde` n'est pas tiré comme dépendance de proc-macro). N'affecte pas la
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

## Tests

- `src/lib.rs`, module `tests` : deux tests fumigènes (version de crate,
  `PROTOCOL_VERSION`).
- `src/crypto.rs`, module `tests` : 15 tests — 3 KAT RFC 8032 §7.1 sur 4 vecteurs
  (clé publique, signature octet à octet, `verify`), aller-retour, déterminisme,
  négatifs (forgée, bit-flip exhaustif message + signature, tronqué, mauvaise
  clé, clé invalide, faible ordre).
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
  #63, round 3 : garantit que l'invariant « `ttl > 1` ⇒ `RELAY_OK` »
  (`synthese/05:203`) reste vrai vecteur par vecteur — y compris pour des
  paquets **adressés** comme `ack-addressed`, pas seulement broadcast,
  contrairement à une formulation précédente de cette fiche — pas
  seulement pour les deux corrigés au round 2).
- Commande : `cargo test -p dengon-core` → **46 passés** (42 lib + 4
  intégration + 0 doc — 16 pour `ledger`, 13 pour `store`, 13 pour
  `protocol::{consts,types}` et `lib.rs`), rejoué le
  2026-09-28 après intégration de `ledger` (#75) et `store` (#76). `cargo
  clippy --workspace --all-targets --all-features -- -D warnings` et `cargo
  fmt --all -- --check` verts. `cargo check -p dengon-core
  --no-default-features` (frontière `no_std`) vert.
- Négatif vérifié en local : la garde de longueur `hdr + 2` réintroduite
  temporairement fait échouer `accept_vectors_are_structurally_consistent`
  sur le nouveau vecteur `noise-msg-addressed-reserved-bit-ignored` (30
  octets, exactement `hdr`) — confirme le « mirror bug » signalé par Paul
  (un paquet valide à `payload_len` faible rejeté à tort).
- Couverture non mesurée par un outil (`cargo llvm-cov` pas encore posé dans
  ce sprint) — objectif ≥ 85 % de la DoD §7.2 non vérifié formellement,
  mais chaque branche de `verify_chain` (Ok/Broken/Fork/Gap) a un test
  dédié qui l'exerce explicitement.

## Limites connues / TODO

- `crypto` : pas de génération de clé, pas de séparation de domaine ; vecteur
  RFC 8032 « TEST 1024 » non repris. Cross-compilation xtensa prouvée par le
  Spike A sur une crate jouet, pas sur `crypto` (vérifié à la main sur
  `thumbv7em-none-eabi`, 2026-09-28).
- `verify_chain()` ne vérifie pas la signature (voir « Décisions » —
  dépend de `crypto`, US-203).
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
- **Pas de codec** : aucun `encode`/`decode` — c'est US-201. Le test des
  vecteurs est donc *structurel* (pas « `decode(bytes) == expect` »).
- Pas de property test sur `protocol` (la DoD §7.2 en exigera dès qu'il y
  aura de la logique de sérialisation — `ledger`, lui, en a déjà).
- Couverture ≥ 85 % non mesurée ni imposée (`cargo llvm-cov` pas encore
  posé) — mais chaque chemin d'erreur de `store` (`Encryption`,
  `Decryption`, `InvalidUtf8`, `Sqlite`) a un test dédié qui l'exerce.
- `no_std` vérifié sur cible hôte seulement ; la vraie cross-compilation
  `xtensa-esp32-none-elf` est l'objet du Spike A (US-101), déjà validé pour
  `protocol`/`crypto` mais pas encore rejoué pour `ledger` spécifiquement.
  `store` n'a jamais vocation à y compiler (voir « Décisions »).
- **`timestamp_ms` des vecteurs `accept` figé à une date fixe (2024-07-29),
  hors tolérance anti-rejeu `TIMESTAMP_TOLERANCE_MS` (±2 h)** — signalé
  hors-diff par Paul (revue PR #63) : un décodeur qui appliquerait l'anti-rejeu
  (US-201) rejetterait les 8 vecteurs `accept` tels quels. Pas corrigé dans
  cette session : la bonne solution (un `reference_now_ms` racine dans
  `vectors_v0.json`, lu par le futur décodeur au lieu de l'horloge système)
  relève du design du codec, pas d'un ajustement de constante — mieux traité
  avec US-201 qui en aura l'usage réel. Idem pour `expect.msg_id` (absent des
  vecteurs — seul endroit où une divergence d'endianness serait visible entre
  Rust/C/Python).

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
