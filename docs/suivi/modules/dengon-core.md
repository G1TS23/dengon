# Module : `dengon-core` (`crates/dengon-core/`)

**Rôle en une phrase :** la bibliothèque qui contient **tout le protocole** dengon, sans aucune entrée/sortie.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §2 et §5 (décision A-2).
**Dernière mise à jour :** 2026-09-26
**État :** esquisse, `store` livré (US-207)

## À quoi ça sert

C'est le cœur du projet, et la raison pour laquelle il n'y a **qu'une seule**
implémentation du protocole : la même bibliothèque est utilisée par
l'application Android (via `dengon-ffi`), par le nœud en ligne de commande
`dengon-node` et par le firmware de l'ESP32. Écrire le protocole trois fois,
c'est garantir trois comportements légèrement différents et un audit de sécurité
trois fois plus cher.

La règle qui rend ce partage possible : `dengon-core` ne parle **ni à la radio
ni au réseau**. Il produit et consomme des `Vec<u8>`, que quelqu'un d'autre se
charge de transporter (le trait `Transport` de `dengon-ble`).

## Structure

```
dengon-core/
  src/
    lib.rs        — bascule no_std, PROTOCOL_VERSION, VERSION, pub mod store (feature std)
    store.rs      — persistance SQLite, chiffrement champ par champ (US-207)
```

Les modules restant à écrire (`protocol`, `crypto`, `identity`, `sync`,
`ledger`, `observability`, `api`) **n'existent pas encore**.

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `PROTOCOL_VERSION: u8` | `src/lib.rs` | Version du protocole ; correspond au champ `version` de chaque paquet, qui permet la négociation à l'ANNOUNCE. |
| `VERSION: &str` | `src/lib.rs` | Version de la crate, lue dans `Cargo.toml` à la compilation. |
| `store::Store<K: KeySource>` | `src/store.rs` | Connexion SQLite + migrations. `open()`/`open_in_memory()`, puis `set_identity`/`get_identity_private_keys`, `upsert_contact`, `insert_conversation`, `insert_message`/`get_message_body`, `set_noise_session`/`get_noise_session_state`. |
| `store::KeySource` / `store::FixedKeySource` | `src/store.rs` | Trait qui fournit la clé de chiffrement des champs sensibles + bouchon à clé fixe (tests uniquement) — voir « Décisions », même schéma que `ledger::Signer` (US-206). |
| `store::encrypt_field`/`decrypt_field` (privées) | `src/store.rs` | XChaCha20-Poly1305, nonce aléatoire de 24 o préfixé au résultat stocké. |

## Flux principal (exemple)

```
Store::open("dengon.db", key_source)
  → run_migrations() (schema_migrations, IF NOT EXISTS, une transaction par migration)
  → upsert_contact(peer_id, ...) → insert_conversation(conv_id, peer_id)
  → insert_message(msg_uuid, conv_id, ..., body="salut", ...)
       body chiffré (XChaCha20-Poly1305) AVANT le INSERT — jamais en clair sur disque
  → get_message_body(msg_uuid) → déchiffre, renvoie "salut"
```

Le flux applicatif complet (Alice écrit → chiffrement → trame → diffusion BLE
→ relais → Bob → accusé) reste décrit dans
[`04-architecture.md`](../../synthese/04-architecture.md) §4 — `store` n'en
est que la persistance locale, pas le protocole lui-même.

## Dépendances

- **Internes :** aucune. C'est la racine du graphe — les cinq autres crates
  dépendent d'elle, elle ne dépend de personne. C'est volontaire : une
  dépendance sortante de `dengon-core` serait une dépendance imposée au
  firmware ESP32.
- **Externes (crates), `store` seulement (feature `std`) :** `rusqlite`
  (`features = ["bundled"]` — sqlite3 vendorisé en C, pas de dépendance
  système) et `chacha20poly1305` (`features = ["getrandom"]`, pour
  `aead::OsRng`). Viendront encore `ed25519-dalek`, `snow`, `x25519-dalek`,
  `serde`.

## Décisions d'implémentation

- **Bascule `no_std`** : `#![cfg_attr(not(feature = "std"), no_std)]` en
  `src/lib.rs`, avec une feature `std` activée par défaut. Retirer la feature
  active `#![no_std]` **sur la cible hôte**, ce qui suffit à détecter tout usage
  involontaire de `std` sans avoir à installer une cible bare metal. La CI le
  vérifie à chaque PR (`cargo check -p dengon-core --no-default-features`).
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

## Tests

- `src/lib.rs`, module `tests` : deux tests fumigènes (version de crate,
  `PROTOCOL_VERSION`).
- `src/store.rs`, module `tests` : 9 tests — migrations rejouables sans
  erreur, round-trip identité/message/session Noise (chiffré puis
  déchiffré, on retrouve le texte d'origine), deux chiffrements du même
  texte donnent des octets différents (nonce aléatoire), déchiffrer avec la
  mauvaise clé échoue, déchiffrer une donnée modifiée échoue (garantie
  d'authenticité Poly1305), déchiffrer un buffer tronqué échoue sans
  paniquer, et le test central du critère d'acceptation : **écrire un
  message connu sur un vrai fichier `.db`, puis `grep` binaire sur le
  fichier — le texte en clair n'y est pas**.
- Commande : `cargo test -p dengon-core` → 11 passés, 0 échec (vérifié le
  2026-09-26). `cargo clippy --workspace --all-targets --all-features -- -D
  warnings` et `cargo fmt --all -- --check` verts. `cargo check -p
  dengon-core --no-default-features --locked` (frontière `no_std`) vert —
  `store` est bien absent de cette configuration (feature `std` retirée).

## Limites connues / TODO

- Clé de chiffrement fixe (`FixedKeySource`) — pas de vraie dérivation
  depuis un Keystore/Keychain (voir « Décisions » — dépend d'`identity`,
  US-205).
- CRUD incomplet : `outbox`, `held_envelopes`, `seen_set`, `recon_cache`,
  `ledger`, `ship_cursor` ont leur table créée mais aucune méthode
  d'accès — pas d'appelant avant `sync`/`ledger`.
- Couverture de ligne non mesurée par un outil dédié (`cargo llvm-cov` pas
  encore posé ce sprint), mais chaque chemin d'erreur (`Encryption`,
  `Decryption`, `InvalidUtf8`, `Sqlite`) a un test dédié qui l'exerce.
- Le mode `no_std` est vérifié sur cible hôte seulement pour le reste de la
  crate ; `store` lui-même n'a jamais vocation à y compiler (voir
  « Décisions »).

## Pour l'oral

C'est la pièce centrale : une seule bibliothèque écrite en Rust contient tout le
protocole, et elle tourne aussi bien dans un téléphone Android que dans un petit
microcontrôleur à 5 €. Le point intéressant, c'est la contrainte qu'on s'impose :
cette bibliothèque n'a **pas le droit** de parler à la radio. Ça paraît absurde,
mais c'est exactement ce qui permet de la faire tourner partout — et de n'avoir
qu'un seul endroit à auditer côté sécurité.
