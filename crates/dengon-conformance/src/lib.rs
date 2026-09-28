//! Conformité aux vecteurs partagés, côté **sans `std`** (US-222).
//!
//! Cette crate n'a **pas de code** : toute sa raison d'être est sa table de
//! dépendances (`Cargo.toml`), qui tire `dengon-core` en
//! `default-features = false` — la configuration que le firmware ESP32
//! embarquera via `dengon_core_ffi` (US-307). Les vecteurs de
//! `contracts/packet/` sont rejoués contre ce build-là dans
//! `tests/packet_vectors_nostd.rs`.
//!
//! Le harnais de test, lui, est `std` (c'est `libtest`) : ce qui est prouvé
//! ici, c'est que **la bibliothèque testée** se passe de `std`, pas le test.
//!
//! # Ce qui garantit que le build est vraiment `no_std`
//!
//! Rien dans ce fichier — et c'est voulu. La configuration ne tient que dans
//! la **résolution isolée** du paquet :
//!
//! ```text
//! cargo test -p dengon-conformance          # dengon-core sans std
//! cargo test --workspace                    # dengon-core AVEC std (unifié)
//! ```
//!
//! Un `cargo build --workspace` unifie les features de tout le graphe, et
//! `std` revient par `dengon-node`/`dengon-ble`. Une assertion « je suis
//! sans std » compilée dans cette crate ferait donc échouer `core.yml`, à
//! tort. Le garde-fou est à sa place dans le job `cross-vectors`, qui
//! inspecte la résolution isolée depuis l'extérieur :
//!
//! ```text
//! cargo tree -p dengon-conformance -e features | grep rusqlite   # doit être vide
//! ```
//!
//! Voir `contracts/packet/README.md` et l'entrée « proxy `no_std` » de
//! `docs/suivi/03-ecarts-conception.md`.

#![no_std]
