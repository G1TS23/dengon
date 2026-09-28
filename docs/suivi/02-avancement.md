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
| `dengon-core` | crate Rust : protocol, crypto, store, sync, ledger, observability | squelette + `protocol::{consts, types}` (US-108, PR #63) : 13 types de paquets (INVENTORY = 0x0D), Flags, Header, ~35 constantes, vecteurs de conformité v0 + `ledger` livré (US-206, PR #75) : append/verify_chain/export, signature différée (`Signer` injecté), 16 tests dont 2 property tests + `store` livré (US-207, PR #76) : schéma SQLite complet, chiffrement XChaCha20-Poly1305 champ par champ + AAD, migrations rejouables, 13 tests dont le test négatif (grep binaire) + **`crypto` Ed25519 sign/verify** (US-203, PR #78) : KAT RFC 8032 + négatifs, `no_std` vérifié sur thumbv7em, `SigningKey` branché sur `ledger::Signer` + **`crypto` Noise `XX`/`X`, `recipient_tag`, padding `PAD_BUCKETS`** (US-204, PR #81) : `snow` 0.10, transport sans état tolérant aux pertes, handshake non paddé (192 o) et message 1 sans payload après revue, vecteurs recoupés en Python + **`protocol::codec`** (US-201, PR #80) : encode/decode L3 et frames L4 `Message`/`Ack`, entrée de signature TTL à 0, property tests, vecteurs v0 décodés + **`identity`** (US-205) : keypair X25519 + Ed25519, `peerID`/empreinte, QR `dengon:v1:` (aller-retour strict), code de vérification 60 chiffres symétrique, coffre XChaCha20-Poly1305 derrière un trait `Vault` (clé fournie par l'appelant) + **`sync::status`** (US-211, PR #84) : machine à états MVP sans `READ`, outbox persistante (trait `OutboxStore`), rejeu après redémarrage, 33 tests dont 3 property tests + **`sync::routing`** (US-209, PR #85) : routeur sans-IO (TTL, dédup, jitter, clamp densité, quota par lien, anti-inondation par `peerID` du voisin, pas de relais au-delà de l'horizon du seen-set, horloges murale + monotone), 43 tests unitaires dont 2 property tests + 8 tests de bout en bout contre `MockTransport` ; inondation 500 msg/s × 60 s → 20 relais ; couverture du module 99 % + **`protocol::fragment`** (US-202, PR #88) : fragmentation / réassemblage L2 selon le MTU, mémoire bornée face à un pair malveillant, 23 tests dont 4 property tests + **`observability`** (US-208, PR #89) : `Envelope`, JSON canonique octet-à-octet contre 3 fixtures golden, redaction structurelle (`msg_log_id`), 12 tests + **`sync::courier`** (US-212) : dépôt / collecte d'enveloppes scellées, expiration, magasin borné (`ENVELOPE_STORE_MAX`, politique d'éviction explicite), 15 tests dont 3 property tests et le test négatif de lecture | 63 % | [dengon-core](modules/dengon-core.md) |
| `dengon-ble` | trait Transport + impl btleplug | **contrat livré** (US-105) : `Transport`, `MockTransport`, suite de conformité (12 cas). Pas d'implémentation radio | 40 % | [dengon-ble.md](modules/dengon-ble.md) |
| `dengon-node` | binaire CLI (nœud headless) | squelette : `main` affiche sa version | 3 % | [dengon-node](modules/dengon-node.md) |
| `dengon-sim` | simulateur multi-nœuds | harness N nœuds, `SimTransport` conforme au contrat `Transport`, réseau scriptable (latence, gigue, perte, partition), 4 scénarios RON déterministes, job CI `sim` (US-221). Nœuds = relais de démo, pas encore `dengon-core` | 35 % | [dengon-sim](modules/dengon-sim.md) |
| `dengon-verify` | binaire de vérif de journal chaîné (A-5 / B-5) | squelette : enum `Verdict` (Ok/Broken/Fork/Gap) | 3 % | [dengon-verify](modules/dengon-verify.md) |
| `dengon-ffi` | bindings UniFFI | **contrat v0 gelé** (US-106) : `dengon.udl` + scaffolding UniFFI 0.28.3, implémentation bouchon en mémoire (Rust) + bouchon Kotlin miroir. Pas branché sur `dengon-core` (US-301/US-302) | 25 % | [dengon-ffi](modules/dengon-ffi.md) |
| App Android | Kotlin + Compose | squelette : Compose + `MeshForegroundService` (`foregroundServiceType="connectedDevice"`) + permissions BLE à l'exécution + notification permanente. Messagerie Compose sur bouchon FFI : liste, fil, saisie, statuts, ViewModel testé (US-214). Pas de vraie logique BLE (GATT). | 25 % (US-109, US-214 faits ; US-213/215 restent) | [android-app](modules/android-app.md) |
| App Android | Kotlin + Compose | squelette : Compose + `MeshForegroundService` (`foregroundServiceType="connectedDevice"`) + permissions BLE à l'exécution + notification permanente. Pas de vraie logique BLE (GATT). + **écran d'appairage QR + code 60 chiffres** (US-215, sur bouchon FFI, vérifié sur Pixel 8 Pro + Galaxy A16) | 30 % (US-109, US-215 faits ; US-213/214 restent) | [android-app](modules/android-app.md) |
| Firmware `dengon-relay` | ESP-IDF + NimBLE | — | 0 % | — |
| Dashboard `api` | FastAPI + SQLite + SSE (A-5) | `/healthz` + `/ingest/batch` **validé** (schéma + JWT + signature Ed25519 + dédup `event_id`, `node_id`/`node_kind` par événement vérifiés, US-216, PR #91 mergée) + `/api/nodes` (enregistrement, pas d'upsert sur `node_id` existant → 409) + projection `messages` (reconstruction de statut, US-217, PR #93 en revue), 62 tests — squelette permissif de l'US-110 remplacé | ~40 % | [dashboard-api](modules/dashboard-api.md) |
| Dashboard `api` | FastAPI + SQLite + SSE (A-5) | `/healthz` + `/ingest/batch` **validé** (schéma + JWT + signature Ed25519 + dédup `event_id`, US-216, PR #91) + `/api/nodes` (enregistrement) + projection `messages` (reconstruction de statut, US-217, PR #93) + `GET /api/stream` en SSE (rattrapage + diffusion live, US-218, PR à ouvrir), 65 tests — squelette permissif de l'US-110 remplacé | ~50 % | [dashboard-api](modules/dashboard-api.md) |
| Dashboard `web` | page légère + SSE (A-5) | — | 0 % | — |
| Firmware `dengon-relay` | ESP-IDF + NimBLE | squelette ESP-IDF v5.5 (US-114) + **transport NimBLE** (US-220) : rôle GATT double (annonce + scan, serveur + client), règle anti-boucle sur `peerID[0..4]`, découverte + abonnement côté central, octets opaques (1 trame = 1 PDU ATT, ≤ MTU-3), événements et erreurs miroirs du contrat `Transport`, réannonce après coupure brutale, quota 3 liens. Cœur pur `dengon_transport_core` : 32 tests Unity sur l'hôte (dont les 12 cas de conformité portés 1:1). Tâche de démo. Vérifié sur carte : 32 tests Unity, rôle périphérique contre un téléphone (trames, MTU 517, déconnexion propre, coupure brutale réelle `0x08`, réannonce). **Essai sur 2 cartes (rôle central) pas encore fait.** `peerID` toujours bouchonné ; pas de `dengon_core_ffi`, pas de routage, pas de Wi-Fi. | 25 % (US-114, US-220 codés ; essai matériel US-220 + US-307/308/309 restent) | [firmware-relay](modules/firmware-relay.md) |
| Dashboard `api` | FastAPI + SQLite + SSE (A-5) | — | 0 % | — |
| Dashboard `web` | page légère + SSE (A-5) | — | 0 % | — |
| Contrats (`contracts/`) | schémas d'événements + fixtures golden | événements : envelope/batch/payloads schema + CANONICAL.md + 20 fixtures signées + validate.py (US-107, PR #60) | ~15 % | [contracts-events](modules/contracts-events.md) |
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
| Workflow `firmware` (build ESP-IDF) | **présent** (US-114). Compile via l'image Docker `espressif/idf:v5.5.5` épinglée par digest, la même qu'en local. Filtre les chemins **dans le job**, comme `core`. **Lance les 32 tests Unity du transport sur la cible `linux`** et compile leur version carte (US-220). Publie le binaire en artefact (permet de flasher depuis Windows sans rejouer le build). **Pas encore** dans les checks requis de `main`. | [firmware-relay](modules/firmware-relay.md) |
| Workflow `dashboard` (ruff + pytest) | dans la PR US-110, pas encore sur `main` | — |
| Workflow `contracts` (ruff + validate fixtures) | dans la PR US-107, pas encore sur `main` | — |
| Workspace Cargo + `rustfmt` / `clippy` + toolchain figée (1.98.1) | **présent** (US-104) : `Cargo.toml` et `rust-toolchain.toml` à la racine, `rustfmt.toml` et `clippy.toml` dans `crates/`, `Cargo.lock` versionné | — |
| `.gitattributes` — `merge=union` sur les fichiers de suivi append | **présent** (US-115) | — |
| GitGuardian + SonarCloud | applications GitHub installées, statut sur chaque PR | — |
| Hook Conventional Commits | actif (`.githooks/commit-msg`) | — |
| Toolchain Xtensa (`espup`, cible `xtensa-esp32-none-elf`) | validée par le **Spike A** le 10/09 : les briques crypto compilent en `no_std` — B-1 tranchée « tout en Rust » | [spikes/US-101](spikes/US-101-cross-compile-xtensa.md) |
| Backend BLE desktop (`dengon-node`) | **Spike B** le 25/09 (recherche documentaire) : `btleplug` est *central-only* sur tous les OS — repli `bluer` (Linux) recommandé, B-6 **à ratifier** ; à revérifier sur Linux réel à l'US-303 | [spikes/US-102](spikes/US-102-btleplug-peripheral.md) |
