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
 * Résultat d'un appel à une fonction de cette bibliothèque.
 */
enum DengonStatus {
    /**
     * Succès : `output`/`output_len` portent le paquet ré-encodé.
     */
    DENGON_STATUS_OK = 0,
    /**
     * `input` n'est pas un paquet L3 valide ([`dengon_core::protocol::DecodeError`]
     * ou, en pratique jamais ici, [`dengon_core::protocol::EncodeError`] sur un
     * paquet pourtant valide, l'un et l'autre confondus côté C : le détail
     * reste consultable côté Rust, ce que la frontière C n'a pas besoin de
     * distinguer pour rejouer les vecteurs de conformité).
     */
    DENGON_STATUS_DECODE = 1,
    /**
     * `output_cap` est trop petit pour recevoir le paquet ré-encodé.
     */
    DENGON_STATUS_BUFFER_TOO_SMALL = 2,
    /**
     * Un pointeur obligatoire est nul.
     */
    DENGON_STATUS_NULL_POINTER = 3,
    /**
     * File vide : rien à rendre (`dengon_relay_pop_*`, US-308).
     */
    DENGON_STATUS_EMPTY = 4,
    /**
     * Échec cryptographique (auto-test Noise, US-308).
     */
    DENGON_STATUS_CRYPTO = 5,
};
typedef uint8_t DengonStatus;

/**
 * Handle opaque du relais.
 */
typedef struct DengonRelay DengonRelay;

/**
 * Horloges passées à chaque appel. `wall_ms` : ms UTC, ou toute valeur
 * antérieure à 2024 si l'heure est inconnue (le relais l'apprend alors d'un
 * `ANNOUNCE`) ; `mono_ms` : uptime, ne recule jamais.
 */
typedef struct DengonNow {
    /**
     * Horloge murale, ms UTC.
     */
    uint64_t wall_ms;
    /**
     * Horloge monotone, ms.
     */
    uint64_t mono_ms;
} DengonNow;

/**
 * Compteurs du relais (pour `relay.health` et les journaux série).
 */
typedef struct DengonRelayStats {
    /**
     * Trames illisibles.
     */
    uint64_t malformed;
    /**
     * Paquets signés refusés (usurpation, signature).
     */
    uint64_t unauthentic;
    /**
     * Paquets relayés.
     */
    uint64_t relayed;
    /**
     * Enveloppes déposées.
     */
    uint64_t envelopes_stored;
    /**
     * Enveloppes remises.
     */
    uint64_t envelopes_handed_off;
    /**
     * Paquets ignorés faute d'heure murale.
     */
    uint64_t clock_unknown;
    /**
     * Enveloppes détenues maintenant.
     */
    uint32_t envelopes_held;
    /**
     * Paquets au cache de réconciliation maintenant.
     */
    uint32_t cache_len;
} DengonRelayStats;

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

/**
 * Crée un relais. `dh_secret` / `sign_seed` : 32 octets chacun, générés une
 * fois puis relus de NVS. `anchor_seq` / `anchor_hash` (32 octets) : curseur
 * du journal persisté, ou `0` / `NULL` au tout premier démarrage (genèse).
 * `pseudo` : chaîne C UTF-8, ou `NULL` pour `"relais"`. Rend `NULL` si un
 * secret manque.
 *
 * # Safety
 *
 * `dh_secret`, `sign_seed` : valides pour 32 octets. `anchor_hash` : nul ou
 * valide pour 32 octets. `pseudo` : nul ou chaîne C terminée par `\0`.
 */
struct DengonRelay *dengon_relay_new(const uint8_t *dh_secret,
                                     const uint8_t *sign_seed,
                                     uint64_t anchor_seq,
                                     const uint8_t *anchor_hash,
                                     const char *pseudo,
                                     uint64_t routing_seed);

/**
 * Libère un relais créé par [`dengon_relay_new`] (sans effet sur `NULL`).
 *
 * # Safety
 *
 * `relay` : nul, ou rendu par [`dengon_relay_new`] et pas encore libéré.
 */
void dengon_relay_free(struct DengonRelay *relay);

/**
 * Écrit le `peerID` du relais (8 octets) dans `out`.
 *
 * # Safety
 *
 * `relay` valide ; `out` valide pour 8 octets écrits.
 */
DengonStatus dengon_relay_peer_id(const struct DengonRelay *relay, uint8_t *out);

/**
 * Écrit la clé publique Ed25519 du relais (32 octets) dans `out` : celle de
 * `dengon-verify --pubkey`.
 *
 * # Safety
 *
 * `relay` valide ; `out` valide pour 32 octets écrits.
 */
DengonStatus dengon_relay_verifying_key(const struct DengonRelay *relay, uint8_t *out);

/**
 * Un lien vient de s'ouvrir.
 *
 * # Safety
 *
 * `relay` nul ou valide.
 */
void dengon_relay_link_up(struct DengonRelay *relay,
                          uint64_t link,
                          struct DengonNow now);

/**
 * Le lien est tombé.
 *
 * # Safety
 *
 * `relay` nul ou valide.
 */
void dengon_relay_link_down(struct DengonRelay *relay, uint64_t link);

/**
 * Une trame (`len` octets à `bytes`) est arrivée sur `link`.
 *
 * # Safety
 *
 * `relay` nul ou valide ; `bytes` valide pour `len` octets (ou nul si
 * `len == 0`).
 */
void dengon_relay_on_frame(struct DengonRelay *relay,
                           uint64_t link,
                           const uint8_t *bytes,
                           uintptr_t len,
                           struct DengonNow now);

/**
 * Fait avancer toutes les échéances (relais jitterés, pushs, expirations).
 *
 * # Safety
 *
 * `relay` nul ou valide.
 */
void dengon_relay_poll(struct DengonRelay *relay, struct DengonNow now);

/**
 * Relais jitterés arrivés à échéance seulement (tâche `route`).
 *
 * # Safety
 *
 * `relay` nul ou valide.
 */
void dengon_relay_poll_routing(struct DengonRelay *relay, struct DengonNow now);

/**
 * Pushs d'inventaire cadencés seulement (tâche `inventory`).
 *
 * # Safety
 *
 * `relay` nul ou valide.
 */
void dengon_relay_poll_inventory(struct DengonRelay *relay, struct DengonNow now);

/**
 * Expiration des enveloppes seulement (tâche `courier`).
 *
 * # Safety
 *
 * `relay` nul ou valide.
 */
void dengon_relay_poll_courier(struct DengonRelay *relay, struct DengonNow now);

/**
 * Prochaine échéance (ms **monotones**) dans `*out` ; `false` s'il n'y en a
 * pas (le firmware peut alors attendre la prochaine trame).
 *
 * # Safety
 *
 * `relay` nul ou valide ; `out` nul ou valide.
 */
bool dengon_relay_next_deadline(const struct DengonRelay *relay,
                                struct DengonNow now,
                                uint64_t *out);

/**
 * Retire la prochaine trame à émettre : lien dans `*link`, octets dans
 * `buf`, longueur dans `*len`. [`Status::Empty`] si rien à émettre ;
 * [`Status::BufferTooSmall`] si `cap` ne suffit pas (`*len` = taille
 * requise, trame conservée).
 *
 * # Safety
 *
 * `relay` valide ; `link`, `len` valides ; `buf` valide pour `cap` octets.
 */
DengonStatus dengon_relay_pop_outgoing(struct DengonRelay *relay,
                                       uint64_t *link,
                                       uint8_t *buf,
                                       uintptr_t cap,
                                       uintptr_t *len);

/**
 * Retire la prochaine entrée de journal à persister (`Entry::to_bytes`,
 * à ajouter telle quelle à `ledger.bin`). Mêmes conventions que
 * [`dengon_relay_pop_outgoing`].
 *
 * # Safety
 *
 * `relay` valide ; `len` valide ; `buf` valide pour `cap` octets.
 */
DengonStatus dengon_relay_pop_ledger(struct DengonRelay *relay,
                                     uint8_t *buf,
                                     uintptr_t cap,
                                     uintptr_t *len);

/**
 * Curseur du journal **après** toutes les entrées produites jusqu'ici
 * (y compris celles pas encore retirées par [`dengon_relay_pop_ledger`]) :
 * `seq` de la prochaine entrée dans `*seq`, hash de la dernière (32
 * octets) dans `hash`. À persister **une fois la file vidée et écrite**.
 *
 * # Safety
 *
 * `relay`, `seq` valides ; `hash` valide pour 32 octets écrits.
 */
DengonStatus dengon_relay_ledger_anchor(const struct DengonRelay *relay,
                                        uint64_t *seq,
                                        uint8_t *hash);

/**
 * Ajoute au journal un événement du firmware (`relay.boot`,
 * `peer.connected`…). `false` si le nom n'est pas au catalogue ou si une
 * chaîne n'est pas de l'UTF-8.
 *
 * # Safety
 *
 * `relay` nul ou valide ; `name`, `payload_json` nuls ou chaînes C
 * terminées par `\0`.
 */
bool dengon_relay_record_event(struct DengonRelay *relay,
                               const char *name,
                               const char *payload_json,
                               struct DengonNow now);

/**
 * Écrit le `node_id` du relais côté dashboard (`relay-` + `peerID[0..3]` en
 * hex, chaîne C de 12 caractères terminée par `\0`) dans `out` (US-309).
 * Pas de constante exportée pour la taille : cbindgen lui ajouterait
 * le préfixe `Dengon` ; le firmware définit `DENGON_NODE_ID_LEN` (13).
 *
 * # Safety
 *
 * `relay` valide ; `out` valide pour 13 octets écrits.
 */
DengonStatus dengon_relay_node_id(const struct DengonRelay *relay, char *out);

/**
 * Construit le corps signé de `POST /ingest/batch` (US-309) à partir
 * d'entrées de journal **déjà retirées** par [`dengon_relay_pop_ledger`],
 * concaténées telles quelles dans `entries` (`entries_len` octets). La
 * signature Ed25519 est faite ici, avec la clé du relais, qui ne sort pas
 * du handle.
 *
 * Écrit le JSON (non terminé par `\0`) dans `out` et sa longueur dans
 * `*out_len`. [`Status::BufferTooSmall`] : `*out_len` porte la taille
 * requise. [`Status::Decode`] : une entrée est tronquée, hors catalogue ou
 * son payload n'est pas un objet JSON — le lot ne partira jamais tel quel.
 * [`Status::Empty`] : aucune entrée.
 *
 * # Safety
 *
 * `relay`, `out_len` valides ; `entries` valide pour `entries_len` octets
 * lus ; `out` valide pour `out_cap` octets écrits.
 */
DengonStatus dengon_relay_build_batch(const struct DengonRelay *relay,
                                      const uint8_t *entries,
                                      uintptr_t entries_len,
                                      uint8_t *out,
                                      uintptr_t out_cap,
                                      uintptr_t *out_len);

/**
 * Compteurs du relais dans `*out`.
 *
 * # Safety
 *
 * `relay`, `out` valides.
 */
DengonStatus dengon_relay_stats(const struct DengonRelay *relay,
                                struct DengonRelayStats *out);

/**
 * Auto-test Noise avec l'aléa de la plateforme : deux paires de clés
 * tirées de `fill`, handshake `XX` complet, puis un message chiffré dans
 * chaque sens. [`Status::Ok`] si tout aboutit, [`Status::Crypto`] sinon
 * (notamment si `snow` n'obtient aucun aléa), [`Status::NullPointer`] si
 * `fill` est nul.
 *
 * # Safety
 *
 * `fill` : voir [`PlatformRng::new`].
 */
DengonStatus dengon_noise_selftest(void (*fill)(uint8_t *buf, uintptr_t len));

#endif  /* DENGON_CORE_H */
