# Module : `dengon-ffi` (`crates/dengon-ffi/`)

**Rôle en une phrase :** le pont qui permet à l'application Android, écrite en Kotlin, d'appeler le cœur écrit en Rust.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §3 et §5 ; [`docs/powl/09-data-model.md`](../../powl/09-data-model.md) §1/§3 (formats QR, code de vérification).
**Dernière mise à jour :** 2026-09-20
**État :** **contrat v0 gelé** (US-106) — UDL + bouchon Kotlin. Pas encore branché sur `dengon-core` (US-301/US-302).

## ⚠️ Vérification côté Rust — non faite dans cette session

Cette crate ajoute une dépendance externe réelle (`uniffi`) pour la première
fois du workspace. L'environnement où l'US-106 a été codée **n'a aucun
toolchain Rust installé** (`cargo`/`rustc` absents) — contrainte dure de la
session, comme l'absence d'appareil Android pour le Spike C (US-103).
Conséquences concrètes :

- `Cargo.lock` **n'a pas été régénéré**. `cargo build --locked` (job `core`
  de la CI) va très probablement échouer avec *"the lock file … needs to be
  updated but --locked was passed"* — c'est un échec attendu, pas une
  surprise à corriger en catastrophe. **Prochaine étape obligatoire avant
  merge** : quelqu'un avec `cargo` installé lance `cargo build --workspace`
  une fois à la racine, ce qui régénère `Cargo.lock`, puis commit/push.
- `cargo fmt --check` et `cargo clippy -D warnings` n'ont **pas** pu être
  exécutés sur `crates/dengon-ffi/`. Le code a été écrit et relu à la main en
  visant `crates/rustfmt.toml` (max_width 100, etc.) et
  `[workspace.lints]` (pas d'`unwrap`/`expect` hors test, `Debug` sur tout
  type public, etc.), mais rien ne remplace un run réel.
- L'API exacte d'UniFFI `0.28.3` (version choisie *volontairement* : plus
  ancienne et beaucoup mieux documentée dans les sources publiques que les
  versions `0.31`/`0.32` disponibles sur crates.io en 2026, dont
  l'architecture a changé — « pipeline ») a été utilisée de mémoire
  (`uniffi::generate_scaffolding`, `uniffi::include_scaffolding!`), sans
  pouvoir vérifier contre la doc réelle de cette version précise.

Côté Kotlin en revanche, tout est **réellement vérifié** (JDK + Gradle
disponibles) : voir « Tests » plus bas.

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
  src/
    dengon.udl    — contrat v0 : Identity, Message, Conversation, NodeEvent,
                    DengonError, interface DengonNode, fonctions identité/QR
    lib.rs        — uniffi::include_scaffolding!("dengon") + implémentation
                    bouchon (DengonNode en mémoire, génération d'identité
                    placeholder, QR base64url, code de vérification placeholder)

android/app/src/main/java/com/dengon/app/ffi/
  DengonTypes.kt      — miroirs Kotlin des types du .udl
  DengonNodeStub.kt   — interface DengonNode + implémentation canned
                        (DengonNodeStub) + objet DengonIdentity (génération,
                        QR, code de vérification)
android/app/src/test/java/com/dengon/app/ffi/
  DengonNodeStubTest.kt — 5 tests JVM purs (voir « Tests »)
```

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `dengon.udl` | `src/dengon.udl` | Contrat gelé : `dictionary Identity/Message/Conversation`, `enum MessageStatus`, `[Enum] interface NodeEvent`, `[Error] enum DengonError`, `interface DengonNode` (constructor + 5 méthodes), fonctions libres `generate_identity`/`identity_qr_code`/`identity_from_qr_code`/`verification_code`. |
| `DengonNode` (Rust) | `src/lib.rs:138` | Bouchon en mémoire : `Mutex<NodeState>` (messages, conversations, événements en attente, pairs connectés). `send_message` toujours `Ok` ; le vrai routage/erreurs arrivent avec `sync` (US-301). |
| `generate_identity`/`identity_qr_code`/`identity_from_qr_code`/`verification_code` | `src/lib.rs:230-329` | Placeholders **non cryptographiques** : pas de vraies clés X25519/Ed25519 (pas de module `crypto`/`identity` avant US-108/US-203/US-205). Fixent la FORME du contrat (types, format `dengon:v1:…`, 12 groupes de 5 chiffres), pas le contenu. |
| `encode_base64url`/`decode_base64url` | `src/lib.rs:339-393` | base64url (RFC 4648 §5) sans padding, écrit à la main : `dengon-ffi` n'a qu'`uniffi` comme dépendance externe, pas de crate `base64`. |
| `DengonNode` (Kotlin, interface) | `ffi/DengonNodeStub.kt:11` | Même forme que l'interface UDL. `DengonNodeStub` l'implémente avec des données canned, y compris une conversation pré-remplie (critère DoR « un test affiche une conversation »). |
| `DengonIdentity` (Kotlin) | `ffi/DengonNodeStub.kt:~104` | Miroir Kotlin des fonctions libres. Base64url et code de vérification codés à la main **aussi** côté Kotlin — voir « Décisions d'implémentation ». |

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
- **`Identity` (Kotlin) n'est pas une `data class`** : elle contient des
  `ByteArray`, dont l'égalité structurelle par défaut compare l'identité de
  l'objet, pas le contenu. `equals`/`hashCode` sont réécrits à la main
  (`contentEquals`/`contentHashCode`).

## Tests

- **Rust** (`cargo test -p dengon-ffi`) : 5 tests dans `src/lib.rs` — version,
  aller-retour base64url, aller-retour QR, symétrie du code de vérification,
  `send_message`/`poll_events`/`on_peer_connected`. **Non exécutés dans cette
  session** (pas de `cargo` disponible) — voir l'avertissement en tête de
  fiche.
- **Kotlin** (`cd android && ./gradlew testDebugUnitTest`) : 5 tests dans
  `DengonNodeStubTest` — conversation canned visible sans appel préalable,
  envoi de message + événement `PeerConnected`, `pollEvents` ne renvoie
  chaque événement qu'une fois, aller-retour QR, symétrie du code de
  vérification. **Réellement exécutés** : `BUILD SUCCESSFUL`,
  `tests="5" skipped="0" failures="0" errors="0"`
  (`app/build/test-results/testDebugUnitTest/TEST-com.dengon.app.ffi.DengonNodeStubTest.xml`).
  `./gradlew assembleDebug` passe aussi (pas de régression sur le reste de
  l'app).

## Limites connues / TODO

- **`Cargo.lock` non régénéré** — voir l'avertissement en tête de fiche.
  Bloquant pour la CI `core` tant que quelqu'un avec `cargo` ne l'a pas
  régénéré.
- `cargo fmt`/`cargo clippy -D warnings` non exécutés sur `dengon-ffi` dans
  cette session.
- Aucun branchement réel sur `dengon-core` : tout est en mémoire, canned. Le
  vrai FFI (bindings UniFFI générés, remplaçant le bouchon Kotlin) arrive
  avec US-302 ; l'API réelle (`send_message`/`poll_events`/... branchés sur
  `sync`/`store`) arrive avec US-301.
- Pas de génération Kotlin réelle via `uniffi-bindgen` (feature `"cli"` non
  activée, volontairement hors périmètre de l'US-106).
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
