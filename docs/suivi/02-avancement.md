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
| `dengon-core` | crate Rust : protocol, crypto, store, sync, ledger, observability | — | 0 % | — |
| `dengon-ble` | trait Transport + impl btleplug | — | 0 % | — |
| `dengon-node` | binaire CLI (nœud headless) | — | 0 % | — |
| `dengon-sim` | simulateur multi-nœuds | — | 0 % | — |
| `dengon-ffi` | bindings UniFFI | — | 0 % | — |
| App Android | Kotlin + Compose | — | 0 % | — |
| Firmware `dengon-relay` | ESP-IDF + NimBLE | — | 0 % | — |
| Dashboard `api` | FastAPI + SQLite + SSE (A-5) | — | 0 % | — |
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
| Workflow `core` (fmt, clippy, nextest, couverture) | dans la PR US-104, pas encore sur `main` | — |
| Workflow `dashboard` (ruff + pytest) | dans la PR US-110, pas encore sur `main` | — |
| Workflow `contracts` (ruff + validate fixtures) | dans la PR US-107, pas encore sur `main` | — |
| `.gitattributes` — `merge=union` sur les fichiers de suivi append | **présent** (US-115) | — |
| GitGuardian + SonarCloud | applications GitHub installées, statut sur chaque PR | — |
| Hook Conventional Commits | actif (`.githooks/commit-msg`) | — |
