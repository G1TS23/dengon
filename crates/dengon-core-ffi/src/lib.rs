//! `dengon-core-ffi` — fonctions C exportées de `dengon-core`, compilées en
//! `no_std` + `alloc` pour le firmware ESP32 (US-307).
//!
//! Cette crate déclare les fonctions et le header `cbindgen` associé
//! (`include/dengon_core.h`, régénéré par `build.rs` et **versionné**), mais
//! ne produit PAS elle-même l'archive `.a` liable : elle reste volontairement
//! un `rlib` normal, testable par `cargo test` comme n'importe quelle autre
//! crate du workspace. L'archive `libdengon_core.a` (autonome, avec son
//! allocateur global et son panic handler) est assemblée par la crate sœur
//! [`dengon-core-embed`](../dengon_core_embed/index.html), qui réexporte ces
//! fonctions — voir sa documentation pour pourquoi ce n'est pas la même
//! crate.
//!
//! # Périmètre de cette US
//!
//! US-307 est la tête du chemin critique firmware
//! (US-101 → US-307 → US-308 → US-309 → US-312) : elle prouve que la chaîne
//! de compilation et le pont C fonctionnent, pas qu'elle expose toute la
//! surface de `dengon-core`. La seule fonction exposée,
//! [`dengon_decode_reencode`], rejoue exactement ce que
//! `dengon-conformance` vérifie côté Rust `no_std` (US-222) : décoder un
//! paquet L3 puis le ré-encoder, ce qui prouve que le décodeur prend les
//! mêmes décisions **qu'il soit appelé depuis Rust ou depuis C**. Les
//! vecteurs de `contracts/packet/vectors_v0.json` sont rejoués contre elle
//! par un programme C hôte (`tests/c/host_test.c`, dans `dengon-core-embed`),
//! qui remplace le proxy `dengon-conformance` dans le job `cross-vectors`
//! pour la patte firmware (voir `docs/suivi/03-ecarts-conception.md`, entrée
//! US-307). Le routage, le journal et l'observabilité seront exposés par une
//! US ultérieure, au fil des besoins réels du firmware (US-308/309), plutôt
//! que par anticipation ici.
//!
//! # Sécurité de la frontière FFI
//!
//! - `dengon_core::protocol::decode` ne panique sur aucune entrée (c'est son
//!   contrat, voir sa documentation) : un `input` arbitraire, y compris
//!   tronqué ou hostile, ne peut donc pas faire paniquer cette bibliothèque
//!   via le chemin de décodage.
//! - Chaque fonction exportée vérifie ses pointeurs avant de les
//!   déréférencer et documente sa section `# Safety`.
//! - `unsafe_code = "deny"` (lints de workspace) est neutralisé ici pour la
//!   même raison que dans `dengon-ffi` (voir son `src/lib.rs`) : une
//!   frontière C ne peut pas s'écrire sans `unsafe`. C'est précisément pour
//!   ce cas que le lint est en `deny` et non en `forbid`.

#![no_std]
#![allow(unsafe_code)]

use dengon_core::protocol::{decode, encode};

/// Résultat d'un appel à [`dengon_decode_reencode`].
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Succès : `output`/`output_len` portent le paquet ré-encodé.
    Ok = 0,
    /// `input` n'est pas un paquet L3 valide ([`dengon_core::protocol::DecodeError`]
    /// ou, en pratique jamais ici, [`dengon_core::protocol::EncodeError`] sur un
    /// paquet pourtant valide, l'un et l'autre confondus côté C : le détail
    /// reste consultable côté Rust, ce que la frontière C n'a pas besoin de
    /// distinguer pour rejouer les vecteurs de conformité).
    Decode = 1,
    /// `output_cap` est trop petit pour recevoir le paquet ré-encodé.
    BufferTooSmall = 2,
    /// Un pointeur obligatoire est nul.
    NullPointer = 3,
}

/// Décode `input` (`input_len` octets) comme un paquet L3 dengon, puis le
/// ré-encode dans `output` (capacité `output_cap` octets). Écrit la longueur
/// réellement produite dans `*output_len` **uniquement** en cas de succès.
///
/// N'alloue rien à l'échec ; ne touche à `output`/`*output_len` qu'en cas de
/// [`Status::Ok`]. C'est la fonction que rejoue le programme C hôte de
/// `tests/c/host_test.c` contre `contracts/packet/vectors_v0.json`.
///
/// # Safety
///
/// - `input` doit pointer vers `input_len` octets lisibles (ou être nul avec
///   `input_len == 0`).
/// - `output` doit pointer vers `output_cap` octets accessibles en écriture
///   (ou être nul avec `output_cap == 0`).
/// - `output_len` doit pointer vers un `usize` valide et accessible en
///   écriture ; il ne doit pas être nul.
#[no_mangle]
pub unsafe extern "C" fn dengon_decode_reencode(
    input: *const u8,
    input_len: usize,
    output: *mut u8,
    output_cap: usize,
    output_len: *mut usize,
) -> Status {
    if output_len.is_null()
        || (input.is_null() && input_len > 0)
        || (output.is_null() && output_cap > 0)
    {
        return Status::NullPointer;
    }

    // SAFETY : `input` est non nul dès que `input_len > 0` (vérifié
    // ci-dessus), et l'appelant garantit `input_len` octets lisibles.
    let raw = if input_len == 0 {
        &[]
    } else {
        unsafe { core::slice::from_raw_parts(input, input_len) }
    };

    let Ok(packet) = decode(raw) else {
        return Status::Decode;
    };
    let Ok(encoded) = encode(&packet) else {
        // `decode` a déjà validé les invariants qu'`encode` revérifie :
        // n'arrive jamais en pratique (voir `encode_reproduit_les_vecteurs_*`
        // dans `dengon-conformance`), mais reste géré sans `unwrap`.
        return Status::Decode;
    };

    if encoded.len() > output_cap {
        return Status::BufferTooSmall;
    }

    // SAFETY : `output` est non nul dès que `output_cap > 0` (vérifié
    // ci-dessus), `encoded.len() <= output_cap`, et l'appelant garantit
    // `output_cap` octets accessibles en écriture.
    if !encoded.is_empty() {
        unsafe {
            core::ptr::copy_nonoverlapping(encoded.as_ptr(), output, encoded.len());
        }
    }
    // SAFETY : `output_len` est non nul (vérifié ci-dessus) et valide selon
    // le contrat documenté en `# Safety`.
    unsafe {
        *output_len = encoded.len();
    }

    Status::Ok
}

/// Version du protocole dengon implémentée par cette bibliothèque
/// (`dengon_core::PROTOCOL_VERSION`). Fonction triviale, appelée par le
/// programme C hôte pour prouver que la liaison marche avant de rejouer les
/// vecteurs.
#[no_mangle]
pub extern "C" fn dengon_protocol_version() -> u8 {
    dengon_core::PROTOCOL_VERSION
}
