# Avancement — par composant & outillage

> **Édité en place.** On modifie **uniquement la ou les lignes du composant
> touché** — on ne réécrit jamais le fichier entier. C'est ce qui permet à deux
> PR concurrentes de ne pas se marcher dessus (les lignes changées sont
> différentes → fusion automatique ; `merge=union` sert de filet).
>
> Ne **pas** lister ici les PR en vol : c'est le rôle du board GitHub et de
> `gh pr list`, toujours à jour. Ici, on décrit ce qui est **sur `main`**.

## Avancement par composant

| Composant | Prévu (conception) | Réel | Avancement | Fiche |
|---|---|---|---|---|
| `dengon-core` | crate Rust : protocol, crypto, store, sync, ledger, observability | squelette + `protocol::{consts, types}` : 13 types de paquets (INVENTORY = 0x0D), Flags, Header, ~35 constantes, vecteurs de conformité v0 (US-108, PR #63) | ~12 % | [dengon-core](modules/dengon-core.md) |
| `dengon-ble` | trait Transport + impl btleplug | squelette : aucune API, 1 test de liaison | 3 % | [dengon-ble](modules/dengon-ble.md) |
| `dengon-core` | crate Rust : protocol, crypto, store, sync, ledger, observability | squelette : bascule `no_std`, `PROTOCOL_VERSION`, 2 tests | 5 % | [dengon-core](modules/dengon-core.md) |
| `dengon-ble` | trait Transport + impl btleplug | **contrat livré** (US-105) : `Transport`, `MockTransport`, suite de conformité (12 cas). Pas d'implémentation radio | 40 % | [dengon-ble.md](modules/dengon-ble.md) |
| `dengon-node` | binaire CLI (nœud headless) | squelette : `main` affiche sa version | 3 % | [dengon-node](modules/dengon-node.md) |
| `dengon-sim` | simulateur multi-nœuds | squelette : lib + bin, graine fixe déclarée | 3 % | [dengon-sim](modules/dengon-sim.md) |
| `dengon-verify` | binaire de vérif de journal chaîné (A-5 / B-5) | squelette : enum `Verdict` (Ok/Broken/Fork/Gap) | 3 % | [dengon-verify](modules/dengon-verify.md) |
| `dengon-ffi` | bindings UniFFI | squelette : `lib` + `cdylib`, `version()` | 3 % | [dengon-ffi](modules/dengon-ffi.md) |
| App Android | Kotlin + Compose | squelette : Compose + `MeshForegroundService` (`foregroundServiceType="connectedDevice"`) + permissions BLE à l'exécution + notification permanente. Pas de vraie logique BLE (GATT). | 15 % (US-109 fait ; US-213/214/215 restent) | [android-app](modules/android-app.md) |
| Firmware `dengon-relay` | ESP-IDF + NimBLE | squelette ESP-IDF v5.5 : annonce le service `dengon` (UUID 128 bits + manufacturer data `peerID`‖flags, nom en réponse de scan), table GATT `CHAR_RX` (write sans réponse) / `CHAR_TX` (notify) déclarée, ATT MTU 517 demandé. `peerID` bouchonné sur la MAC eFuse. Pas de scan/central, pas de `dengon_core_ffi`, pas de routage, pas de Wi-Fi. | 10 % (US-114 fait ; US-220/307/308/309 restent) | [firmware-relay](modules/firmware-relay.md) |
| Dashboard `api` | FastAPI + SQLite + SSE (A-5) | — | 0 % | — |
| Dashboard `web` | page légère + SSE (A-5) | **squelette livré** (US-111) : page statique liste + détail (timeline des sauts), données bidon, aucun appel réseau, rendu vérifié à 360 px. Pas d'API ni de SSE (US-217/US-219) | 15 % | [dashboard-web](modules/dashboard-web.md) |
| Contrats (`contracts/`) | schémas d'événements + fixtures golden | — | 0 % | — |
| Déploiement VPS | reverse-proxy TLS + `uvicorn` (A-5 / A-6) | — | 0 % | — |

## Outillage et processus

| Élément | Réel | Fiche |
|---|---|---|
| Formulaires d'issue (`user-story`, `spike`, `bug`) | présents, non encore actifs (GitHub ne les lit que depuis `main`) | [processus-github.md](modules/processus-github.md) |
| Template de PR (DoD §7.1 + §7.2) | présent | idem |
| `CODEOWNERS` — revue croisée | présent ; **suggère** un relecteur, ne le **bloque** pas encore | idem |
| `labels.yml` — labels versionnés | présent, 0 écart avec GitHub | idem |
| Workflow `labels` (synchro manuelle) | présent, lançable après merge | idem |
| **Protection de `main`** | active depuis le 09/09 15:18 (`G1TS23`) : 1 approbation, `core` en check requis, approbations invalidées à chaque push | idem |
| Squash-only + suppression auto des branches | actifs au niveau du dépôt | idem |
| Workflow `core` (fmt, clippy, nextest, no_std, couverture) | **présent** (US-104). Filtre les chemins **dans le job**, donc `core` est rapporté sur **toutes** les PR, y compris documentaires | — |
| Workflow `firmware` (build ESP-IDF) | **présent** (US-114). Compile via l'image Docker `espressif/idf:v5.5.5` épinglée par digest, la même qu'en local. Filtre les chemins **dans le job**, comme `core`. Publie le binaire en artefact (permet de flasher depuis Windows sans rejouer le build). **Pas encore** dans les checks requis de `main`. | [firmware-relay](modules/firmware-relay.md) |
| Workflow `dashboard` (ruff + pytest) | dans la PR US-110, pas encore sur `main` | — |
| Workflow `contracts` (ruff + validate fixtures) | dans la PR US-107, pas encore sur `main` | — |
| Workspace Cargo + `rustfmt` / `clippy` + toolchain figée (1.98.1) | **présent** (US-104) : `Cargo.toml` et `rust-toolchain.toml` à la racine, `rustfmt.toml` et `clippy.toml` dans `crates/`, `Cargo.lock` versionné | — |
| `.gitattributes` — `merge=union` sur les fichiers de suivi append | **présent** (US-115) | — |
| GitGuardian + SonarCloud | applications GitHub installées, statut sur chaque PR | — |
| Hook Conventional Commits | actif (`.githooks/commit-msg`) | — |
| Toolchain Xtensa (`espup`, cible `xtensa-esp32-none-elf`) | validée par le **Spike A** le 10/09 : les briques crypto compilent en `no_std` — B-1 tranchée « tout en Rust » | [spikes/US-101](spikes/US-101-cross-compile-xtensa.md) |
