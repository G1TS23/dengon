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
| `dengon-core` | crate Rust : protocol, crypto, store, sync, ledger, observability | squelette : bascule `no_std`, `PROTOCOL_VERSION`, 2 tests | 5 % | [dengon-core](modules/dengon-core.md) |
| `dengon-ble` | trait Transport + impl btleplug | squelette : aucune API, 1 test de liaison | 3 % | [dengon-ble](modules/dengon-ble.md) |
| `dengon-node` | binaire CLI (nœud headless) | squelette : `main` affiche sa version | 3 % | [dengon-node](modules/dengon-node.md) |
| `dengon-sim` | simulateur multi-nœuds | squelette : lib + bin, graine fixe déclarée | 3 % | [dengon-sim](modules/dengon-sim.md) |
| `dengon-verify` | binaire de vérif de journal chaîné (A-5 / B-5) | squelette : enum `Verdict` (Ok/Broken/Fork/Gap) | 3 % | [dengon-verify](modules/dengon-verify.md) |
| `dengon-ffi` | bindings UniFFI | squelette : `lib` + `cdylib`, `version()` | 3 % | [dengon-ffi](modules/dengon-ffi.md) |
| App Android | Kotlin + Compose | — | 0 % | — |
| Firmware `dengon-relay` | ESP-IDF + NimBLE | — | 0 % | — |
| Dashboard `api` | FastAPI + SQLite + SSE (A-5) | squelette : `/healthz` + `/ingest/batch` permissif, migrations, 8 tests (US-110, PR #59) | ~10 % | [dashboard-api](modules/dashboard-api.md) |
| Dashboard `web` | page légère + SSE (A-5) | — | 0 % | — |
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
| Workflow `dashboard` (ruff + pytest) | dans la PR US-110, pas encore sur `main` | — |
| Workflow `contracts` (ruff + validate fixtures) | dans la PR US-107, pas encore sur `main` | — |
| Workspace Cargo + `rustfmt` / `clippy` + toolchain figée (1.98.1) | **présent** (US-104) : `Cargo.toml` et `rust-toolchain.toml` à la racine, `rustfmt.toml` et `clippy.toml` dans `crates/`, `Cargo.lock` versionné | — |
| `.gitattributes` — `merge=union` sur les fichiers de suivi append | **présent** (US-115) | — |
| GitGuardian + SonarCloud | applications GitHub installées, statut sur chaque PR | — |
| Hook Conventional Commits | actif (`.githooks/commit-msg`) | — |
