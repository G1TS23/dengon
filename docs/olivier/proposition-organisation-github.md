# Proposition — organisation GitHub, backlog et répartition

> Rédigé le 2026-09-09 par Olivier. **Proposition à valider en réunion.**
> Dérivé de [`../synthese/`](../synthese/) (conception figée) et de
> [`../plan-mvp.md`](../plan-mvp.md) (plan par pistes).
> Objectif : passer d'un plan « par personne » à un backlog **d'US
> interchangeables**, organisé en milestones / sprints / issues sur GitHub,
> avec DoR, DoD et dépendances explicites.

---

## 1. État des lieux

La conception est **solide et figée** : 12 fichiers dans `docs/synthese/`,
26 décisions référencées (A-1 → C-12), un format de trame arrêté, une
Definition of Done du MVP en 7 points, 8 lots de livraison, une stratégie de
test/CI complète. Le dépôt GitHub existe (`G1TS23/dengon`), le hook
Conventional Commits tourne. Il n'y a **pas encore de code**.

`plan-mvp.md` découpe le travail en **3 pistes**, une par personne :

| Piste | Personne | Périmètre |
| --- | --- | --- |
| P1 | Paul | Cœur Rust + firmware ESP32 |
| P2 | Tanguy | App Android (Kotlin + UniFFI) |
| P3 | Olivier | Dashboard + spec + rapport |

**Échéance : soutenance le 29/09/2026.** Aujourd'hui 09/09 → il reste
**14 jours ouvrés**, dont ~5 sur le sprint 1 déjà entamé.

---

## 2. Diagnostic : le plan actuel est verrouillé par personne

C'est un découpage **par composant**, donc **par compétence**. Il est efficace
sur le papier (3 fronts en parallèle) mais il produit exactement ce que vous
voulez éviter :

| Problème | Conséquence concrète |
| --- | --- |
| **Bus factor = 1 partout** | Paul absent 3 jours → le cœur Rust **et** le firmware s'arrêtent. Personne d'autre ne peut reprendre. |
| **Chemin critique mono-personne** | `plan-mvp.md` §7 le dit lui-même : « Si P1 prend du retard, **tout** glisse ». Une seule personne porte le risque de tout le projet. |
| **US non prenables** | Après la S1, une issue « implémenter `sync::routing` » n'est prenable que par Paul. Le backlog n'est pas un backlog, c'est trois todo-lists. |
| **Charge non lissable** | Si Tanguy finit sa S2 en avance, il ne peut pas aider : rien dans le backlog de Paul ne lui est accessible. |
| **Apprentissage cloisonné** | Pour une soutenance de M2, chacun ne pourra présenter qu'un tiers du projet. |

Le paradoxe : la conception a fait le bon choix technique (**un seul cœur
Rust**, A-2) pour éviter la dérive entre implémentations — mais ce choix
concentre 60 % de la valeur dans le langage le moins accessible de la stack.

---

## 3. Le principe qui débloque : contrats d'abord, bouchons partout

Trois mécanismes, appliqués ensemble, rendent les US indépendantes.

### 3.1 Les contrats livrent du **code compilable**, pas de la prose

`plan-mvp.md` §6 identifie déjà les 5 coutures à figer. La proposition : les
transformer en **issues de sprint 1 qui livrent des artefacts exécutables**.

| Contrat | Livre (pas seulement un document) |
| --- | --- |
| `trait Transport` | le trait Rust **+ `MockTransport` en mémoire** |
| Surface `dengon-ffi` | le fichier UDL **+ un bouchon Kotlin qui renvoie des données canned** |
| Enveloppe d'événement + `/ingest/batch` | le schéma JSON **+ 20 fixtures golden signées** |
| Sous-ensemble MVP du format | **`protocol::types`** (enums, structs, constantes) **+ vecteurs de conformité v0** |

> **Le point clé** : `protocol::types` (les types) est livré **séparément** de
> `protocol::codec` (encode/decode). Du coup `sync::routing` peut être écrit
> contre les types sans attendre la sérialisation. C'est ce qui casse la
> dépendance la plus structurante du projet.

### 3.2 Règle d'or des dépendances

> **Une US ne dépend jamais d'une US du même sprint.**
> Toute dépendance pointe vers un sprint **antérieur**.

Si une US `A` a besoin de `B` dans le même sprint, on **scinde** :
`A₁` (ce qui se fait contre un bouchon, ce sprint) + `A₂` (le branchement réel,
sprint suivant). Conséquence directe : **le jour d'ouverture d'un sprint,
toutes ses issues sont démarrables**. C'est ça, « n'importe qui peut prendre
n'importe quelle US ».

### 3.3 Squelette au sprint N, conformité au sprint N+1

Corollaire pratique : le squelette dashboard accepte n'importe quel JSON en S1 ;
la **validation de schéma + vérification de signature** est une US distincte en
S2. Idem pour l'app (UI sur bouchon en S2, branchée sur le vrai FFI en S3).

---

## 4. Architecture des dépôts GitHub

### 4.1 Recommandation : **monorepo**, sans hésitation

Garder **un seul dépôt `G1TS23/dengon`**, avec le découpage interne déjà décidé
en A-14 / `synthese/04-architecture.md` §5.

| Argument | Détail |
| --- | --- |
| **Cohérence du cœur unique** | A-2 = « une seule implémentation du protocole, auditer une fois ». Séparer les dépôts réintroduit la dérive de version entre `dengon-core`, le firmware et l'app — exactement le risque que A-2 tue. |
| **Le job CI `cross-vectors`** | (`synthese/10` §4.7) compare core ↔ firmware ↔ dashboard sur les **mêmes vecteurs**. En multi-dépôts, il faut publier/pinner des artefacts à chaque changement : intenable en 3 semaines. |
| **Changements atomiques** | Modifier un type de paquet touche `crates/`, `firmware/` et `dashboard/` dans **une seule PR** revue d'un bloc. |
| **Coût polyrepo** | submodules ou versions épinglées, 5 CI à maintenir, releases à coordonner. Pur surcoût pour 3 personnes. |
| **Interchangeabilité** | Un seul `git clone`, un seul board, un seul backlog. Le multi-dépôts **renforcerait** le cloisonnement qu'on veut casser. |

**Contre-arguments honnêtes** : CI plus lourde (réglé par des workflows
filtrés par chemin, §4.3) ; historique mélangé (réglé par les scopes
Conventional Commits déjà en place : `feat(core):`, `feat(android):`…).

**Ne pas créer** de dépôt séparé pour le dashboard déployé : utiliser les
**GitHub Environments** (`vps-prod`) + un workflow de déploiement.

### 4.2 Arborescence cible (rappel A-14, inchangée)

```text
dengon/
├── .github/                    ← à créer (§4.3)
├── crates/                     dengon-core, -ble, -node, -sim, -verify, -ffi
├── android/                    app Kotlin + Compose
├── firmware/dengon-relay/      ESP-IDF + NimBLE
├── dashboard/{api,web}/        FastAPI + page légère
├── docs/                       powl/ oswin/ olivier/ synthese/ suivi/
└── Cargo.toml                  workspace
```

### 4.3 Ce qu'il faut ajouter dans `.github/`

```text
.github/
├── ISSUE_TEMPLATE/
│   ├── user-story.yml          champs : contexte, critères d'acceptation,
│   │                           DoR cochée, dépendances, area, estimation
│   ├── spike.yml               question posée, timebox, livrable = décision
│   ├── bug.yml
│   └── config.yml
├── PULL_REQUEST_TEMPLATE.md    checklist DoD + « closes #N » + doc suivi à jour
├── CODEOWNERS                  révision croisée obligatoire (§9.3)
├── labels.yml                  taxonomie (§5.3)
└── workflows/
    ├── core.yml        paths: crates/**            fmt, clippy -D, nextest, llvm-cov
    ├── sim.yml         paths: crates/**            scénarios dengon-sim (seed fixe)
    ├── audit.yml       cron quotidien + PR         cargo audit, cargo deny
    ├── android.yml     paths: android/**, crates/dengon-ffi/**
    ├── firmware.yml    paths: firmware/**
    ├── dashboard.yml   paths: dashboard/**         pytest, lint, build
    ├── cross-vectors.yml                           conformité core↔fw↔dash
    └── deploy-vps.yml  env: vps-prod, manuel
```

**Protection de `main`** : PR obligatoire, 1 approbation, **checks requis**
`core` + `sim` + `audit` + `cross-vectors` (conforme à `synthese/10` §4.7),
historique linéaire, squash merge, suppression auto des branches.

**Branches** : `main` protégée + branches courtes `type/US-XXX-slug`
(ex. `feat/US-209-sync-routing`). Pas de `develop` : inutile sur 3 semaines.

### 4.4 GitHub Projects (v2) — un seul board

Champs personnalisés à créer :

| Champ | Type | Valeurs |
| --- | --- | --- |
| `Sprint` | itération | S1 (08-14/09), S2 (15-21/09), S3 (22-28/09) |
| `Area` | select | core-rust, contract, android, firmware, dashboard-api, dashboard-web, docs, process |
| `Estimation` | nombre | points (1, 2, 3, 5) |
| `Priorité MoSCoW` | select | Must, Should, Could |
| `DoR` | select | ✅ prête / ⛔ pas prête |
| `Bloquée par` | texte | IDs des US |
| `Contrainte dure` | select | aucune, matériel-esp32, matériel-android, vps |

Vues : **Board par statut** (Backlog → DoR OK → En cours → En revue → Done),
**Table par sprint**, **Board par Area** (pour voir l'équilibrage),
**Vue « démarrables »** (filtre : sprint courant + DoR ✅ + pas de bloquant ouvert).

---

## 5. Milestones, sprints, labels

### 5.1 Milestones = les 6 jalons (repris de `plan-mvp.md` §5)

| Milestone | Échéance | Critère de clôture |
| --- | --- | --- |
| **J0 — Go/No-Go** | 14/09 | Spike A tranché ; Spike C réussi ; workspace + CI verts ; **4 contrats livrés avec bouchons** |
| **J1 — Cœur en simulation** | 18/09 | `dengon-sim` : `direct`, `multihop`, `recipient_offline`, `sender_offline`, `partition_merge` passent |
| **J2 — Deux téléphones parlent** | 21/09 | Scénario 1 du DoD sur 2 vrais téléphones (chiffré, statuts, QR + code 60 chiffres) |
| **J3 — Dashboard en ligne** | 21/09 | Déployé sur le VPS, ingère des batchs, reconstruit un statut, SSE OK |
| **J4 — MVP matériel** | 25/09 | Scénarios 2, 3, 4 du DoD sur vrai matériel |
| **J5 — Prêt soutenance** | 28/09 | Recette 9 points passée ; démo répétée **3× sans intervention** ; rapport + slides rendus |

### 5.2 Sprints

| Sprint | Dates | Thème | Invariant |
| --- | --- | --- | --- |
| **S1** | 08 → 14/09 | **Contrats, spikes, squelettes** | 0 dépendance intra-sprint |
| **S2** | 15 → 21/09 | **Construire en parallèle sur bouchons** | 0 dépendance intra-sprint |
| **S3** | 22 → 28/09 | **Intégrer, démontrer, rédiger** | sprint de convergence — adhérence assumée |

### 5.3 Labels

- `area:` core-rust · contract · android · firmware · dashboard-api ·
  dashboard-web · docs · process
- `type:` us · spike · bug · chore · ci
- `sprint:` s1 · s2 · s3
- `moscow:` must · should · could
- `skill:` rust-avancé · rust-débutant · kotlin · c-embarqué · python · web ·
  rédaction
- `needs:` materiel-esp32 · materiel-android · vps  ← **contraintes dures**
- `good-first-issue` · `blocked` · `contract` (= gèle une couture, ne se
  modifie qu'en réunion)

---

## 6. Definition of Ready (DoR)

Une issue entre dans un sprint **seulement si les 8 points sont cochés**.

| # | Critère | Pourquoi |
| --- | --- | --- |
| 1 | **Titre en forme d'US** : « En tant que X, je veux Y, afin de Z » — ou, pour une tâche technique, un livrable nommé sans ambiguïté | évite les issues fourre-tout |
| 2 | **Critères d'acceptation** listés, **vérifiables** (pas « ça marche ») | c'est ce que la revue vérifiera |
| 3 | **Référence documentaire** : lien vers la section de `docs/synthese/` qui fait foi | personne ne réinvente la spec |
| 4 | **Dépendances déclarées** et **toutes fermées** (règle d'or §3.2) | l'issue est démarrable *aujourd'hui* |
| 5 | **Contrat d'interface disponible** (trait, UDL, schéma JSON, types) ou bouchon fourni | pas d'attente d'une autre personne |
| 6 | **Estimation** en points (1/2/3/5) — si > 5, **scinder** | limite le WIP invisible |
| 7 | **Stratégie de test nommée** : quel test, à quel niveau (unitaire / property / sim / e2e), d'après `synthese/10` §4 | le test n'est pas une option |
| 8 | **Contrainte dure identifiée** (`needs:`) ou explicitement « aucune » | dit qui peut *physiquement* la prendre |

**Anti-critères** (une US n'est *pas* prête si) : elle dit « voir avec Paul » ;
elle dépend d'une décision non prise dans `01-sujets-a-trancher.md` ; elle
mélange deux `area:`.

---

## 7. Definition of Done (DoD)

### 7.1 DoD **globale** — toute US

1. Critères d'acceptation **tous** vérifiés, démontrés dans la PR.
2. **Tests** écrits au niveau annoncé en DoR, et **verts**.
3. **CI verte** sur les jobs concernés (`core`, `sim`, `audit`, `cross-vectors`
   obligatoires pour merger — `synthese/10` §4.7).
4. **Revue par une personne d'une autre `area:`** (CODEOWNERS, §9.3).
5. **Documentation de suivi à jour** : entrée `docs/suivi/00-journal.md`,
   `01-etat-du-code.md` rafraîchi, fiche module — c'est la règle `CLAUDE.md`,
   elle est **dans la DoD**, pas optionnelle.
6. Écart vs conception → consigné dans `docs/suivi/03-ecarts-conception.md`.
7. Commit **Conventional Commits** (hook actif), PR squashée, branche supprimée.
8. **Aucun TODO bloquant** laissé : soit fait, soit une issue de suite créée.

### 7.2 DoD **par type d'US**

| Type | Ajouts spécifiques |
| --- | --- |
| **Spike** | Timebox respectée. Livrable = **une décision écrite** (`docs/suivi/` + mise à jour de `synthese/01-sujets-a-trancher.md`), pas du code. Répond par oui/non à la question posée. Le code jetable est jeté. |
| **Contrat** | Artefact **compilable** livré (trait + mock, UDL + bouchon, schéma + fixtures, types + vecteurs). Label `contract` posé. Annoncé en point d'équipe : **il est gelé**. |
| **Module `dengon-core`** | Tests unitaires **+ property tests** là où `synthese/10` §4.2 les exige. `clippy -D warnings`. Couverture ≥ 85 % sur le module. Compile en `no_std` si le module est embarqué (`protocol`, `sync`, `ledger`). |
| **Transport (Android / NimBLE / btleplug)** | Passe la **suite de conformité `Transport`** partagée. Testé sur ≥ 2 appareils réels (Android) ou 2 cartes (ESP32). Comportement documenté en cas de déconnexion brutale. |
| **App Android** | Build `assembleDebug` + tests unitaires verts. Testé sur la **matrice d'appareils**. Pas de régression du service de fond (l'app relaie toujours écran éteint pendant ≥ 5 min). |
| **Firmware** | `idf.py build` propre. Tests host de `libdengon_core` verts. Tests Unity cible. **Vecteurs de conformité identiques au core**. Survit à `esp_restart()` sans rupture de chaîne de journal. |
| **Dashboard API** | `pytest` vert, base SQLite éphémère. **Test négatif de redaction** : aucun `msg_uuid`, texte ou destinataire en clair ne peut être stocké ni renvoyé. Idempotence sur `event_id` prouvée. |
| **Dashboard web** | Rendu correct sur mobile. Les écrans tiennent avec des données partielles (un nœud n'a pas remonté). |
| **Doc / rapport** | Relu par une autre personne. Aucun renvoi vers un fichier inexistant. Cohérent avec la table de décisions de `synthese/00`. |

### 7.3 DoD **de jalon** (milestone)

Un milestone se ferme quand : toutes ses US `must` sont Done · son critère de
clôture (§5.1) est **démontré en réunion**, pas déclaré · la CI est verte sur
`main` · `docs/suivi/01-etat-du-code.md` reflète la réalité.

### 7.4 DoD **du MVP**

Inchangée — c'est celle de `synthese/10-benchmarks-mvp-tests.md` §3.1
(7 points, sur vrai matériel). Elle est la **définition de fin de projet** ;
les DoD ci-dessus sont les paliers qui y mènent.

---

## 8. Le backlog — 55 US

Notation : **Deps** = US bloquantes · **Pts** = estimation ·
**M** = MoSCoW · **Dure** = contrainte matérielle.

### 8.1 Sprint 1 — Contrats, spikes, squelettes (14 US · 30 pts)

**Toutes démarrables immédiatement. Aucune dépendance.**

| ID | US | Area | Deps | Pts | M | Dure |
| --- | --- | --- | :---: | :---: | :---: | --- |
| US-101 | **Spike A** — `protocol`+`crypto` cross-compilent pour `xtensa-esp32-none-elf` ? → tranche B-1 | core-rust | — | 3 | Must | — |
| US-102 | **Spike B** — `btleplug` en rôle peripheral sur Linux | core-rust | — | 2 | Should | — |
| US-103 | **Spike C** — « hello mesh » Android : 2 téléphones échangent 20 o en foreground + **mesure du MTU réel** | android | — | 3 | Must | android |
| US-104 | Workspace Cargo + `rustfmt`/`clippy` + CI `core` | core-rust | — | 2 | Must | — |
| US-105 | **Contrat** `trait Transport` + `TransportEvent` + **`MockTransport` en mémoire** | contract | — | 3 | Must | — |
| US-106 | **Contrat** surface `dengon-ffi` v0 (UDL) + **bouchon Kotlin** (données canned) | contract | — | 3 | Must | — |
| US-107 | **Contrat** enveloppe d'événement + schéma `/ingest/batch` + **20 fixtures golden** | contract | — | 3 | Must | — |
| US-108 | **Contrat** `protocol::types` (enums, `Header`, `flags`, constantes) + sous-ensemble MVP + n° `INVENTORY` + **vecteurs v0** | contract | — | 3 | Must | — |
| US-109 | Squelette Android : Compose + `foregroundServiceType="connectedDevice"` + permissions + notification permanente | android | — | 2 | Must | — |
| US-110 | Squelette `dashboard/api` : FastAPI + migration SQLite + `/healthz` + `/ingest/batch` permissif | dashboard-api | — | 2 | Must | — |
| US-111 | Squelette `dashboard/web` : page statique, maquette liste + détail, données bidon | dashboard-web | — | 2 | Must | — |
| US-112 | **Delta doc sécurité** sur base `powl/04` (résultat Spike A, `recipient_tag`, colonnes chiffrées) | docs | — | 3 | Must | — |
| US-113 | `.github/` : templates issue/PR, labels, CODEOWNERS, protection de `main` | process | — | 2 | Must | — |
| US-114 | Squelette `firmware/dengon-relay` : ESP-IDF + NimBLE, advertise le service `dengon` | firmware | — | 3 | Should | esp32 |

### 8.2 Sprint 2 — Construire en parallèle (24 US · 62 pts)

**Toutes ne dépendent que de S1. Aucune dépendance intra-sprint.**

| ID | US | Area | Deps | Pts | M | Dure |
| --- | --- | --- | :---: | :---: | :---: | --- |
| US-201 | `protocol::codec` encode/decode + property `decode(encode(p))==p` | core-rust | 104,108 | 3 | Must | — |
| US-202 | `protocol` fragmentation / réassemblage L2 (MTU aléatoire) | core-rust | 104,108 | 3 | Must | — |
| US-203 | `crypto` : Ed25519 sign/verify + forge rejetée | core-rust | 101,104 | 2 | Must | — |
| US-204 | `crypto` : Noise `XX` + `X`, `recipient_tag`, padding `PAD_BUCKETS` | core-rust | 101,104 | 5 | Must | — |
| US-205 | `identity` : keypair, coffre, QR encode/decode, **code de vérification 60 chiffres** | core-rust | 104 | 3 | Must | — |
| US-206 | `ledger` : `append` / `verify_chain` / `export` + reprise après redémarrage | core-rust | 104 | 3 | Must | — |
| US-207 | `store` SQLite : schéma `synthese/09` §11.1 + migrations + chiffrement champ par champ | core-rust | 104 | 3 | Must | — |
| US-208 | `observability` : catalogue d'événements + JSON canonique + **redaction (test négatif)** | core-rust | 104,107 | 3 | Must | — |
| US-209 | `sync::routing` : pipeline TTL / dedup / jitter / clamp densité / quotas / anti-inondation | core-rust | 105,108 | 5 | Must | — |
| US-210 | `sync::inventory` : échange `INVENTORY` + push du manquant + re-dedup | core-rust | 105,108 | 3 | Must | — |
| US-211 | `sync::status` : machine à états MVP (sans `READ`) + outbox + rejeu | core-rust | 108 | 3 | Must | — |
| US-212 | `sync::courier` : dépôt / collecte d'enveloppes scellées, expiration | core-rust | 108 | 3 | Must | — |
| US-213 | `AndroidTransport` : GATT server + advertiser + scanner, implémente le contrat | android | 103,105,109 | 5 | Must | android |
| US-214 | UI Compose : conversations, fil, saisie, statuts — **branchée sur le bouchon FFI** | android | 106,109 | 3 | Must | — |
| US-215 | UI Compose : écran QR (affichage + scan) + comparaison du code 60 chiffres | android | 106,109 | 3 | Must | — |
| US-216 | Dashboard : validation de schéma + **signature Ed25519 des batchs** + JWT + dédup `event_id` | dashboard-api | 107,110 | 3 | Must | — |
| US-217 | Dashboard : projections + **reconstruction de statut** (`synthese/09` §10) sur fixtures | dashboard-api | 107,110 | 5 | Must | — |
| US-218 | Dashboard : `GET /api/stream` en **SSE** | dashboard-api | 110 | 2 | Must | — |
| US-219 | Web : écran **parcours d'un message** (timeline verticale) branché sur l'API | dashboard-web | 110,111 | 3 | Must | — |
| US-220 | Firmware : `transport_nimble.c` — relaie des octets opaques entre 2 cartes | firmware | 105,114 | 5 | Must | esp32 |
| US-221 | `dengon-sim` : harness N nœuds + transport mémoire scriptable (latence, perte, partition) | core-rust | 104,105 | 3 | Must | — |
| US-222 | CI : jobs `sim`, `audit`, `cross-vectors` + filtrage par chemin | process | 104,108,113 | 3 | Must | — |
| US-223 | **Rapport écrit** : plan détaillé + sections problème / état de l'art / conception | docs | 112 | 3 | Must | — |
| US-224 | Déploiement VPS : reverse-proxy TLS + `uvicorn` + script de purge par session | dashboard-api | 110 | 3 | Must | vps |

### 8.3 Sprint 3 — Intégrer, démontrer, rédiger (17 US · 63 pts)

**Sprint de convergence : l'adhérence y est irréductible et assumée.**

| ID | US | Area | Deps | Pts | M | Dure |
| --- | --- | --- | :---: | :---: | :---: | --- |
| US-301 | `api` façade `dengon-core` : `send_message`, `poll_events`, `on_peer_connected` | core-rust | 201-212 | 5 | Must | — |
| US-302 | `dengon-ffi` **réel** (UniFFI) — remplace le bouchon US-106 | core-rust | 106,301 | 5 | Must | — |
| US-303 | `dengon-node` CLI (transport `btleplug`) | core-rust | 102,301 | 3 | Should | — |
| US-304 | `dengon-sim` : les **5 scénarios du DoD** passent (`direct`, `multihop`, `recipient_offline`, `sender_offline`, `partition_merge`) | core-rust | 221,301 | 5 | Must | — |
| US-305 | `dengon-verify` : binaire `ok / broken / fork / gap` | core-rust | 206 | 3 | Must | — |
| US-306 | App branchée sur le **vrai** FFI → **scénario 1 du DoD** | android | 213,214,215,302 | 5 | Must | android |
| US-307 | `libdengon_core.a` : cross-compile xtensa + header via `cbindgen` | firmware | 101,209-212 | 5 | Must | — |
| US-308 | Firmware : tâches FreeRTOS route / inventory / courier / ledger + `Store` NVS + littlefs | firmware | 220,307 | 5 | Must | esp32 |
| US-309 | Firmware : client HTTPS + buffer ring + JWT + signature Ed25519 des batchs | firmware | 216,308 | 5 | Must | esp32 |
| US-310 | Dashboard : `GET /api/integrity` via `dengon-verify` + écran intégrité | dashboard-api | 216,305 | 3 | Must | — |
| US-311 | Web : carte réseau + flotte de relais + alerting → **scénario 4 du DoD** | dashboard-web | 217,219 | 5 | Must | — |
| US-312 | **Intégration E2E** app ↔ relais : scénarios 2 et 3 du DoD sur vrai matériel | android | 306,309 | 5 | Must | esp32+android |
| US-313 | App : écran réseau, mode éco, bouton « Renvoyer », statut « Échec » | android | 306 | 3 | Should | — |
| US-314 | **Recette E2E** — checklist 9 points de `synthese/10` §4.6, rapport daté | process | 311,312 | 3 | Must | esp32+android |
| US-315 | **Rapport écrit** finalisé | docs | 223,314 | 5 | Must | — |
| US-316 | **Slides + script de démo** + répétition **3× sans intervention** | docs | 314,315 | 5 | Must | — |
| US-317 | Consolidation `docs/suivi/` + support oral | docs | — | 2 | Should | — |

---

## 9. Analyse des dépendances

### 9.1 Indice de parallélisme

| Sprint | US | Chaîne interne la plus longue | **Parallélisme** | Besoin (3 pers.) |
| --- | :---: | :---: | :---: | :---: |
| S1 | 14 | 1 | **14** | 3 ✅ largement |
| S2 | 24 | 1 | **24** | 3 ✅ largement |
| S3 | 17 | 5 | **3,4** | 3 ⚠️ juste |

**Lecture** : en S1 et S2, personne ne peut être bloqué — il y a toujours
4 à 8 fois plus d'US démarrables que de personnes. En S3, on tombe à la limite :
c'est le sprint à surveiller.

### 9.2 Chemin critique

```
US-108 (types protocole)  ──▶ US-209 (routing) ──▶ US-301 (façade api)
   ──▶ US-302 (ffi réel)  ──▶ US-306 (app branchée)
   ──▶ US-312 (E2E matériel) ──▶ US-314 (recette) ──▶ US-316 (démo répétée)
```

**8 maillons sur 14 jours ouvrés** ≈ 1,75 jour par maillon. C'est tenable, mais
**sans marge**. Conséquences opérationnelles :

- **US-108 est l'US la plus importante du projet.** Elle doit être finie
  **cette semaine**, avant tout le reste. Tant qu'elle n'est pas gelée, la
  moitié du backlog S2 est théorique.
- **US-301 et US-302** concentrent le fan-in (12 dépendances). Les placer
  **au tout début de S3** (idéalement dès le 22/09 au matin), pas au milieu.
- Chemin critique secondaire, **firmware** :
  `US-101 (Spike A) ──▶ US-307 ──▶ US-308 ──▶ US-309 ──▶ US-312`.
  **US-101 gate toute la branche firmware** : à finir dans les 2 premiers jours.

### 9.3 Ce que la règle d'or a réellement coûté

Quatre US ont été **scindées** pour respecter §3.2 — c'est le prix à payer, et
il est faible :

| US d'origine | Scindée en | Gain |
| --- | --- | --- |
| « Contrat protocole » | US-108 (types) puis US-201 (codec) | `sync::*` démarre sans attendre la sérialisation |
| « Dashboard ingestion » | US-110 (permissif) puis US-216 (validation + signature) | le dashboard démarre J1 sans schéma figé |
| « App Android » | US-214/215 (bouchon) puis US-306 (vrai FFI) | l'UI avance 1 sprint plus tôt |
| « Firmware relais » | US-220 (octets opaques) puis US-308 (logique dengon) | le BLE embarqué est dérisqué avant le cœur |

---

## 10. Répartition par collaborateur

### 10.1 La vérité sur l'interchangeabilité

Soyons factuels : **avec Rust + Kotlin + C/ESP-IDF + Python en 3 semaines, une
interchangeabilité totale n'est pas atteignable.** Ce qui l'est :

| Objectif | Atteignable ? |
| --- | --- |
| Personne n'est jamais **bloqué** en attente d'un autre | ✅ oui (§9.1) |
| **Bus factor ≥ 2** sur chaque area | ✅ oui, via revue croisée + rotation |
| **N'importe qui** peut prendre **n'importe quelle** US | ❌ non |
| **~60 %** des US prenables par n'importe qui après onboarding | ✅ oui |

Répartition réelle des 55 US par accessibilité :

| Catégorie | US | Part |
| --- | :---: | :---: |
| **Ouvertes** — prenables par tous (docs, process, web, squelettes, contrats, tests) | 21 | 38 % |
| **Semi-ouvertes** — prenables après lecture d'une note d'onboarding (Python, Kotlin UI, sim, modules Rust simples) | 17 | 31 % |
| **Spécialisées** — demandent une montée en compétence réelle (Noise/crypto Rust, FFI/UniFFI, ESP-IDF/NimBLE, cross-compile xtensa) | 17 | 31 % |

### 10.2 Contraintes dures (elles, ne se négocient pas)

| Contrainte | US concernées | Implication |
| --- | --- | --- |
| `needs:materiel-esp32` | US-114, 220, 308, 309, 312, 314 | Celui qui a les cartes. **Prévoir d'en distribuer une 2ᵉ** pour ne pas créer un goulot. |
| `needs:materiel-android` | US-103, 213, 306, 312, 314 | ≥ 2 téléphones Android. À mutualiser. |
| `needs:vps` | US-224 | Accès SSH au VPS. **Donner l'accès aux 3** dès maintenant. |

> **Action immédiate** : ces 3 contraintes sont les seules vraies barrières.
> Les lever (2ᵉ carte ESP32, téléphones partagés, accès VPS pour tous)
> transforme ~10 US « réservées » en US ouvertes.

### 10.3 Le dispositif qui rend la rotation réelle

1. **DRI + doublure.** Chaque US a un responsable *et* une doublure nommée. La
   doublure relit la PR. Sur 3 semaines, ça suffit à créer le bus factor 2.
2. **Revue croisée obligatoire** via `CODEOWNERS` : le reviewer d'une PR
   **n'est jamais** de la même `area:`. C'est le mécanisme d'apprentissage le
   moins coûteux (30 min de lecture ≫ 0 min).
   ```
   /crates/      @paul @tanguy
   /android/     @tanguy @olivier
   /firmware/    @paul @olivier
   /dashboard/   @olivier @tanguy
   /docs/        @olivier @paul
   ```
3. **Note d'onboarding par area** (½ page, dans `docs/suivi/modules/`) : écrite
   par la 1ʳᵉ personne qui ouvre l'area — comment builder, comment tester, les
   3 pièges. C'est **dans la DoD** de la première US de chaque area.
4. **Règle de rotation** : sur 3 sprints, chacun prend au moins **une US dans
   3 areas différentes**. Vérifié au point du vendredi.
5. **Politique de déblocage** : si quelqu'un finit son lot, il prend une US
   `good-first-issue` d'une **autre** area — jamais une de la sienne.

### 10.4 Répartition indicative S1 (à ajuster en réunion)

Les affinités existantes de `plan-mvp.md` sont conservées **comme point de
départ**, pas comme frontières.

| | Paul | Tanguy | Olivier |
| --- | --- | --- | --- |
| **Priorité 1** | US-101 (Spike A) ⚡ | US-103 (Spike C) ⚡ | US-108 (contrat protocole) ⚡ |
| **Priorité 2** | US-104, US-105 | US-109, US-106 | US-107, US-113 |
| **Priorité 3** | US-114 | US-102 | US-110, US-111, US-112 |

⚡ = gate un chemin critique, à finir en premier.

> **Choix délibéré** : US-108 (le contrat protocole, US la plus critique) est
> confiée à **Olivier**, pas à Paul. Raison : Paul est déjà sur le chemin
> critique firmware (Spike A), et Olivier a écrit `format-trame.md` — il a le
> contexte. Ça **répartit le risque** au lieu de le concentrer.

### 10.5 Capacité vs charge — le point qui fâche

- **Capacité** : 3 personnes × 14 jours ouvrés ≈ 42 jours-personne, moins
  cours/réunions/soutenances ≈ **~32 jours-personne effectifs**.
- **Charge** : 155 points. À ~5 points/jour-personne (optimiste pour une équipe
  qui découvre Rust et ESP-IDF) → **~31 jours-personne**.

**C'est à 100 % de la capacité, sans aucune marge.** Recommandation :
sortir immédiatement les 5 US `Should` du périmètre engagé (US-102, 114, 303,
313, 317 = 13 pts) et les traiter en bonus. On retombe à ~142 pts ≈ 28,5 j·p,
soit **~11 % de marge** — le strict minimum pour absorber un imprévu.

L'**ordre de repli** de `plan-mvp.md` §7 reste la soupape : hors-ligne + relais
d'abord, sécurité ensuite, dashboard en dernier.

---

## 11. Risques introduits par cette organisation

| Risque | Probabilité | Mitigation |
| --- | --- | --- |
| Les contrats S1 sont **bâclés** → il faut les rouvrir en S2, et tout le backlog S2 vacille | moyenne | Label `contract` + gel formel en réunion + revue à 3 obligatoire sur les 4 contrats |
| Les **bouchons deviennent la réalité** : l'app marche sur le bouchon, le vrai FFI arrive trop tard | **haute** | US-302 planifiée **au premier jour de S3**, pas au milieu. Répétition E2E dès qu'elle est verte. |
| **Sur-processus** : 55 issues, DoR à 8 points, revue croisée — pour 3 personnes sur 3 semaines | moyenne | La DoR/DoD s'applique aux US `Must` ; les `chore` et `docs` passent en mode allégé. Le board ne doit pas coûter plus de 10 min/jour à tenir. |
| S3 sous-dimensionné (63 pts sur ~10 j·p) | **haute** | C'est le vrai point de rupture. Avancer US-301/302 en fin de S2 si le cœur est prêt. |
| La rotation ralentit l'équipe à court terme | certaine | Assumé : le coût est en S1-S2, le bénéfice en S3 (quand il faut que tout le monde puisse débugger). |

---

## 12. Prochaines actions

**Cette semaine (avant le 14/09) :**

1. **Réunion de ratification** — valider : monorepo (§4.1), les 3 sprints
   (§5.2), la DoR/DoD (§6-7), la répartition S1 (§10.4), et surtout **sortir
   les 5 US `Should` du périmètre** (§10.5).
2. **Lever les 3 contraintes dures** (§10.2) : 2ᵉ carte ESP32, téléphones
   mutualisés, accès VPS pour les 3. C'est ce qui a le meilleur rapport
   effort/effet sur l'interchangeabilité.
3. **Finir US-101 et US-108 en priorité absolue** — ce sont les deux gates.
4. Créer le projet GitHub, les 6 milestones, les labels, les 55 issues.

**Je peux créer tout ça avec `gh`** (le token a bien le scope `project`) :
milestones, labels, template d'issues, les 55 issues avec leurs dépendances en
`Blocked by`, et le board Projects v2 avec ses champs personnalisés. Dis-moi si
je lance — et si tu veux que je crée les issues en français avec le corps
pré-rempli (contexte + critères d'acceptation + lien `docs/synthese/`).
