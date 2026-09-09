# Module : `dengon-ffi` (`crates/dengon-ffi/`)

**Rôle en une phrase :** le pont qui permet à l'application Android, écrite en Kotlin, d'appeler le cœur écrit en Rust.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §3 et §5.
**Dernière mise à jour :** 2026-09-09
**État :** esquisse

## À quoi ça sert

L'application Android est en Kotlin, le protocole est en Rust. Écrire à la main
le code d'interfaçage entre les deux (JNI) est long et une source classique de
plantages. **UniFFI** génère ce code automatiquement à partir d'une description
d'interface : on décrit les fonctions exposées, l'outil produit la bibliothèque
native et les classes Kotlin correspondantes.

## Structure

```
dengon-ffi/
  src/
    lib.rs        — pour l'instant : une fonction version()
```

Le fichier UDL (description d'interface) et le `build.rs` **n'existent pas
encore** : c'est US-106.

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `fn version() -> String` | `src/lib.rs:22` | Renvoie la version de la crate et du protocole. Sert de première fonction exportable. |

## Flux principal (exemple)

Visé : Kotlin appelle `DengonCore.envoyerMessage(...)` → UniFFI traduit →
`dengon-ffi` → `dengon-core::api`.

Actuel : aucun binding n'est généré.

## Dépendances

- **Internes :** `dengon-core`.
- **Externes (crates) :** aucune. `uniffi` arrivera avec US-106.

## Décisions d'implémentation

- **`crate-type = ["lib", "cdylib"]`** (`Cargo.toml:17`). `cdylib` produit le
  `libdengon_ffi.so` que l'application Android charge. `lib` est indispensable
  malgré les apparences : sans lui, `cargo test` ne peut pas compiler les tests
  unitaires de la crate.
- **Le lint `unsafe_code` est en `deny`, jamais en `forbid`**, précisément à
  cause de cette crate : UniFFI génère du code `unsafe`, et il faudra le
  neutraliser ici par un `#![allow(unsafe_code)]`. `forbid` serait inviolable et
  rendrait la crate impossible à écrire. C'est la raison du choix fait dans
  `[workspace.lints.rust]`.
- `staticlib` n'est pas déclaré : il ne servira que le jour où le firmware ESP32
  aura besoin d'une `libdengon_core.a`.

## Tests

- `src/lib.rs`, module `tests` : 1 test (la version expose le numéro de protocole).
- Commande : `cargo test -p dengon-ffi` → 1 passé, 0 échec.

## Limites connues / TODO

- Aucune surface FFI réelle. Pas de fichier UDL, pas de génération Kotlin, pas
  de `build.rs`.
- La cross-compilation vers les ABI Android (`cargo-ndk`, `jniLibs`) n'est
  documentée nulle part dans la conception, et n'a pas été essayée.

## Pour l'oral

C'est le point de rencontre entre deux mondes. L'application se voit en Kotlin,
comme n'importe quelle application Android ; mais dès qu'il s'agit de chiffrer
ou de router un message, elle appelle du Rust. Cette crate est le traducteur
entre les deux, et l'essentiel de son code sera **généré** plutôt qu'écrit — ce
qui évite toute une famille de bugs difficiles.
