# Module : `dengon-ffi` (`crates/dengon-ffi/`)

**Rôle en une phrase :** le pont qui permet à l'application Android, écrite en Kotlin, d'appeler le cœur écrit en Rust.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §3 et §5 ; [`docs/powl/09-data-model.md`](../../powl/09-data-model.md) §1/§3 (formats QR, code de vérification).
**Dernière mise à jour :** 2026-09-28
**État :** **contrat v0 gelé** (US-106) — UDL + bouchon Kotlin. Pas encore branché sur `dengon-core` (US-301/US-302). Retours de revue PR #69 tous traités (2026-09-25 puis 2026-09-28, voir journal) ; PR approuvée.

## Vérification côté Rust — faite depuis le 2026-09-25

Cette crate ajoute la première dépendance externe réelle du workspace
(`uniffi` 0.28.3). Les deux premières sessions (2026-09-20, puis la première
réponse à la revue PR #69 le 2026-09-25) n'avaient pas de toolchain Rust :
`Cargo.lock` non régénéré, fmt/clippy corrigés à la main sans compiler.
Un toolchain a ensuite été installé et le commit `24cd926` (2026-09-25) a
régénéré `Cargo.lock` et fait passer pour de vrai `cargo fmt --check`,
`cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
et `cargo test --workspace --all-features --locked` — confirmé
indépendamment par le relecteur (G1TS23) et par le job CI `core`.

L'API d'UniFFI `0.28.3` (choisie volontairement, plus ancienne et mieux
documentée que `0.31`/`0.32`) compile donc bien avec
`uniffi::generate_scaffolding` / `uniffi::include_scaffolding!`.

## À quoi ça sert

L'application Android est en Kotlin, le protocole est en Rust. Écrire à la main
le code d'interfaçage entre les deux (JNI) est long et une source classique de
plantages. **UniFFI** génère ce code automatiquement à partir d'une description
d'interface (`.udl`) : on décrit les fonctions exposées, l'outil produit la
bibliothèque native et les classes Kotlin correspondantes.

L'US-106 livre la **description d'interface** (le contrat) et un **bouchon
Kotlin** qui l'implémente avec des données canned, en mémoire — pas encore le
vrai pont généré. Objectif : l'UI Android (US-214, US-215) peut se développer
contre une forme d'API stable pendant tout le sprint 2, avant que le vrai FFI
(US-302) n'existe.

## Structure

```
dengon-ffi/
  build.rs        — appelle uniffi::generate_scaffolding("src/dengon.udl")
  uniffi.toml     — paquet Kotlin des bindings générés (com.dengon.app.ffi)
  src/
    dengon.udl    — contrat v0 : Identity, Message, Conversation, NodeEvent,
                    DengonError, interface DengonNode, fonctions identité/QR
    lib.rs        — uniffi::include_scaffolding!("dengon") + implémentation
                    bouchon (DengonNode en mémoire, génération d'identité
                    placeholder, QR base64url, code de vérification placeholder)

android/app/src/main/java/com/dengon/app/ffi/
  DengonTypes.kt      — types Kotlin, MÊME FORME que les bindings générés
  DengonNodeStub.kt   — interface DengonNodeInterface + implémentation canned
                        thread-safe (DengonNodeStub) + fonctions de premier
                        niveau generateIdentity / identityQrCode /
                        identityFromQrCode / verificationCode
android/app/src/test/java/com/dengon/app/ffi/
  DengonNodeStubTest.kt — 10 tests JVM purs (voir « Tests »)
```

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `dengon.udl` | `src/dengon.udl` | Contrat gelé : `dictionary Identity/Message/Conversation`, `enum MessageStatus`, `[Enum] interface NodeEvent`, `[Error] enum DengonError`, `interface DengonNode` (constructor + 5 méthodes), fonctions libres `generate_identity`/`identity_qr_code`/`identity_from_qr_code`/`verification_code`. |
| `DengonNode` (Rust) | `src/lib.rs:138` | Bouchon en mémoire : `Mutex<NodeState>` (messages, conversations, événements en attente, pairs connectés). `send_message` toujours `Ok` ; le vrai routage/erreurs arrivent avec `sync` (US-301). |
| `generate_identity`/`identity_qr_code`/`identity_from_qr_code`/`verification_code` | `src/lib.rs:230-329` | Placeholders **non cryptographiques** : pas de vraies clés X25519/Ed25519 (pas de module `crypto`/`identity` avant US-108/US-203/US-205). Fixent la FORME du contrat (types, format `dengon:v1:…`, 12 groupes de 5 chiffres), pas le contenu. |
| `encode_base64url`/`decode_base64url` | `src/lib.rs:339-393` | base64url (RFC 4648 §5) sans padding, écrit à la main : `dengon-ffi` n'a qu'`uniffi` comme dépendance externe, pas de crate `base64`. |
| `DengonNodeInterface` (Kotlin) | `ffi/DengonNodeStub.kt` | Le nom que génère UniFFI pour l'interface de `class DengonNode`. L'UI dépend de cette interface : `DengonNodeStub` aujourd'hui, `DengonNode(identity)` généré à l'US-302. Le bouchon est thread-safe (un verrou), avec une conversation pré-remplie (critère DoR « un test affiche une conversation »). |
| `generateIdentity` / `identityQrCode` / `identityFromQrCode` / `verificationCode` (Kotlin) | `ffi/DengonNodeStub.kt` | Fonctions de **premier niveau**, comme les génère UniFFI pour un `namespace`. Base64url et code de vérification codés à la main **aussi** côté Kotlin — voir « Décisions d'implémentation ». |
| `DengonException` (Kotlin) | `ffi/DengonTypes.kt` | Scellée, une sous-classe par variante (`UnknownPeer`, `NotConnected`, `Internal`), comme les bindings générés. |

## Flux principal (exemple)

Visé (US-302) : Kotlin appelle les bindings UniFFI générés → `dengon-ffi`
(Rust, vrai FFI) → `dengon-core::api`.

Actuel (US-106) : deux bouchons **indépendants**, un par langage, qui
respectent la même forme de contrat mais ne s'appellent pas l'un l'autre.
L'UI Android peut appeler `DengonNodeStub` dès maintenant ; côté Rust, les
tests appellent `DengonNode` à travers le **vrai** scaffolding généré par
UniFFI depuis `dengon.udl` (pas un bouchon — la preuve que le `.udl` est
syntaxiquement valide, sous réserve de la vérification CI, voir l'avertissement
en tête de fiche).

## Dépendances

- **Internes :** `dengon-core` (juste `PROTOCOL_VERSION`, via `version()`).
- **Externes (crates) :** `uniffi = "=0.28.3"` (`default-features = false` en
  dépendance normale ; feature `"build"` en dépendance de build seulement —
  pas besoin de `"cli"`/`"bindgen"`, la génération Kotlin est hors périmètre
  de l'US-106, elle arrivera avec US-302).

## Décisions d'implémentation

- **UDL plutôt que macros procédurales** (`#[uniffi::export]`) : la DoR de
  l'US-106 demande explicitement un fichier `.udl`. C'est aussi le flux le
  mieux documenté pour la version d'UniFFI choisie.
- **Version d'UniFFI épinglée en EXACT** (`=0.28.3`, pas de caret) : même
  logique que `rust-toolchain.toml` — une montée de version est une PR
  volontaire. Choix de `0.28.3` (novembre 2024) plutôt qu'une version plus
  récente disponible sur crates.io (`0.31`/`0.32`, 2026) : ces dernières ont
  changé d'architecture interne (« pipeline ») et étaient hors de portée pour
  une vérification fiable sans compilateur local — voir l'avertissement en
  tête de fiche.
- **Pas de vraie cryptographie dans les placeholders** (`generate_identity`,
  `verification_code`) : `identity`/`crypto` n'existent pas encore côté
  `dengon-core` (US-108/US-203/US-205). Documenté en commentaire à chaque
  fonction concernée, des deux côtés (Rust et Kotlin) — écart de conception
  volontaire et temporaire, voir `03-ecarts-conception.md`.
- **base64url et le mélange FNV-1a du code de vérification sont codés à la
  main, deux fois (Rust et Kotlin), et ne produisent PAS le même résultat
  numérique** : ce sont deux bouchons indépendants, pas encore reliés par un
  vrai appel FFI. Documenté en commentaire pour éviter la confusion « pourquoi
  ça ne matche pas » quand le vrai FFI arrivera.
- **`android.util.Base64` évité côté Kotlin** au profit d'un encodeur/décodeur
  écrit à la main : `app/build.gradle.kts` a
  `unitTests.isReturnDefaultValues = true` (pas de Robolectric, voir
  `android-app.md`) — un appel à une API `android.*` en test JVM pur renvoie
  une valeur par défaut (`null`) au lieu de s'exécuter réellement. Un
  aller-retour QR basé sur `android.util.Base64` n'aurait donc rien prouvé.
- **Le bouchon Kotlin a la forme EXACTE des bindings générés** (revue PR
  #69 de Paul, 2026-09-28). Référence : `uniffi-bindgen` 0.28.3 lancé sur le
  `.udl` avec `uniffi.toml`. Cinq écarts corrigés :
  - `sequence<u8>` → `bytes` dans le `.udl` (Kotlin `ByteArray` au lieu de
    `List<UByte>`) ;
  - `sent_ms` en `i64` (`Long`) au lieu de `u64` (`ULong`) ;
  - `unread_count` reste `u32`, donc `UInt` côté Kotlin ;
  - `interface DengonNodeInterface` (et non `DengonNode`, le nom de la classe
    générée) ;
  - `DengonException` scellée avec ses trois sous-classes.
  Écart supplémentaire relevé en générant : les fonctions d'identité sont de
  **premier niveau** (plus d'`object DengonIdentity`). Paquet Kotlin fixé à
  `com.dengon.app.ffi` par `uniffi.toml`.
  Vérifié en compilant le fichier de test **inchangé** contre les bindings
  générés (copie jetable du projet Android) : succès ; l'ancien fichier de
  test, lui, donne 16 erreurs de compilation.
- **`Identity` (Kotlin) est une `data class` à champs `ByteArray` SANS
  égalité par contenu**, comme la génère UniFFI. L'ancien bouchon réécrivait
  `equals`/`hashCode` : l'UI aurait pu en dépendre et casser silencieusement à
  l'US-302. Les tests comparent champ par champ (`assertArrayEquals`).
- **`identityFromQrCode` (Kotlin) lève `DengonException.Internal` pour
  TOUTE entrée invalide**, y compris un QR qui n'est pas un QR dengon (URL,
  menu…). Ce cas levait `IllegalArgumentException` (`require`) jusqu'au
  2026-09-28 (revue PR #69, Paul).
- **Pseudo > 255 octets : tronqué à la frontière de caractère, des deux
  côtés.** Le Rust coupait à 255 octets pile (un caractère UTF-8 coupé en
  deux rendait le QR indécodable) et le Kotlin levait une exception.
- **`fromQrCode` (Kotlin) borne explicitement la taille du
  payload avant d'indexer** (`payload.isEmpty()`, puis
  `payload.size < offset + pseudoLen + 2*KEY_LEN`), et lève `DengonException`
  plutôt que de laisser passer une `IndexOutOfBoundsException` — corrigé le
  2026-09-25 suite à la revue PR #69 (un payload tronqué, ex. QR mal scanné,
  plantait sans lever le type d'erreur promis par le contrat). Symétrique au
  `.get(...).ok_or(DengonError::Internal)` déjà utilisé côté Rust.

## Tests

- **Rust** (`cargo test -p dengon-ffi --locked`) : **7 tests** depuis la
  revue PR #69 de Paul (+ pseudo de 260 octets UTF-8 tronqué à 254 sans couper
  de caractère, QR décodable). Détail des 6 premiers : 6 tests dans `src/lib.rs`
  — version, aller-retour base64url, aller-retour QR, **QR malformé** (ajouté
  le 2026-09-28 : octet `pseudo_len` seul, payload valide amputé d'un octet,
  préfixe absent → `Err(DengonError::Internal)`, jamais de panique), symétrie
  du code de vérification, `send_message`/`poll_events`/`on_peer_connected`.
  **Réellement exécutés** : 6/6.
- **Kotlin, depuis la revue PR #69 de Paul (2026-09-28)** : **10 tests**,
  écrits uniquement contre la surface générée (`DengonNodeInterface`,
  fonctions de premier niveau). +3 : QR non dengon (`https://example.com`) →
  `DengonException.Internal` ; pseudo de 260 octets tronqué comme en Rust ;
  8 threads × 400 appels concurrents sur le bouchon. `tests="10" skipped="0"
  failures="0" errors="0"`, `assembleDebug` vert.
- **Kotlin, historique** (`cd android && ./gradlew testDebugUnitTest`) : 7 tests dans
  `DengonNodeStubTest` — conversation canned visible sans appel préalable,
  envoi de message + événement `PeerConnected`, `pollEvents` ne renvoie
  chaque événement qu'une fois, aller-retour QR, symétrie du code de
  vérification, et (ajouté le 2026-09-25 suite à la revue PR #69)
  `fromQrCode` lève `DengonException` sur un payload tronqué au lieu de
  planter ; le 2026-09-28, un 7ᵉ test sur un payload **non vide mais
  tronqué** (octet `pseudo_len` seul, payload valide amputé d'un octet) qui
  exerce la seconde garde (taille), la première ne couvrant que le payload
  vide. **Réellement exécutés** : `BUILD SUCCESSFUL`,
  `tests="7" skipped="0" failures="0" errors="0"`
  (`app/build/test-results/testDebugUnitTest/TEST-com.dengon.app.ffi.DengonNodeStubTest.xml`).
  `./gradlew assembleDebug` passe aussi (pas de régression sur le reste de
  l'app).

## Limites connues / TODO

- Aucun branchement réel sur `dengon-core` : tout est en mémoire, canned. Le
  vrai FFI (bindings UniFFI générés, remplaçant le bouchon Kotlin) arrive
  avec US-302 ; l'API réelle (`send_message`/`poll_events`/... branchés sur
  `sync`/`store`) arrive avec US-301.
- La génération Kotlin via `uniffi-bindgen` n'est pas intégrée au build
  (US-302). Elle a été lancée **à la main** le 2026-09-28 pour vérifier la
  forme du bouchon. Rien n'empêche encore une dérive si le `.udl` change avant
  l'US-302 : un job CI qui génère les bindings et compile le test contre eux
  la fermerait.
- **À trancher au point d'équipe avant le gel** (revue PR #69, points 3 à 5,
  reportés sur #6) : clés secrètes absentes du constructeur, forme de
  `on_peer_connected` et entrée des trames reçues, variante d'erreur « QR
  invalide » (et `Cancelled` sans méthode `cancel`).
- La cross-compilation vers les ABI Android (`cargo-ndk`, `jniLibs`) n'est
  toujours pas essayée.

## Pour l'oral

C'est le point de rencontre entre deux mondes. L'application se voit en
Kotlin, comme n'importe quelle application Android ; mais dès qu'il s'agit de
chiffrer ou de router un message, elle appelle du Rust. L'US-106 montre la
mécanique du **contrat gelé** : décrire l'interface une fois (`dengon.udl`),
la figer, et laisser deux équipes (UI Android, cœur Rust) avancer en
parallèle sur un bouchon partagé — sans que l'une attende que l'autre ait fini
sa moitié du système.
