# Répartition Sprint 3 (22 → 28/09)

**Décidé le :** 2026-09-28
**Décidé par :** Olivier Falahi (G1TS23), avec Claude (Sonnet 5) pour l'analyse
des dépendances et l'équilibrage des points — **à valider par Paul et Oswin**.
**Contrainte de départ, donnée par Olivier** : c'est **Paul** qui a la garde
des deux téléphones Android et de la carte ESP32 de test pour ce sprint —
toute US dont l'AC exige du matériel physique lui revient donc par
construction, indépendamment de l'`area:` GitHub de l'issue.

## 1. Constat de départ

Sprint 3 = 17 issues, **72 points**. US-305 (#43, `dengon-verify`) est déjà
**fait** — Oswin l'a livré et mergé pendant le sprint 2 (PR #92, approuvée),
il reste donc **69 points sur 16 issues** à répartir.

Répartition par area avant attribution :

| Area | Issues | Points |
|---|---|---|
| `core-rust` | 4 (39, 40, 41, 42) | 18 |
| `firmware` | 3 (45, 46, 47) | 15 |
| `android` | 3 (44, 50, 51) | 13 |
| `dashboard-api` | 1 (48) | 3 |
| `dashboard-web` | 1 (49) | 5 |
| `process` | 1 (52) | 3 |
| `docs` | 3 (53, 54, 55) | 12 |

6 issues portent le marqueur **⚡ chemin critique** (#39, #40, #44, #50, #52,
#54) — leur retard décale directement une échéance de jalon. Une septième
(#45, US-307) est la tête du **chemin critique secondaire** (firmware :
`US-101 → US-307 → US-308 → US-309 → US-312`), sans porter le marqueur ⚡
elle-même.

## 2. Bloqueur réel au moment de cette répartition (à lire avant de commencer)

**#39 (US-301, façade `api`) dépend formellement des 12 US de `core-rust` du
sprint 2 — trois sont encore ouvertes, pas mergées :**

| US | Issue | PR | État |
|---|---|---|---|
| US-208 (`observability`) | #22 | #89 | ouverte, CI verte, en attente de revue |
| US-210 (`sync::inventory`) | #24 | #96 | ouverte, en attente de revue |
| US-212 (`sync::courier`) | #26 | #90 | ouverte, **revue faite (Claude), 5 findings mineurs, pas de bloquant** — approbation à donner |

**#47 (US-309, firmware HTTPS) dépend de US-216 (#30, `dashboard-api`
ingestion validée) — PR #91, ouverte, en attente d'approbation.**

Tant que ces 4 PR ne sont pas mergées, #39 et #47 ne peuvent pas être
**terminées** (leurs critères d'acceptation vérifient un comportement qui
n'existe pas encore sur `main`), même si le travail de conception peut
démarrer. **Action recommandée avant d'attaquer le sprint : review + merge
de #89, #90, #91, #96** — aucune n'a de blocage de fond identifié à ce jour.

## 3. Attribution retenue

Légende : ⚡ = chemin critique principal ; 🔧 = tête du chemin critique
secondaire (firmware) ; 🔒 = contrainte matérielle dure (Android et/ou ESP32).

### Paul (POWLAIR) — matériel + intégration — 23 pts

| # | US | Titre | Pts | Dépend de | Pourquoi Paul |
|---|---|---|---|---|---|
| #44 | US-306 | App branchée sur le vrai FFI ⚡🔒 | 5 | US-213/214/215 (fait), US-302 (#40, Oswin) | AC = démo sur **2 vrais téléphones** |
| #46 | US-308 | Firmware FreeRTOS route/inventory/courier/ledger 🔒 | 5 | US-220 (fait), US-307 (#45, Oswin) | AC = testé sur **2 cartes réelles**, survit à `esp_restart()` |
| #47 | US-309 | Firmware HTTPS, buffer ring, JWT, signature 🔒 | 5 | US-216 (#30, PR #91 — voir §2), US-308 (#46) | AC = bout en bout contre le dashboard déployé, sur carte |
| #50 | US-312 | Intégration E2E app↔relais ⚡🔒 | 5 | US-306 (#44), US-309 (#47) | AC = scénarios 2 et 3 **sur vrai matériel** |
| #52 | US-314 | Recette E2E — checklist 9 points ⚡🔒 | 3 | US-311 (#49, Oswin), US-312 (#50) | AC = recette exécutée **sur vrai matériel** |

**Ordre conseillé** : #46 avant #47 (dépendance directe) ; #44 dès que
Oswin livre #40 ; #50 seulement une fois #44 **et** #47 prêts (point de
convergence du sprint — prévoir du temps de débogage, le texte de l'issue
le dit explicitement) ; #52 en tout dernier.

**Point d'attention** : Paul démarre #46 bloqué sur #45 (Oswin) et #44
bloqué sur #40 (Oswin) — sans travail d'amont disponible immédiatement,
les tout premiers jours sont à consacrer à la mise en route matérielle
(flash d'un firmware de test, vérification ESP-IDF/toolchain, charge des
deux téléphones, test physique du matériel lui-même) plutôt qu'à rester
bloqué.

### Oswin (OswinFreyr) — Rust avancé + Android/Web — 21 pts

| # | US | Titre | Pts | Dépend de | Pourquoi Oswin |
|---|---|---|---|---|---|
| #45 | US-307 | `libdengon_core.a` cross-compile 🔧 | 5 | US-101 (fait), US-209/210/211/212 (voir §2) | `skill:rust-avancé`, **aucun matériel requis** (tests hôte) — à faire **en premier**, Paul est bloqué dessus |
| #40 | US-302 | `dengon-ffi` réel (UniFFI) ⚡ | 5 | US-106 (fait), US-301 (#39, Olivier) | `skill:rust-avancé` ; Oswin a écrit le bouchon FFI (US-106) et l'UI Compose qui l'appelle |
| #41 | US-303 | `dengon-node` CLI `btleplug` | 3 | US-102 (fait), US-301 (#39) | `Should`, `skill:rust-débutant` — bonus si capacité restante |
| #51 | US-313 | App — écran réseau, mode éco, Renvoyer | 3 | US-306 (#44, Paul) | `Should`, continuité UI Compose (US-214/215) |
| #49 | US-311 | Web — carte réseau, flotte, alerting | 5 | US-217 (#31, Olivier — PR #93), US-219 (#33) | continuité directe de `dashboard/web` (US-111, déjà livré par Oswin) |

**Ordre conseillé** : #45 **immédiatement** (débloque Paul) ; #40 dès que
#39 (Olivier) est mergée ; #49/#41/#51 remplissent le reste du sprint,
#41 et #51 étant `Should` (à sacrifier en premier si le temps manque).

### Olivier (G1TS23) — intégration core + dashboard + docs — 25 pts

| # | US | Titre | Pts | Dépend de | Pourquoi Olivier |
|---|---|---|---|---|---|
| #39 | US-301 | Façade `api` `dengon-core` ⚡ | 5 | US-201…212 (voir §2, 3 ouvertes) | **Le plus gros fan-in du sprint** (12 dépendances) — à démarrer en premier, débloque Oswin (#40) |
| #42 | US-304 | `dengon-sim` — 5 scénarios du DoD | 5 | US-221 (fait), US-301 (#39) | `skill:rust-débutant`, continuité de US-221/US-217 (déjà écrits par Olivier/Oswin) |
| #48 | US-310 | Dashboard `GET /api/integrity` | 3 | US-216 (#30, PR #91), US-305 (#43, fait) | continuité directe de `dashboard-api` (US-216/217/218) |
| #53 | US-315 | Rapport écrit finalisé | 5 | US-223 (#37, Olivier), US-314 (#52, Paul) | continuité de US-223, déjà en cours |
| #54 | US-316 | Slides + script de démo ⚡ | 5 | US-314 (#52), US-315 (#53) | dernier maillon du chemin critique — nécessite une vue d'ensemble transverse |
| #55 | US-317 | Consolidation `docs/suivi` + support oral | 2 | aucune — démarrable jour 1 | Olivier tient déjà `docs/suivi/` à jour depuis le sprint 2 |

**Ordre conseillé** : #39 **immédiatement** (débloque Oswin) ; #55 peut se
faire en tâche de fond n'importe quand (aucune dépendance) ; #42 dès que
#39 est posée ; #48 dès que #91 (US-216) merge ; #53/#54 en fin de sprint,
une fois #52 (Paul) livré.

## 4. Équilibrage et risques

- **Points :** Paul 23, Oswin 21, Olivier 25 — écart de 4 points, jugé
  acceptable : le travail de Paul est presque entièrement gated par du
  matériel physique (flash, débogage sur cible, pas de retour CI rapide),
  donc plus coûteux en temps réel que son total de points ne le suggère.
- **Chemin critique réparti sur les trois** (#39 Olivier, #40 Oswin, #44/#50/#52
  Paul, #54 Olivier) plutôt que concentré sur une personne — cohérent avec
  la règle de rotation de l'US-317 (« chacun peut présenter au moins deux
  areas »).
- **Risque identifié dans le texte même des issues** : #50 (US-312) est le
  point de convergence de TOUT le sprint (app + firmware + dashboard) —
  seule US du sprint à dépendre de deux chaînes de travail par deux
  personnes différentes (Paul lui-même pour #44/#47). Prévoir une marge
  avant le jalon J4 plutôt que d'enchaîner #50 immédiatement après #47.
- **Sprint 2 pas totalement clos** (§2) : 4 PR ouvertes (#89, #90, #91, #96)
  conditionnent le démarrage réel de #39 et #47. Aucune ne semble bloquée
  sur le fond — un passage de revue rapide en tout début de sprint 3
  débloque tout le monde.

## 5. Assignation GitHub

Assignations posées le 2026-09-28 via `gh issue edit --add-assignee` sur les
16 issues listées ci-dessus (#39-42, #44-55, hors #43 déjà fait). À ajuster
librement si Paul ou Oswin voient un déséquilibre une fois le travail
commencé — ce document explique le raisonnement de départ, il ne fige rien.
