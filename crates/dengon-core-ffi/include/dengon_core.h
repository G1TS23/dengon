/* Généré par cbindgen (build.rs de dengon-core-ffi) — NE PAS ÉDITER À LA MAIN.
 * Régénérer : `cargo build -p dengon-core-ffi`, puis committer le résultat.
 */

#ifndef DENGON_CORE_H
#define DENGON_CORE_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Résultat d'un appel à [`dengon_decode_reencode`].
 */
enum DengonStatus {
    /**
     * Succès : `output`/`output_len` portent le paquet ré-encodé.
     */
    OK = 0,
    /**
     * `input` n'est pas un paquet L3 valide ([`dengon_core::protocol::DecodeError`]
     * ou, en pratique jamais ici, [`dengon_core::protocol::EncodeError`] sur un
     * paquet pourtant valide, l'un et l'autre confondus côté C : le détail
     * reste consultable côté Rust, ce que la frontière C n'a pas besoin de
     * distinguer pour rejouer les vecteurs de conformité).
     */
    DECODE = 1,
    /**
     * `output_cap` est trop petit pour recevoir le paquet ré-encodé.
     */
    BUFFER_TOO_SMALL = 2,
    /**
     * Un pointeur obligatoire est nul.
     */
    NULL_POINTER = 3,
};
typedef uint8_t DengonStatus;

/**
 * Décode `input` (`input_len` octets) comme un paquet L3 dengon, puis le
 * ré-encode dans `output` (capacité `output_cap` octets). Écrit la longueur
 * réellement produite dans `*output_len` **uniquement** en cas de succès.
 *
 * N'alloue rien à l'échec ; ne touche à `output`/`*output_len` qu'en cas de
 * [`Status::Ok`]. C'est la fonction que rejoue le programme C hôte de
 * `tests/c/host_test.c` contre `contracts/packet/vectors_v0.json`.
 *
 * # Safety
 *
 * - `input` doit pointer vers `input_len` octets lisibles (ou être nul avec
 *   `input_len == 0`).
 * - `output` doit pointer vers `output_cap` octets accessibles en écriture
 *   (ou être nul avec `output_cap == 0`).
 * - `output_len` doit pointer vers un `usize` valide et accessible en
 *   écriture ; il ne doit pas être nul.
 */
DengonStatus dengon_decode_reencode(const uint8_t *input,
                                    uintptr_t input_len,
                                    uint8_t *output,
                                    uintptr_t output_cap,
                                    uintptr_t *output_len);

/**
 * Version du protocole dengon implémentée par cette bibliothèque
 * (`dengon_core::PROTOCOL_VERSION`). Fonction triviale, appelée par le
 * programme C hôte pour prouver que la liaison marche avant de rejouer les
 * vecteurs.
 */
uint8_t dengon_protocol_version(void);

#endif  /* DENGON_CORE_H */
