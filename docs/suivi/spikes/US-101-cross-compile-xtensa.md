# Spike A (US-101) — `protocol` + `crypto` cross-compilent-ils pour `xtensa-esp32-none-elf` ?

**Issue :** [#1](https://github.com/G1TS23/dengon/issues/1) · **Sprint** S1 ·
**Area** `core-rust` · 3 pts · **Jalon J0** · Timebox 1 jour (tenue : ~1 h de
manipulation, dont 15 min de téléchargement de toolchain).
**Date :** 2026-09-10 · **Auteur :** Paul Claverie + Claude (Opus 5).
**Tranche :** [B-1](../../synthese/01-sujets-a-trancher.md) — curseur Rust / C de
la crypto embarquée.

---

## 1. Réponse

> # OUI.
>
> Les briques crypto envisagées — `sha2`, `ed25519-dalek`, `x25519-dalek`,
> `chacha20poly1305` **et `snow` (Noise)** — compilent toutes pour
> `xtensa-esp32-none-elf` en `no_std + alloc`, et produisent un
> `libdengon_core.a` linkable dans un projet ESP-IDF.
>
> **À deux conditions, toutes deux tenables :**
>
> 1. **`snow` ≥ 0.10.0** (sortie récente). La version 0.9.6, celle que la
>    conception avait en tête, **ne peut pas** être compilée en `no_std` — voir
>    §5. C'est la seule vraie découverte du spike.
> 2. **Le firmware fournit lui-même la source d'aléa.** Sans la feature
>    `use-getrandom` (indisponible sur cette cible), `snow` compile mais son
>    resolver ne sait pas tirer d'aléa : le handshake échouerait **au runtime**.
>    Il faut un `CryptoResolver` maison branché sur `esp_fill_random()` de
>    l'ESP-IDF. Ce chemin a été écrit et compilé (§4, étape 6).

**Conséquence pour le projet :** le repli « trait `Crypto` + mbedTLS côté C »
prévu par A-3 et par [`08-relais-esp32.md`](../../synthese/08-relais-esp32.md)
**n'a pas à être activé**. La branche firmware (US-307 → US-308 → US-309 →
US-312) part sur du **tout-Rust**. Aucune implémentation crypto séparée en C
n'est à écrire ni à maintenir, donc pas de risque d'incompatibilité de format
d'octets entre les deux côtés — c'était le principal argument « contre » de
l'option libsodium/mbedTLS dans A-3.

---

## 2. Question posée

`docs/synthese/08-relais-esp32.md:71` suppose un
`libdengon_core.a  (Rust, no_std+alloc, cross-compilé xtensa)` embarquant
`protocol`, routing/dedup, inventaire, `ledger`, `observability` — et laisse
ouvert le cas de la crypto : « Rust si Spike A OK ; sinon trait `Crypto` +
mbedTLS pour le seul handshake Noise XX de lien ».

Le spike tranche exactement ce « si ».

---

## 3. Environnement exact

| Élément | Valeur |
|---|---|
| Machine | Linux 6.6.87.2 (WSL2), x86_64 |
| Rust hôte | 1.98.1 stable |
| `espup` | 0.17.1 (`cargo install espup --locked`, 7 min 41 s de compilation) |
| Toolchain Xtensa | `esp` = `rustc 1.97.0-nightly (8ea53bcd7 2026-07-08) (1.97.0.0)`, LLVM 21.1.3 |
| GCC | `xtensa-esp-elf` (installé par `espup`) |
| Empreinte disque | 1,9 Go (`~/.rustup/toolchains/esp`), dont 335 Mo de LLVM Xtensa |
| Durée `espup install` | 6 min 54 s |

**Point à connaître :** `xtensa-esp32-none-elf` **n'existe pas** dans le Rust
amont. Elle n'est fournie que par le fork d'Espressif, installé par `espup`.
C'est une cible **tier 3** : `core` et `alloc` ne sont pas précompilés, il faut
les rebâtir à la volée (`-Z build-std`, d'où l'obligation d'une toolchain
nightly — celle du fork l'est).

Configuration de la crate jouet :

```toml
# rust-toolchain.toml
[toolchain]
channel = "esp"

# .cargo/config.toml
[build]
target = "xtensa-esp32-none-elf"
[unstable]
build-std = ["core", "alloc"]
```

---

## 4. Protocole suivi et résultats

Crate jouet `no_std`, `crate-type = ["staticlib"]` (exactement la forme attendue
par le firmware), avec allocateur bidon + `panic_handler`. Les dépendances ont
été ajoutées **une par une**, avec une compilation après chacune : un échec
désigne ainsi la brique fautive au lieu d'un « ça ne compile pas » inexploitable.

Chaque brique est **réellement appelée** derrière un `#[no_mangle] extern "C"` —
sinon l'éditeur de liens élague le code et on « compile » du vide.

| # | Brique | Version | Features utilisées | Résultat |
|---|---|---|---|---|
| 1 | socle `core` + `alloc` | — | — | ✅ |
| 2 | `sha2` | 0.10.9 | `default-features = false` | ✅ |
| 3 | `ed25519-dalek` | 2.2.0 | `default-features = false`, `zeroize` | ✅ |
| 4 | `x25519-dalek` | 2.0.1 | `default-features = false`, `static_secrets` | ✅ |
| 5 | `chacha20poly1305` | 0.10.1 | `default-features = false`, `alloc` | ✅ |
| 6a | `snow` **0.9.6** | 0.9.6 | `default-features = false`, `default-resolver` | ❌ **échec** |
| 6b | `snow` **0.10.0** | 0.10.0 | `default-features = false` + `default-resolver`, `use-curve25519`, `use-chacha20poly1305`, `use-sha2` | ✅ |
| 6c | idem + `CryptoResolver` sur `esp_fill_random()` | — | — | ✅ |

Compilation propre de bout en bout : **54,69 s**, 0 erreur.
Artefact : `libspike_us101.a`, **2 105 688 octets**.

Preuve que le code est bien présent dans l'archive (et pas élagué) :

```
$ xtensa-esp32-elf-nm libspike_us101.a | grep " T spike_"
00000000 T spike_aead
00000000 T spike_ed25519
00000000 T spike_noise_xx
00000000 T spike_sha256
00000000 T spike_socle
00000000 T spike_x25519

$ xtensa-esp32-elf-nm libspike_us101.a | grep esp_fill_random
         U esp_fill_random          # non defini ici : resolu au link par l'ESP-IDF
```

`snow` contribue 211 symboles et 4 objets à l'archive : le cadre Noise est bien
compilé pour xtensa, pas seulement « accepté ».

---

## 5. Le point dur : pourquoi `snow` 0.9 échoue et 0.10 passe

C'est le cœur du spike, et la raison pour laquelle B-1 hésitait.

**`snow` 0.9.6 est structurellement impossible en `no_std`** — trois causes
cumulées, dont aucune ne se contourne depuis la crate appelante :

1. Son `Cargo.toml` déclare `rand_core` en dépendance **non optionnelle** avec
   `features = ["std", "getrandom"]` **en dur**. Les features Cargo étant
   *additives*, un utilisateur ne peut pas les retirer.
2. `getrandom` 0.2 n'a **pas de backend** pour `xtensa-esp32-none-elf` :
   `error: target is not supported`.
3. `snow/src/lib.rs` **ne porte aucun `#![no_std]`** et son propre code utilise
   `std::` dans 8 fichiers. Même en réglant 1 et 2, il faudrait patcher la crate.

Erreurs obtenues (log `08-etape6-snow.log`) :

```
error[E0463]: can't find crate for `std`   --> subtle-2.6.1/src/lib.rs:93:1
error[E0463]: can't find crate for `std`   --> getrandom-0.2.17/src/error_impls.rs:1:1
error: target is not supported, for more information see: https://docs.rs/getrandom/#unsupported-targets
```

**`snow` 0.10.0 a été refait pour ça** :

- `#![cfg_attr(not(feature = "std"), no_std)]` — `no_std` officiellement
  supporté « si `alloc` est fourni » (dixit sa doc, `src/lib.rs:96`) ;
- `std` devient une feature **opt-in**, pas un défaut ;
- `getrandom` devient **optionnel**, derrière `use-getrandom` ;
- les primitives sont sélectionnables une par une (`use-curve25519`,
  `use-chacha20poly1305`, `use-sha2`), ce qui évite de tirer `aes-gcm` et
  `blake2` dont on n'a pas besoin.

Résultat : **`getrandom` est totalement absent de l'arbre de dépendances final**
(`cargo tree -e normal | grep -c getrandom` → `0`). Le seul `rand_core` restant
(0.6.4, tiré par `x25519-dalek`) est en `no_std` et ne pose pas de problème.

**La contrepartie** — et c'est ce qu'il faut retenir côté firmware : sans
`use-getrandom`, `DefaultResolver::resolve_rng()` renvoie `None`, et
`build_initiator()` échouerait **à l'exécution** avec `Error::Rng`. Le compilateur
ne le dira pas. Il faut donc impérativement un resolver maison :

```rust
struct EspRng;
impl snow::types::Random for EspRng {
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), snow::Error> {
        unsafe { esp_fill_random(dest.as_mut_ptr().cast(), dest.len()) };
        Ok(())
    }
}
// puis snow::Builder::with_resolver(params, Box::new(EspResolver(DefaultResolver)))
```

⚠️ `esp_fill_random()` n'est un vrai TRNG que si le Wi-Fi ou le Bluetooth est
actif, ou après `bootloader_random_enable()`. Sinon il renvoie du
pseudo-aléatoire. **À vérifier en US-307** : c'est une faille de sécurité
silencieuse, pas une erreur de compilation.

---

## 6. Ce que le spike ne prouve pas

Honnêteté sur le périmètre (règle n°7 de [`../README.md`](../README.md)) :

- **Aucune exécution sur matériel.** On a compilé et lié, pas fait tourner. Les
  vecteurs de conformité cible restent à faire en US-307 / DoD « Firmware ».
- **Le link final dans un vrai projet ESP-IDF n'est pas testé.** L'archive est
  produite pour `xtensa-esp32-none-elf` (bare-metal) alors que l'ESP-IDF
  compile en `xtensa-esp32-espidf`. Les deux s'assemblent en pratique (c'est le
  montage habituel d'une `staticlib` Rust dans un composant IDF), mais la
  cohabitation des allocateurs et du `panic_handler` reste à valider.
- **Ni la taille flash/RAM réelle, ni les performances** n'ont été mesurées. Le
  `.a` de 2,1 Mo n'est pas représentatif : il n'est pas élagué et contient les
  symboles de debug.
- `protocol` n'existe pas encore (US-104 n'est pas mergée) : on a testé les
  **dépendances** qu'il utilisera, pas son code.

---

## 7. Suites à donner

| Quoi | Où |
|---|---|
| Épingler `snow = "0.10"` (et **pas** `0.9`) au moment d'ajouter la crypto | US-108, `Cargo.toml` du workspace |
| Écrire le `CryptoResolver` ESP32 + vérifier l'entropie réelle d'`esp_fill_random` | US-307 |
| Valider le link `staticlib` dans un composant ESP-IDF | US-307 |
| Mesurer flash / RAM après élagage | US-308 |

---

## Annexe — commandes réellement exécutées

```bash
cargo install espup --locked          # espup 0.17.1
espup install                         # toolchain esp + LLVM xtensa
. ~/export-esp.sh
rustc +esp --print target-list | grep xtensa      # xtensa-esp32-none-elf present

# crate jouet, dependances ajoutees une par une
cargo build --release                 # x6, une fois par brique

# verifications
cargo tree -e normal | grep -c getrandom          # 0
xtensa-esp32-elf-nm libspike_us101.a | grep " T spike_"
xtensa-esp32-elf-ar t libspike_us101.a | grep snow
```

Logs bruts conservés hors dépôt (code jetable, critère d'acceptation n°5) :
`scratchpad/spike-us101/logs/01..12`.
