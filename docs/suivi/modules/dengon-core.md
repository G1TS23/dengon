# Module : `dengon-core` (`crates/dengon-core/`)

**Rôle en une phrase :** la bibliothèque qui contient **tout le protocole** dengon, sans aucune entrée/sortie.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §2 et §5 (décision A-2).
**Dernière mise à jour :** 2026-09-26
**État :** esquisse, `ledger` livré (US-206)

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
    lib.rs        — bascule no_std, PROTOCOL_VERSION, VERSION, extern crate alloc
    ledger.rs     — journal chaîné append-only (US-206)
```

Les modules restant à écrire (`protocol`, `crypto`, `identity`, `store`,
`sync`, `observability`, `api`) **n'existent pas encore**.

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `PROTOCOL_VERSION: u8` | `src/lib.rs` | Version du protocole ; correspond au champ `version` de chaque paquet, qui permet la négociation à l'ANNOUNCE. |
| `VERSION: &str` | `src/lib.rs` | Version de la crate, lue dans `Cargo.toml` à la compilation. |
| `ledger::Entry` | `src/ledger.rs` | Une entrée du journal : `seq`, `ts_ms`, `event_name`, `payload_json`, `prev_hash`, `entry_hash`, `sig`. `entry_hash = SHA-256(seq‖ts_ms‖len‖event_name‖len‖payload_json‖prev_hash)`, longueurs préfixées pour lever toute ambiguïté de découpage. |
| `ledger::Ledger<S: Signer>` | `src/ledger.rs` | Le journal lui-même : `append()`, `verify_chain()`, `export(range)`, `entries()`. Paramétré par un `Signer` injecté, pas câblé sur une implémentation Ed25519 concrète. |
| `ledger::Signer` / `ledger::NullSigner` | `src/ledger.rs` | Trait de signature + bouchon nul, tant que `crypto` (US-203, même sprint) n'existe pas — voir « Décisions ». |
| `ledger::Verdict` | `src/ledger.rs` | `Ok`/`Broken`/`Fork`/`Gap`, renvoyé par `verify_chain()`. Réutilisé tel quel par `dengon-verify::main`, qui n'en a plus de copie locale. |
| `Entry::to_bytes`/`Entry::from_bytes` | `src/ledger.rs` | Sérialisation binaire simple d'une entrée — sert le test de reprise après redémarrage, pas un vrai backend de stockage (voir « Décisions »). |

## Flux principal (exemple)

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

Le flux applicatif complet (Alice écrit → chiffrement → trame → diffusion BLE
→ relais → Bob → accusé) reste décrit dans
[`04-architecture.md`](../../synthese/04-architecture.md) §4 — `ledger` n'en
est qu'un maillon (la traçabilité), pas le chemin des messages.

## Dépendances

- **Internes :** aucune. C'est la racine du graphe — les cinq autres crates
  dépendent d'elle, elle ne dépend de personne. C'est volontaire : une
  dépendance sortante de `dengon-core` serait une dépendance imposée au
  firmware ESP32.
- **Externes (crates) :** `sha2` (`default-features = false`, no_std) pour
  `ledger`. En dev-dependency : `proptest`. Viendront encore
  `ed25519-dalek`, `snow`, `x25519-dalek`, `chacha20poly1305`, `rusqlite`,
  `serde`.

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

## Tests

- `src/lib.rs`, module `tests` : deux tests fumigènes (version de crate,
  `PROTOCOL_VERSION`).
- `src/ledger.rs`, module `tests` : 12 tests unitaires (chaîne vide valide,
  append→verify_chain toujours Ok, seq consécutives, trou détecté, doublon
  de seq détecté comme fork, entrée modifiée détectée comme broken,
  `prev_hash` incohérent détecté, export par plage, reprise après
  redémarrage par sérialisation, désérialisation d'un buffer tronqué sans
  panique) + **2 property tests** (`proptest`) : toute séquence d'appends
  reste vérifiable ; corrompre n'importe quelle entrée d'une séquence
  quelconque est toujours détecté comme `Broken`.
- Commande : `cargo test -p dengon-core` → 14 passés, 0 échec (vérifié le
  2026-09-26). `cargo clippy --workspace --all-targets --all-features -- -D
  warnings` et `cargo fmt --all -- --check` verts. `cargo check -p
  dengon-core --no-default-features --locked` (frontière `no_std`) vert.
- Couverture non mesurée par un outil (`cargo llvm-cov` pas encore posé dans
  ce sprint) — objectif ≥ 85 % de la DoD §7.2 non vérifié formellement,
  mais chaque branche de `verify_chain` (Ok/Broken/Fork/Gap) a un test
  dédié qui l'exerce explicitement.

## Limites connues / TODO

- `verify_chain()` ne vérifie pas la signature (voir « Décisions » —
  dépend de `crypto`, US-203).
- Pas de vrai backend de persistance câblé (SQLite/littlefs) — `to_bytes`/
  `from_bytes` prouvent le format, pas l'écriture disque réelle.
- Couverture de ligne non mesurée par un outil dédié (voir « Tests »).
- Le mode `no_std` est vérifié sur cible hôte seulement ; la vraie
  cross-compilation `xtensa-esp32-none-elf` est l'objet du Spike A (US-101),
  déjà validé pour `protocol`/`crypto` mais pas encore rejoué pour `ledger`
  spécifiquement.

## Pour l'oral

C'est la pièce centrale : une seule bibliothèque écrite en Rust contient tout le
protocole, et elle tourne aussi bien dans un téléphone Android que dans un petit
microcontrôleur à 5 €. Le point intéressant, c'est la contrainte qu'on s'impose :
cette bibliothèque n'a **pas le droit** de parler à la radio. Ça paraît absurde,
mais c'est exactement ce qui permet de la faire tourner partout — et de n'avoir
qu'un seul endroit à auditer côté sécurité.
