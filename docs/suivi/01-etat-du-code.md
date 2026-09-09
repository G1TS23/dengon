# État du code — photo courante

> Réécrit à chaque session. Décrit ce qui **existe et fonctionne aujourd'hui**, pas
> ce qui est prévu. Dernière mise à jour : **2026-09-09**.

## Résumé

**Aucun code applicatif écrit sur `main`.** Le dépôt contient : la conception
([`docs/powl/`](../powl/), [`docs/synthese/`](../synthese/)), ce dossier de
suivi, un `.gitignore`, des hooks git (`.githooks/`) et, depuis le 09/09,
l'**outillage de processus** dans `.github/` (US-113).

Des PR sont en vol et ne sont pas comptées ci-dessous, puisqu'elles ne sont pas
encore sur `main` : **#57** (US-104 — workspace Cargo, six crates squelettes, CI
`core`), **#56** (US-109 — squelette Android), **#59** (US-110 — squelette
dashboard `api`), et la branche `contract/US-107-enveloppe-evenement` (US-107 —
contrat des événements + 20 fixtures golden, décrit dans
[`modules/contracts-events.md`](modules/contracts-events.md)).

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
| Dashboard `api` | Axum + Postgres/Timescale | — | 0 % | — |
| Dashboard `web` | React + TS | — | 0 % | — |
| Déploiement VPS | docker-compose + Caddy + Mosquitto | — | 0 % | — |

## Outillage et processus

| Élément | Réel | Fiche |
|---|---|---|
| Formulaires d'issue (`user-story`, `spike`, `bug`) | présents, non encore actifs (GitHub ne les lit que depuis `main`) | [processus-github.md](modules/processus-github.md) |
| Template de PR (DoD §7.1 + §7.2) | présent | idem |
| `CODEOWNERS` — revue croisée | présent ; **suggère** un relecteur, ne le **bloque** pas encore | idem |
| `labels.yml` — 32 labels versionnés | présent, 0 écart avec GitHub | idem |
| Workflow `labels` (synchro manuelle) | présent, lançable après merge | idem |
| **Protection de `main`** | **active depuis le 09/09 15:18** (posée par `G1TS23`) : 1 approbation, `core` en check requis, approbations invalidées à chaque push. Posée avant le merge de #57 : #56 et #58 sont bloquées sur un `core` que rien ne rapporte. | idem |
| Squash-only + suppression auto des branches | **déjà actifs** au niveau du dépôt, avant l'US-113 | idem |
| Workflow `core` (fmt, clippy, nextest, couverture) | dans la PR #57, pas sur `main` | — |
| GitGuardian + SonarCloud | applications GitHub installées, rapportent un statut sur chaque PR (vertes sur #58) | — |
| Hook Conventional Commits | actif (`.githooks/commit-msg`) | — |

## Ce qui tourne / commandes utiles

_(à remplir quand il y aura du code : comment builder, lancer, tester chaque partie)_

En attendant, la vérification de l'outillage :

```bash
# Les labels du dépôt correspondent-ils au fichier versionné ?
gh label list --limit 100 --json name,color,description

# Où en est la protection de main ?
# ⚠ Depuis un compte NON ADMIN, cet endpoint renvoie 404 que la protection
#   existe ou non : son 404 ne prouve rien.
gh api repos/G1TS23/dengon/branches/main/protection

# Contrôle fiable sans droit admin : l'état de fusion d'une PR.
gh pr view <n> --json mergeStateStatus,statusCheckRollup
```

## Prochaines étapes

Voir [`docs/powl/10-mvp-scope-roadmap.md`](../powl/10-mvp-scope-roadmap.md) — **Lot 0** :
1. Spike A — `dengon-core` (protocol + crypto) cross-compile pour xtensa-esp32 ?
2. Spike B — `btleplug` en rôle GATT peripheral sous Linux.
3. Spike C — Android `BluetoothGattServer` + scan en foreground service.
