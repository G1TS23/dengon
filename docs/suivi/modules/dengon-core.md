# Module : `dengon-core` (`crates/dengon-core/`)

**Rôle en une phrase :** la bibliothèque qui contient **tout le protocole** dengon, sans aucune entrée/sortie.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §2 et §5 (décision A-2).
**Dernière mise à jour :** 2026-09-09
**État :** esquisse

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
    lib.rs        — pour l'instant : la bascule no_std, PROTOCOL_VERSION, VERSION
```

Les modules prévus (`protocol`, `crypto`, `identity`, `store`, `sync`, `ledger`,
`observability`, `api`) **n'existent pas encore**.

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `PROTOCOL_VERSION: u8` | `src/lib.rs:31` | Version du protocole ; correspond au champ `version` de chaque paquet, qui permet la négociation à l'ANNOUNCE. |
| `VERSION: &str` | `src/lib.rs:34` | Version de la crate, lue dans `Cargo.toml` à la compilation. |

## Flux principal (exemple)

Aucun : la crate ne fait rien. Le flux visé est décrit dans
[`04-architecture.md`](../../synthese/04-architecture.md) §4 (Alice écrit →
chiffrement → trame → diffusion BLE → relais → Bob → accusé).

## Dépendances

- **Internes :** aucune. C'est la racine du graphe — les cinq autres crates
  dépendent d'elle, elle ne dépend de personne. C'est volontaire : une
  dépendance sortante de `dengon-core` serait une dépendance imposée au
  firmware ESP32.
- **Externes (crates) :** aucune pour l'instant. Viendront `ed25519-dalek`,
  `snow`, `x25519-dalek`, `chacha20poly1305`, `sha2`, `rusqlite`, `serde`.

## Décisions d'implémentation

- **Bascule `no_std`** : `#![cfg_attr(not(feature = "std"), no_std)]` en
  `src/lib.rs:24`, avec une feature `std` activée par défaut. Retirer la feature
  active `#![no_std]` **sur la cible hôte**, ce qui suffit à détecter tout usage
  involontaire de `std` sans avoir à installer une cible bare metal. La CI le
  vérifie à chaque PR (`cargo check -p dengon-core --no-default-features`).
- **Pas de `extern crate alloc;`** tant qu'`alloc` n'est pas utilisé : le groupe
  de lints `rust_2018_idioms` contient `unused_extern_crates`, qui deviendrait
  une erreur sous `-D warnings`.

## Tests

- `src/lib.rs`, module `tests` : deux tests fumigènes (la version de crate est
  renseignée, `PROTOCOL_VERSION` vaut 1).
- Commande : `cargo test -p dengon-core` → 2 passés, 0 échec.
- Il n'y a **aucun** property test pour l'instant, alors que la DoD §7.2 en
  exigera dès qu'il y aura de la logique.

## Limites connues / TODO

- La crate ne fait littéralement rien d'autre qu'exposer deux constantes.
- L'objectif de couverture ≥ 85 % (`10-benchmarks-mvp-tests.md` §4.2) n'est pas
  encore mesuré ni imposé.
- Le mode `no_std` est vérifié sur cible hôte seulement ; la vraie
  cross-compilation `xtensa-esp32-none-elf` est l'objet du Spike A (US-101).

## Pour l'oral

C'est la pièce centrale : une seule bibliothèque écrite en Rust contient tout le
protocole, et elle tourne aussi bien dans un téléphone Android que dans un petit
microcontrôleur à 5 €. Le point intéressant, c'est la contrainte qu'on s'impose :
cette bibliothèque n'a **pas le droit** de parler à la radio. Ça paraît absurde,
mais c'est exactement ce qui permet de la faire tourner partout — et de n'avoir
qu'un seul endroit à auditer côté sécurité.
