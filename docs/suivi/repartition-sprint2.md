# Répartition Sprint 2 (15 → 21/09, en pratique démarré le 25/09)

**Décidé le :** 2026-09-25
**Décidé par :** Olivier Falahi (G1TS23), avec Claude (Sonnet 5) pour l'analyse
des dépendances et l'équilibrage des points — **à valider par Paul et Oswin**,
notamment le point de friction §3.
**Convention de noms :** « Paul » = POWLAIR, « Oswin » = OswinFreyr = Tanguy
(voir `.github/CODEOWNERS`) — pseudo GitHub retenu de façon cohérente dans
ce document (retour de revue d'OswinFreyr : `CODEOWNERS` et
`proposition-organisation-github.md` §13 utilisent « Tanguy », choisir une
convention et s'y tenir pour que la présentation orale reste lisible).

## 1. Pourquoi ce document

Le board GitHub (issues + labels `sprint:s2`) fait foi pour le statut au jour
le jour. Ce document explique le **raisonnement** derrière l'attribution —
pourquoi telle personne plutôt qu'une autre, quelles dépendances ont compté,
et où ça peut frotter — pour que la présentation orale de fin de projet
puisse s'appuyer dessus, et pour qu'on n'ait pas à se remémorer le pourquoi
en cours de sprint.

## 2. Constat de départ : le déséquilibre `core-rust`

Sprint 2 = 24 issues, **80 points** au total. Répartition par area avant
attribution :

| Area | Issues | Points |
|---|---|---|
| `core-rust` | 13 | **42** |
| `dashboard-api` | 4 | 13 |
| `android` | 3 | 11 |
| `firmware` | 1 | 5 |
| `dashboard-web` | 1 | 3 |
| `process` | 1 | 3 |
| `docs` | 1 | 3 |

`core-rust` à lui seul = 42 points sur 13 issues, **toutes en `Must`, toutes
jalon J1** (échéance 18/09 — déjà 7 jours de retard au moment de cette
répartition). C'est injouable pour une seule personne sur ce qui reste du
sprint, même si CODEOWNERS ne désigne que Paul comme propriétaire principal
de `/crates/`. **Décision : `core-rust` est coupé en 3 tracks et réparti
entre les 3 personnes**, pas laissé à une seule — voir §4.

**Le retard ne concerne pas que J1 (retour de revue d'OswinFreyr).** Au
25/09, date de cette répartition : J2 et J3 (échéance 21/09) sont aussi
dépassés, J4 (25/09) tombe le jour même, et **J5 (28/09, recette + rapport
+ slides) tombe trois jours après la rédaction de ce document**. « À faire
en fin de sprint, pas maintenant » (§4, US-223) est donc risqué pour un
jalon de soutenance — soit les jalons sont explicitement replanifiés en
réunion d'équipe, soit US-223 doit être priorisé plus tôt que prévu ici.

## 3. Point de friction à trancher en équipe : `sync::` coupé en deux

Les 4 modules `sync::routing` (US-209), `sync::inventory` (US-210),
`sync::status` (US-211) et `sync::courier` (US-212) forment un ensemble
étroitement lié :

- `routing` décide **si** un paquet est relayé (TTL, dedup, anti-inondation).
- `inventory` décide **quoi** échanger entre deux pairs qui se rencontrent.
- `status` suit le cycle de vie d'un message (queued → in_flight → delivered).
- `courier` gère le dépôt/collecte des enveloppes scellées pour les pairs
  hors-ligne.

Cette répartition les coupe en deux : **Paul** prend `routing` + `inventory`,
**Oswin** prend `status` + `courier`. Chacun peut avancer sur des fichiers
distincts (`crates/dengon-core/src/sync/{routing,inventory}.rs` vs
`{status,courier}.rs`), donc pas de conflit Git direct — mais les 4 modules
s'appellent probablement entre eux (ex. `routing` doit connaître le `status`
d'un message pour décider s'il vaut encore la peine d'être relayé).

**Recommandation, à discuter avant de partir chacun de son côté** : un point
rapide (15-20 min) entre Paul et Oswin sur les signatures publiques des 4
modules — quelles fonctions/types chacun expose à l'autre — avant d'écrire
l'implémentation. Sinon, risque réel d'intégration cassée en fin de sprint,
symétrique au risque déjà identifié dans
`docs/olivier/proposition-organisation-github.md` §11 (« bouchons qui
deviennent la réalité »).

## 4. Attribution retenue

Légende dépendances : ✅ = déjà mergée sur `main` ou approuvée/prête ; 🟡 =
PR ouverte, pas encore mergée ; 🔴 = bloquante, pas encore résolue.

### Paul (POWLAIR) — protocole/crypto + firmware — 26 pts

| # | US | Titre | Pts | Dépend de | État dépendance |
|---|---|---|---|---|---|
| #17 | US-203 | `crypto` — Ed25519 sign/verify | 2 | US-101 | ✅ mergé (#64) |
| #18 | US-204 | `crypto` — Noise XX/X, `recipient_tag`, padding | 5 | US-101 | ✅ mergé (#64) |
| #19 | US-205 | `identity` — keypair, coffre, QR, code vérif | 3 | US-104 | ✅ mergé |
| #23 | US-209 | `sync::routing` — TTL, dedup, anti-inondation | 5 | US-105, US-108 | 🟡 US-108 = PR #63, prête (checks verts, attend approbation) |
| #24 | US-210 | `sync::inventory` — échange, push du manquant | 3 | US-105, US-108 | 🟡 idem #63 |
| #34 | US-220 | Firmware `transport_nimble.c` | 5 | US-105, US-114 | ✅ US-114 mergée (#68, 25/09) |
| #36 | US-222 | CI — jobs `sim`/`audit`/`cross-vectors` | 3 | US-104, US-108, US-113 | 🟡 US-108 idem ; reste dépend du même #63 |

**Ordre conseillé** : crypto (203) avant identity (205) — identity **génère
un keypair** (critère d'acceptation de #19), donc utilise réellement les
primitives Ed25519/X25519 de crypto en interne (retour de revue
d'OswinFreyr : « probablement » était trop prudent, c'est une dépendance
réelle US-203 → US-205, même si ce n'est pas listé comme dépendance
formelle inter-sprint sur l'issue). `routing` avant `inventory` (§3).

### Oswin (OswinFreyr) — Android + protocole L2/L3 — 26 pts

| # | US | Titre | Pts | Dépend de | État dépendance |
|---|---|---|---|---|---|
| #27 | US-213 | `AndroidTransport` — GATT server+advertiser+scanner | 5 | US-103, US-105, US-109 | ✅ US-103 (voir §5, MàJ 28/09) ; 🟡 US-109 (#9), PR #74 en attente |
| #28 | US-214 | UI Compose — conversations, fil, statuts | 3 | US-106, US-109 | 🟡 US-106 = PR #69, prête techniquement, attend approbation Paul (CODEOWNER `/crates/`) ; 🟡 US-109 (#9), PR #74 en attente |
| #29 | US-215 | UI Compose — QR + code vérif 60 chiffres | 3 | US-106, US-109 | 🟡 idem #69 ; 🟡 US-109 (#9), PR #74 en attente |
| #15 | US-201 | `protocol::codec` — encode/decode | 3 | US-104, US-108 | 🟡 US-108 = PR #63 |
| #16 | US-202 | `protocol` — fragmentation L2, MTU aléatoire | 3 | US-104, US-108 | 🟡 idem |
| #25 | US-211 | `sync::status` — machine à états MVP | 3 | US-108 | 🟡 idem |
| #26 | US-212 | `sync::courier` — dépôt/collecte enveloppes | 3 | US-108 | 🟡 idem |
| #35 | US-221 | `dengon-sim` — harness N nœuds | 3 | US-104, US-105 | ✅ mergées |

**Ordre conseillé** : US-201 avant US-202 (fragmentation dépend de la forme
encodée). US-213 était bloquée par US-103, désormais close (§5, MàJ
28/09) ; reste bloquée par US-109 (#9, PR #74) — commencer par les autres
tant que ça se débloque.

### Olivier (G1TS23) — dashboard + stockage/observabilité + docs — 28 pts

| # | US | Titre | Pts | Dépend de | État dépendance |
|---|---|---|---|---|---|
| #30 | US-216 | Dashboard — validation schéma + sig Ed25519 | 3 | US-107, US-110 | 🟡 US-107/#60 et US-110/#59, prêtes, attendent approbation |
| #31 | US-217 | Dashboard — projections, reconstruction statut | 5 | US-107, US-110 | idem |
| #32 | US-218 | Dashboard — `GET /api/stream` SSE | 2 | US-110 | idem |
| #38 | US-224 | Déploiement VPS | 3 | US-110 | idem |
| #33 | US-219 | Web — écran « parcours d'un message » | 3 | US-110, US-111 | US-110 idem ; US-111 ✅ mergée (#70, #71) |
| #20 | US-206 | `ledger` — append/verify_chain/export | 3 | US-104 | ✅ mergée |
| #21 | US-207 | `store` SQLite — schéma, chiffrement champ/champ | 3 | US-104 | ✅ mergée |
| #22 | US-208 | `observability` — catalogue, JSON canonique, redaction | 3 | US-104, US-107 | 🟡 US-107/#60 |
| #37 | US-223 | Rapport écrit | 3 | US-112 | 🟡 US-112/#66, prête, attend approbation — **jalon J5 (28/09), à faire en fin de sprint, pas maintenant** |

**Pourquoi le stockage/observabilité (US-206/207/208) plutôt qu'à Paul** :
ce sont des modules `dengon-core` mais orientés persistance/données, pas
protocole/crypto — proche de ce que je fais déjà côté `dashboard-api`
(schéma SQLite, catalogue d'événements en #60). Ça équilibre aussi les
points sans surcharger Paul.

## 5. Le vrai bloqueur transverse : US-103 non close

`US-213` (Oswin, Android) **dépend formellement** de US-103 (#3, Spike C).
Au 25/09, le Spike C n'est exécuté que **partiellement** (Android +
iPhone/nRF Connect en central de repli — voir `docs/suivi/00-journal.md`,
entrée du 25/09) : l'échange BLE bout-en-bout est démontré, mais le MTU réel
n'a pas pu être mesuré (limitation iOS/CoreBluetooth), ni le timing, ni la
matrice d'appareils. **US-103 reste ouverte tant qu'un second Android n'a
pas fait le test complet.**

Tant que ce n'est pas fait, Oswin ne peut pas démarrer US-213 dans les
règles. **Correction (retour de revue d'OswinFreyr, 2026-09-28) : ce n'est
pas la règle « une US ne dépend jamais d'une US du même sprint » qui
s'applique ici** — US-103 est en **S1**, pas S2, donc ça n'a jamais été un
cas de dépendance intra-sprint. Le vrai blocage est la **DoR** (Definition
of Ready) : une dépendance déclarée doit être **close** avant de démarrer,
et US-103 (#3) ne l'est pas encore.

**US-213 dépend aussi de US-109 (#9, squelette Android), toujours
ouverte** (🟡, bloquée sur l'approbation de la PR #74) — dépendance déjà
listée au §4 mais dont le statut manquait dans la colonne « État
dépendance », ajouté ci-dessous. Même chose pour US-214 (#28) et US-215
(#29).

En attendant que US-103 et US-109 se débloquent, Oswin peut avancer sur ses
**7 autres issues** (US-214, US-215, US-201, US-202, US-211, US-212,
US-221 — **21 pts**, pas 15 : US-211 et US-212 avaient été omises du
compte initial, retour de revue d'OswinFreyr). Ces 7 dépendent elles-mêmes
de US-108 (#63) ou US-106 (#69), comme US-201/US-202 (voir §4).

**Mise à jour 2026-09-28 :** le Spike C a depuis été **exécuté
intégralement** sur 2 vrais Android (Samsung Galaxy A16 + Pixel 8 Pro),
les 4 critères d'acceptation de US-103 sont démontrés (MTU 517,
scan→connexion 354 ms, connexion→échange 1308 ms) — voir
`docs/suivi/modules/android-app.md` §« Spike C » et
`docs/suivi/00-journal.md`, entrée du 2026-09-28. US-213 n'est donc plus
bloquée par US-103 ; il reste US-109 (#9, PR #74 en attente d'approbation).

## 6. Toutes les PR S1 encore ouvertes qui bloquent indirectement

Beaucoup de dépendances ci-dessus pointent vers des PR déjà prêtes
(checks verts, resynchronisées) mais qui attendent une vraie approbation
GitHub — pas un problème technique, un problème d'agenda.

**Correction (retour de revue d'OswinFreyr, 2026-09-28) : les approbateurs
requis n'étaient pas « Paul ou Oswin » pour 4 des 5 PR ci-dessous.** Avec le
`CODEOWNERS` actuel (`require_code_owner_reviews` actif), chaque chemin
touché exige l'approbation d'un de ses owners **autre que l'auteur** — et
plusieurs de ces PR touchent des chemins dont les listes d'owners, une fois
l'auteur (Olivier) retiré, ne se recoupent **pas** :

| PR | US | Chemins touchés | Owners requis (hors auteur) |
|---|---|---|---|
| #59 | US-110 (dashboard-api) | `.github/` + `docs/` (Paul) **et** `dashboard/` (Oswin) | **Paul et Oswin**, pas l'un ou l'autre |
| #60 | US-107 (contracts) | `.github/` + `docs/` | **Paul** (seul owner de ces chemins hors auteur) |
| #63 | US-108 (protocol) | `docs/suivi/` (`crates/` accepte Paul ou Oswin, mais `docs/` exige Paul) | **Paul** |
| #66 | US-112 (doc sécurité) | `docs/` uniquement | **Paul** |
| #69 | US-106 (ffi) | `crates/` | **Paul** spécifiquement (CODEOWNER `/crates/`, Oswin auteur) |

**Conséquence pour ce plan : Paul est sur le chemin critique de 5 PR sur
5.** C'est exactement ce que ce §6 cherche à débloquer — faire approuver ces
5 PR (en particulier obtenir un créneau de Paul) est la façon la plus
rapide de dégager une bonne partie du tableau du §4, plus rapide que
d'attendre que chacun les redécouvre en butant dessus pendant le sprint.

## 7. Suivi

Cette répartition a été posée comme assignation GitHub sur chaque issue
(champ *Assignees*) le 2026-09-25, avec un commentaire pointant vers ce
document. Si la charge réelle diverge en cours de route, mettre à jour ce
fichier avec une note datée plutôt que de le réécrire silencieusement.
