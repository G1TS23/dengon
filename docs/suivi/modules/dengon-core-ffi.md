# Module : `dengon-core-ffi` + `dengon-core-embed` (`crates/dengon-core-ffi/`, `crates/dengon-core-embed/`)

**Rôle en une phrase :** pont C de `dengon-core` (`dengon-core-ffi`) et
assemblage de l'archive autonome `libdengon_core.a` (`dengon-core-embed`) que
le firmware ESP32 liera.
**Correspond à la conception :** `docs/synthese/08-relais-esp32.md` §3
(`libdengon_core.a`), Spike A (`docs/suivi/spikes/US-101-cross-compile-xtensa.md`).
**Dernière mise à jour :** 2026-09-29
**État :** partiel. US-307 : chaîne de compilation, header et vecteurs
rejoués depuis le C. US-308 : API du relais et auto-test Noise, **liée par le
firmware** (`components/dengon_core_ffi`), première compilation xtensa
effective (`libdengon_core.a` de 1,58 Mo, espup 0.17.1).

## À quoi ça sert

Le firmware ESP32 est écrit en C (ESP-IDF) mais doit réutiliser LE décodeur
de paquets L3 de `dengon-core` (décision A-2 : une seule implémentation du
protocole) plutôt que d'en réécrire un second en C. US-307 construit le pont :
des fonctions Rust `#[no_mangle] extern "C"`, un header `.h` généré par
`cbindgen`, et une archive `.a` cross-compilée pour `xtensa-esp32-none-elf`
qu'un projet ESP-IDF peut lier.

## Structure

```
dengon-core-ffi/            — fonctions C exportées, `no_std`, rlib normal
  Cargo.toml
  build.rs                  — régénère include/dengon_core.h via cbindgen
  cbindgen.toml
  include/dengon_core.h     — header versionné, NE PAS ÉDITER À LA MAIN
  src/lib.rs                — dengon_decode_reencode, dengon_protocol_version
  tests/decode_reencode.rs  — vecteurs vectors_v0.json rejoués via la frontière C, en Rust

dengon-core-embed/           — racine de workspace SÉPARÉE (voir son Cargo.toml)
  Cargo.toml                — [workspace] vide, panic = "abort", lints dupliqués
  rust-toolchain.toml       — nightly + rust-src (pas la toolchain figée du dépôt)
  .cargo/config.toml        — build-std = ["core", "alloc"]
  src/lib.rs                — réexporte dengon-core-ffi + alloc/panic handler libc
  tests/c/
    generate_vectors.py     — JSON -> tableaux C, régénéré à chaque run, non committé
    host_test.c             — programme C hôte, lie libdengon_core.a
    run.sh                  — reproduit localement ce que fait la CI
```

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `Status` (enum, `repr(u8)`) | `dengon-core-ffi/src/lib.rs:53` | Verdict C : `OK`/`DECODE`/`BUFFER_TOO_SMALL`/`NULL_POINTER` |
| `dengon_decode_reencode` | `dengon-core-ffi/src/lib.rs:83` | Décode `input` puis ré-encode dans `output` ; ne panique jamais sur une entrée hostile |
| `dengon_protocol_version` | `dengon-core-ffi/src/lib.rs:139` | Fonction triviale, preuve de liaison avant de rejouer les vecteurs |
| `LibcAlloc` / `#[panic_handler]` | `dengon-core-embed/src/lib.rs` | Allocateur global (`malloc`/`free`) et panic handler (`abort`) de l'archive autonome |

## Flux principal (exemple)

1. `contracts/packet/vectors_v0.json` contient un paquet `accept` en hexadécimal.
2. `generate_vectors.py` le transforme en tableau `uint8_t[]` C, à la volée.
3. `host_test.c` appelle `dengon_decode_reencode(bytes, len, out, cap, &out_len)`.
4. La fonction Rust décode avec `dengon_core::protocol::decode` (le même code
   que `dengon-core`/`dengon-conformance`), ré-encode, copie dans `out`.
5. `host_test.c` compare `out` à l'entrée (bits réservés masqués près) :
   identique à ce que `dengon-conformance` vérifie côté Rust `no_std`, mais
   franchissant réellement la frontière C.

## Dépendances

- **Internes :** `dengon-core` (`default-features = false`, `no_std` + `alloc`,
  même piège de dépendance de chemin que `dengon-conformance`).
- **Externes (crates) :** `cbindgen` (build-dependency de `dengon-core-ffi`
  uniquement — reparse le source, ne compile rien).

## Décisions d'implémentation

- **Deux crates, pas une** : `dengon-core-ffi` (rlib, testable par
  `cargo test` comme le reste du workspace) et `dengon-core-embed`
  (`staticlib`, `test = false`, fournit l'allocateur + panic handler). Essayé
  en une seule crate avec une feature Cargo `standalone` d'abord : casse dès
  que `cargo clippy --workspace --all-targets --all-features` unifie les
  features (`error[E0152]: duplicate lang item 'panic_impl'`, le panic
  handler de la feature entrant en conflit avec celui du harnais `std` de
  test). Voir `docs/suivi/03-ecarts-conception.md`.
- **`dengon-core-embed` hors du workspace principal** : `libdengon_core.a`
  doit compiler en `panic = "abort"` + `-Z build-std` (sysroot précompilé
  autrement avec `panic = "unwind"`, lien échoue sur
  `rust_eh_personality` non défini — vérifié sur cible hôte
  `x86_64-pc-windows-gnu`). Un réglage qui s'applique à toute une invocation
  `cargo`, incompatible avec le reste du workspace (`std`, `unwind`). D'où
  `[workspace]` vide dans son `Cargo.toml` (racine séparée) et l'exclusion
  dans le `Cargo.toml` principal.
- **Toolchain `nightly` par défaut, `esp` seulement dans le job `firmware`** :
  la vérification hôte (`cross-vectors`) n'a besoin que de `build-std`, pas
  de la cible xtensa — `esp` (~2 Go, ~7 min, Spike A §3) serait un coût
  inutile à chaque run. Le job `firmware`, qui installe `espup` de toute
  façon pour la cible réelle, invoque `cargo +esp` explicitement.
- **Allocateur `malloc`/`free`/`abort` de la libc**, pas `heap_caps_*`
  d'ESP-IDF : existent des deux côtés (glibc/MSVC pour les tests hôte,
  newlib d'ESP-IDF pour la cible), sans `#[cfg(target_os)]`. Pas
  l'allocateur idiomatique ESP-IDF (SPIRAM, capacités DMA) — à évaluer en
  US-308/309 si besoin réel.
- **Surface C minimale** : une seule fonction utile,
  `dengon_decode_reencode` (+ une fonction triviale de preuve de liaison).
  US-307 est la tête du chemin critique firmware, pas une US d'exposition
  complète de `dengon-core` — le routage, le journal, l'observabilité
  suivront au fil des besoins réels (US-308/309).

## Tests

- `cargo test -p dengon-core-ffi` → 6 tests d'intégration
  (`tests/decode_reencode.rs`) : vecteurs accept/reject, tampon trop petit,
  pointeurs nuls, entrée vide. Verts.
- `cd crates/dengon-core-embed && ./tests/c/run.sh` (ou `+nightly` si la
  toolchain du dépôt principal est active par défaut) : construit
  `libdengon_core.a` pour l'hôte, régénère les vecteurs C, compile et
  exécute `host_test.c`. Vérifié en écrivant cette US avec `nightly` amont
  sur cible `x86_64-pc-windows-gnu` (`espup` indisponible sur ce poste
  Windows) :
  ```
  dengon_protocol_version() = 1
  vecteurs : 8 accept, 5 reject
  OK : tous les vecteurs de conformite passent depuis le C
  ```
- **Non vérifié localement** : la cible réelle `xtensa-esp32-none-elf` et la
  toolchain `esp` (`espup`) — nécessitent un poste Linux/WSL2 (Spike A) ou le
  job CI `firmware`, qui les installe. Le mécanisme (`build-std` +
  `panic = "abort"`) a été validé avec `nightly` amont, dont `esp` est un
  fork ; seule la CI exerce réellement `esp` + `xtensa-esp32-none-elf`.
- **Non fait** : exécution sur matériel (carte ESP32), intégration dans
  `firmware/dengon-relay` (CMake) — hors périmètre de cette US, prévu en
  US-308/309.

## US-308 : l'API C du relais

| Fonction C | Rôle |
|---|---|
| `dengon_relay_new` / `_free` | Crée le relais depuis ses secrets (32 + 32 o) et l'ancre du journal (`seq`, hash ou `NULL` = genèse). |
| `dengon_relay_peer_id` / `_verifying_key` | `peerID` (8 o) et clé Ed25519 (32 o, celle de `dengon-verify --pubkey`). |
| `dengon_relay_link_up` / `_link_down` / `_on_frame` | Entrées du transport. |
| `dengon_relay_poll` / `_poll_routing` / `_poll_inventory` / `_poll_courier` / `_next_deadline` | Échéances, par tâche. |
| `dengon_relay_pop_outgoing` / `_pop_ledger` | Sorties **une à une** dans un tampon de l'appelant ; `DENGON_STATUS_EMPTY` quand c'est vide ; `BUFFER_TOO_SMALL` rend la taille requise **sans perdre** l'élément. |
| `dengon_relay_ledger_anchor` | Curseur à persister. |
| `dengon_relay_record_event` | Événement du firmware (catalogue vérifié). |
| `dengon_relay_stats` | Compteurs (`DengonRelayStats`). |
| `dengon_noise_selftest(fill)` | Handshake Noise `XX` complet et aller-retour chiffré avec l'aléa de `fill` (`esp_fill_random` côté firmware). `DENGON_STATUS_CRYPTO` si ça échoue ou si l'aléa est constant. |

- `PlatformRng` (`src/rng.rs`) implémente `RngCore + CryptoRng` au-dessus d'un
  pointeur de fonction C. C'est tout ce qu'il faut à `CallerResolver` pour
  fournir l'aléa à `snow`, sans second `CryptoResolver`.
- Les variantes C sont **préfixées** (`DENGON_STATUS_OK`…) depuis US-308 : un
  `OK` nu collisionne avec `rom/ets_sys.h` d'ESP-IDF. `host_test.c` a été
  adapté (il repasse : 8 accept, 5 reject).
- Tests : `tests/relay_c_api.rs` (5).

## Limites connues / TODO

- La CI (`cross-vectors`, `firmware`) n'a jamais tourné sur cette branche ni
  sur #108.
- `heap_caps_*` (SPIRAM, DMA) non évalué : `malloc`/`free` suffisent tant
  qu'aucun besoin mémoire spécifique ESP32 n'est identifié.

## Pour l'oral

Le firmware ESP32 est écrit en C, mais on ne veut pas réécrire le décodeur de
paquets une seconde fois dans un langage différent — deux implémentations qui
divergent en silence est exactement le genre de bug qu'on ne détecte qu'en
prod. `dengon-core-ffi`/`dengon-core-embed` sont le pont : le MÊME code Rust,
compilé pour la puce ESP32, appelé depuis C via une poignée de fonctions et
un header généré automatiquement. Le point intéressant : produire une
archive Rust `no_std` totalement autonome (sans aucun autre code Rust autour
pour lui fournir un allocateur mémoire ou gérer ses erreurs fatales) a forcé
à comprendre — et régler soi-même — deux détails habituellement invisibles
d'un programme Rust ordinaire : d'où vient la mémoire allouée, et que se
passe-t-il quand le programme plante.
