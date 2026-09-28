//! `dengon-core-embed` — assemble `libdengon_core.a`, l'archive **autonome**
//! liable dans un exécutable C (US-307).
//!
//! Ne fait que deux choses : réexporter les fonctions C de
//! [`dengon_core_ffi`] (dont `#[no_mangle]` préserve le nom de symbole quel
//! que soit le crate qui les assemble en `.a`), et fournir les deux éléments
//! qu'un artefact `staticlib` doit résoudre lui-même — l'allocateur global et
//! le panic handler — puisque `libdengon_core.a` est liée dans un exécutable
//! où **aucun autre code Rust** ne les fournit : ni le C hôte des tests
//! (`tests/c/host_test.c`), ni, à terme, le firmware ESP-IDF (C pur).
//!
//! # Pourquoi une crate séparée de `dengon-core-ffi`
//!
//! `dengon-core-ffi` reste un `rlib` normal, testé par `cargo test` comme le
//! reste du workspace : son harnais de test est `std`, qui fournit déjà un
//! allocateur global et un panic handler. Les redéfinir dans la MÊME crate
//! (via une feature Cargo, essayé en premier en écrivant cette US) entre en
//! conflit dès qu'une commande unifie les features de tout le graphe —
//! notamment `cargo clippy --workspace --all-targets --all-features`, qui
//! active la feature pour le build ET pour les tests dans la même
//! compilation (`error[E0152]: found duplicate lang item 'panic_impl'`).
//! Une crate à part, sans aucun test (`test = false`, voir `Cargo.toml`),
//! n'est jamais mêlée à un binaire `std` : le problème ne se pose plus.
//!
//! # Choix : `malloc`/`free`/`abort` de la libc, pas un allocateur maison
//!
//! Ces trois symboles existent aussi bien sous glibc/MSVC (tests hôte) que
//! dans le newlib d'ESP-IDF (cible réelle) : le même code sert aux deux sans
//! `#[cfg(target_os = ...)]`. C'est un choix minimal pour cette US — pas
//! l'allocateur `heap_caps_*` idiomatique d'ESP-IDF (SPIRAM, capacités
//! DMA...), qui reste à évaluer en US-308/309 si la taille de tas ou la
//! fragmentation l'exigent. Consigné comme écart mineur dans
//! `docs/suivi/03-ecarts-conception.md`.
//!
//! # Ce qui reste ouvert (Spike A, §5)
//!
//! Le spike avait signalé : « la cohabitation des allocateurs et du
//! `panic_handler` reste à valider [en US-307] ». Cette implémentation
//! répond à la question de PRINCIPE (une seule source de vérité, choisie
//! ici) ; elle n'a été exercée que par le C hôte et par la cross-compilation
//! xtensa (édition de liens), **pas sur matériel** — aucune allocation ni
//! panique réelle n'a été observée tourner sur une carte ESP32.

#![no_std]
#![allow(unsafe_code)]

pub use dengon_core_ffi::*;

use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;
use core::panic::PanicInfo;

extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
    fn abort() -> !;
}

struct LibcAlloc;

// SAFETY : `malloc`/`free` de la libc forment une paire valide de
// (dés)allocation générale ; `dengon-core` n'exige aucun alignement
// supérieur à celui garanti par `malloc` (`max_align_t`, ≥ 8 octets sur
// toutes les cibles visées ici).
unsafe impl GlobalAlloc for LibcAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY : `layout.size()` est un `usize` valide ; `malloc` renvoie
        // soit un pointeur valide pour `layout.size()` octets, soit NULL
        // (que `GlobalAlloc` autorise explicitement comme signal d'échec).
        unsafe { malloc(layout.size()).cast() }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        // SAFETY : `ptr` provient d'un appel précédent à `alloc` sur cette
        // même instance (contrat de `GlobalAlloc`), donc de `malloc`.
        unsafe { free(ptr.cast()) }
    }
}

#[global_allocator]
static ALLOCATOR: LibcAlloc = LibcAlloc;

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    // Pas de message : ni `stderr` (no_std) ni logs ESP-IDF (hors périmètre
    // ici). `dengon_core::protocol::decode` ne panique sur aucune entrée
    // (c'est son contrat), donc ce chemin n'est atteint que par un bug ou un
    // `unwrap`/`expect` — `abort()` évite un comportement indéfini plutôt que
    // de tenter un déroulement de pile qu'un binaire `no_std` ne supporte pas.
    // SAFETY : `abort()` de la libc ne revient jamais, cohérent avec `-> !`.
    unsafe { abort() }
}
