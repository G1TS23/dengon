# Module : `dashboard-web` (`dashboard/web/`)

**Rôle en une phrase :** la page web du dashboard d'observabilité — liste des
messages suivis + écran de détail (parcours), branchée sur l'API réelle.
**Correspond à la conception :** [`docs/olivier/dashboard.md`](../../olivier/dashboard.md)
§3, §4, §8, §9 ; [`docs/synthese/09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md)
§5 (« Parcours d'un message »), §11.2 (schéma SQLite `messages`/`message_hops`).
**Dernière mise à jour :** 2026-09-29 (US-310 — écran « Intégrité », rebasé sur US-219)
**État :** fait (US-219 + US-310) — les deux écrans de l'US-111 consomment
`GET /api/messages`/`GET /api/messages/{id}`, l'écran `#/integrite` consomme
`GET /api/integrity` ; tous se rafraîchissent seuls via `GET /api/stream`
(SSE, US-218). Plus de données bidon.

## Vérification visuelle à 360 px — faite le 2026-09-25 (US-111)

Le critère d'acceptation de l'US-111 demande un « rendu correct sur mobile
(largeur 360 px) », vérifié manuellement avec capture d'écran (DoR n°7). À
l'écriture du code (2026-09-20), aucun navigateur n'était utilisable :
`app.js`/`data.js` avaient seulement été exécutés sous Node.js contre un DOM
reconstitué à la main (0 exception sur 5 écrans, aucune classe CSS orpheline).

**Le 2026-09-25**, le rendu réel a été vérifié avec le Chromium
`chrome-headless-shell` (build 1217) déjà installé par Playwright dans
`~/AppData/Local/ms-playwright/`, en ouvrant `index.html` via `file://` dans
une fenêtre de 360 px de large. Résultat conforme sur les 4 écrans : aucun
débordement horizontal, badges de statut visibles et colorés, `—` sur les
champs absents, message `unknown` sans saut et id inconnu affichés proprement.
Captures : [`../assets/us-111/`](../assets/us-111/).

Limite : le mode sombre n'a été vu qu'avec le Chromium headless complet, qui
impose une largeur de fenêtre minimale d'environ 500 px (captures rognées, non
conservées) ; les couleurs sombres s'appliquent bien, mais pas à 360 px.

**US-219 (2026-09-28) n'a PAS pu refaire cette vérification visuelle dans un
vrai navigateur** — aucun outil de navigation disponible dans cet
environnement pour cette session (écart consigné dans
`03-ecarts-conception.md`). Le rendu HTML/CSS n'a pas changé (mêmes
gabarits `carteMessage`/`ligneHop`/classes CSS qu'à l'US-111, seule la
source des données change) ; ce qui a été vérifié à la place :
`node --check` sur `app.js`/`api.js` (syntaxe), et bout en bout via `curl`
contre une vraie instance de l'API (les 20 fixtures golden ingérées,
`GET /api/messages`/`GET /api/messages/{id}` renvoient la forme attendue,
CORS vérifié avec un serveur web sur un port différent de l'API). À
revérifier visuellement dès qu'un navigateur est disponible.

## À quoi ça sert

Le dashboard observe le réseau sans jamais transporter de messages ni voir de
contenu (`docs/olivier/dashboard.md` §2). L'US-111 avait livré les deux
écrans **débranchés de toute API** (S1), pour ne pas attendre
`dashboard/api`. L'US-219 les branche sur l'API réelle : une liste des
messages suivis et le détail du parcours d'un message, tous deux à jour en
direct via SSE.

## Structure

```
dashboard/web/
  index.html   — squelette de page : bandeau de portée + <main id="app">
  style.css    — mobile-first (360 px), variables CSS clair/sombre
  api.js       — accès réseau : fetchMessages()/fetchMessage()/abonnerFlux()
  app.js       — routage par hash (#/message/<id>), rendu liste + détail
```

`data.js` (US-111, données bidon) est **retiré** — plus utilisé une fois
branché sur l'API réelle.

Si `dashboard/web` n'est pas servi derrière le même reverse-proxy que
`dashboard/api` (US-224), définir `window.DENGON_API_BASE` (l'URL de l'API,
ex. `https://mon-vps:8443`) dans un `<script>` placé AVANT `api.js`/`app.js`
dans `index.html` — vide par défaut (même origine que la page).

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `DengonApi.fetchMessages()` | `api.js` | `GET /api/messages` — liste des messages, déjà triée par l'API (`ORDER BY last_event_ms DESC`). |
| `DengonApi.fetchMessage(id)` | `api.js` | `GET /api/messages/{id}` — détail + `hops`. Renvoie `null` sur 404 (distingué d'une erreur réseau, qui lève) pour que l'écran affiche « introuvable » plutôt que « API injoignable ». |
| `DengonApi.abonnerFlux(cb)` | `api.js` | Ouvre `new EventSource("/api/stream")` (US-218), rappelle `cb` pour chaque événement reçu. Reconnexion gérée nativement par `EventSource` — rien à coder côté client pour ce critère de l'US-218. |
| `route()` | `app.js` | Lit `window.location.hash` : `#/message/<id>` → écran détail, sinon → liste. **Async** (attend `fetch`) ; un jeton de génération (`generationCourante`) empêche une réponse `fetch` périmée d'écraser un écran plus récent (navigation rapide ou rafraîchissement SSE pendant un chargement en cours). |
| `renderListe()`/`renderDetail(id)` | `app.js` | Mêmes gabarits que l'US-111 (`carteMessage`/`ligneHop`), mais chargent leurs données via `DengonApi` au lieu de `window.DENGON_DASHBOARD_DATA`. |
| `planifierRafraichissement()` | `app.js` | Rappelée à chaque événement SSE ; redemande l'écran courant à l'API, débit borné à 1 rafraîchissement / 500 ms (`setTimeout`) pour qu'un batch de plusieurs événements ne déclenche pas autant de `fetch`. |
| `texteOuTiret(valeur)` | `app.js` | `null`/`undefined`/`""` → `"—"` ; **`0` est préservé** (ex. `fanout: 0`) — piège classique du JS (`valeur \|\| "—"` aurait aussi effacé les zéros légitimes), évité par une comparaison stricte à `null`/`undefined`. |

## Flux principal (exemple)

1. Ouverture de `index.html` (servie par un vrai serveur — voir Décisions) →
   `app.js` s'exécute, hash vide → `route()` affiche « Chargement… » puis
   `renderListe()` une fois `GET /api/messages` répondu.
2. Clic sur une carte → navigue vers `#/message/<msg_log_id>` →
   `hashchange` → `renderDetail(id)` affiche l'en-tête et la timeline des
   sauts connus (`GET /api/messages/{id}`, champ `hops`) — ou l'état « aucun
   saut remonté » si `hops` est vide, ou « introuvable » si 404.
3. Lien « Retour à la liste » → `#/` → `renderListe()`.
4. En parallèle, `DengonApi.abonnerFlux()` reçoit chaque événement ingéré
   côté serveur et redemande l'écran courant — la page se met à jour sans
   rechargement quand un nouveau batch arrive.

## Dépendances

- **Internes :** `dashboard/api` — `GET /api/messages`, `GET /api/messages/{id}`
  (`app/messages_api.py`), `GET /api/stream` (US-218). CORS `GET` ouvert à
  toute origine côté API (voir `docs/suivi/modules/dashboard-api.md`), pour
  que cette page fonctionne même servie depuis un port/domaine différent.
- **Externes :** aucune. Pas de framework, pas de CDN, pas de build.

## Décisions d'implémentation

- **`file://` abandonné (US-219)** : l'US-111 choisissait `data.js` en
  `<script src>` précisément pour rester utilisable en `file://`. Une fois
  branché sur une vraie API HTTP, `fetch`/`EventSource` depuis `file://`
  sont bloqués par le navigateur (origine `null`) — la page suppose
  désormais un vrai serveur HTTP (même un simple `python3 -m http.server`
  suffit). Régression assumée, anticipée par le texte même de l'US-111
  (« US-219 branchera ces écrans sur l'API réelle »).
- **`message_hops` (§11.2) dérivée à la lecture, pas une table à part** :
  voir `docs/suivi/modules/dashboard-api.md` (`app/messages_api.py`) — la
  timeline vient de `events`, reconstruite à chaque `GET`, pas d'une table
  alimentée à l'écriture.
- **Un événement SSE quelconque redemande l'écran entier**, pas de fusion
  incrémentale côté client : le serveur (`app/projections.py`) reste la
  seule source de vérité pour `status`/`hop_count` — maintenir une seconde
  copie de cette logique en JavaScript aurait un coût de synchronisation
  pour un gain minime au volume visé (démo 5-8 appareils, B-4).
- **CORS `GET` ouvert à `*` côté API**, pas restreint à l'origine de
  `dashboard/web` : ces routes ne renvoient que des données déjà redigées,
  sans cookie ni session — voir `dashboard-api.md`.
- **Routage par hash (`#/message/<id>`)**, pas un routeur/framework : un lien
  `#...` fonctionne aussi bien en `file://` que servi par un vrai serveur —
  contrairement à l'API History (`pushState`), qui exige un serveur capable de
  répondre à n'importe quelle URL.
- **Timestamps affichés en UTC** (`getUTCDate()`/`getUTCHours()`/…) alors que
  les données bidon sont construites avec `Date.UTC(...)` : l'affichage ne
  dépend donc pas du fuseau horaire de la machine qui ouvre la page —
  important pour que la capture d'écran demandée par le DoR soit reproductible
  d'un poste à l'autre.
- **Statuts alignés sur `docs/synthese/07-cycle-de-vie-et-statuts.md`**
  (`queued`/`in_flight`/`delivered`/`read`/`expired`) plutôt que les quatre
  catégories simplifiées de la première esquisse
  (`docs/olivier/dashboard.md` §9, « En circulation/Distribué/Expiré ») : la
  conception retenue dans `docs/synthese/` réutilise le vocabulaire du cœur
  pour la projection `messages` (§11.2), donc pour rester cohérent avec la
  seule doc qui fait foi (`00-contexte-global.md`), plus `unknown`, propre au
  dashboard (aucun événement reçu pour ce message — vue partielle).
- **Pas de compteur « nœuds actifs »** : `docs/olivier/dashboard.md` §3 le
  liste comme souhaité mais **explicitement pas tranché** (deux options
  ouvertes : battement anonyme séparé, ou retrait du compteur en v1). Afficher
  un chiffre bidon aurait fait croire qu'une question de conception encore
  ouverte était réglée.

## Tests

- **US-111 (2026-09-25)** : vérification manuelle Node.js + Chromium
  headless à 360 px — voir la section « Vérification visuelle » en tête de
  fiche.
- **US-219 (2026-09-28)** : `node --check app.js api.js` (syntaxe) ; bout en
  bout via `curl` contre une vraie instance de `dashboard-api` — les 20
  fixtures golden de US-107 ingérées, `GET /api/messages` renvoie les 20
  `msg_log_id` attendus triés par activité, `GET /api/messages/{id}` renvoie
  le détail + les `hops` dans l'ordre chronologique (mêmes scénarios de
  statut que `test_messages_api.py` côté API : `1122…` → `delivered`,
  latence 880 ms ; `aabbccdd…` → `expired`) ; CORS vérifié avec l'API et un
  serveur web sur deux ports différents (`access-control-allow-origin: *`
  présent sur `GET` et sur le préflight `OPTIONS` de `/api/stream`).
  **Pas de vérification dans un vrai navigateur** cette fois (aucun outil de
  navigation disponible dans cette session) — écart consigné, à refaire dès
  que possible.

## US-310 — écran « Intégrité »

Ajoute un troisième écran, routé sur `#/integrite`, qui liste — un par
nœud connu — le verdict de `GET /api/integrity` (`dengon-verify` côté
`dashboard/api`, voir `dashboard-api.md`).

- **Nav** : `index.html` gagne un bandeau `<nav class="nav-app">` avec deux
  liens (`Messages` / `Intégrité`), au-dessus du `<h1>` existant.
- **`api.js`** : `fetchIntegrity()` (même `DENGON_API_BASE` que les autres
  appels US-219). Rebasée sur l'US-219 le 2026-09-29 : la version d'origine
  faisait son propre `fetch` avec un `API_BASE` local et une garde
  `section.isConnected` ; elle passe désormais par `api.js` et le `route()`
  async de l'US-219, dont le jeton de génération ignore déjà une réponse
  arrivée après une navigation plus récente.
- **`renderIntegrite()`** (`app.js`) : async, comme `renderListe`/
  `renderDetail` — chargement et erreur réseau gérés par `route()`
  (`ecranChargement`/`ecranErreur` communs) ; succès = une carte par nœud
  (verdict + plage + signatures), ou « aucun nœud » si la liste est vide.
- **Libellés** : `ok` → « Intègre », `broken` → « Altéré », `fork` →
  « Fourche détectée », `gap` → « Trou dans le journal », `unverified` →
  « Non vérifiable » ; réutilise les classes `statut--*`
  existantes (`--delivered`, `--expired`, `--inconnu`) plutôt que d'en créer
  de nouvelles.
- **Vérifié en navigateur (2026-09-28)**, via Chromium/Playwright
  (`/opt/pw-browsers/chromium`), API réelle (FastAPI + `dengon-verify`
  compilé) sur un jeu de nœuds seedés manuellement (sain / altéré /
  non-vérifiable) : les 3 libellés s'affichent correctement, capturé à
  500 px et 360 px, plus l'état d'erreur (API injoignable). Captures non
  commitées (script jetable dans le répertoire de travail temporaire de la
  session). Vérification faite **avant** le rebase sur l'US-219, pas refaite
  depuis.

## Limites connues / TODO

- Mode sombre non vérifié à 360 px depuis l'US-111 (seulement à ~500 px,
  voir en tête de fiche).
- **Rendu US-219/US-310 pas revérifié dans un vrai navigateur** après le
  rebase de l'US-310 sur l'US-219 (voir Tests) — l'écran `#/integrite` passe
  désormais par `api.js` (`fetchIntegrity`) et le `route()` async à jeton de
  génération de l'US-219.
- L'écran `#/integrite` est rafraîchi par le SSE comme les autres (débit
  borné à 500 ms) : chaque rafraîchissement relance `dengon-verify` côté API
  pour chaque nœud — acceptable au volume de démo, à revoir si la flotte
  grossit.
- Pas de carte réseau, flotte de relais ni recherche de logs
  (`docs/synthese/09` §5) : ces écrans sont des USs séparées (US-311…), pas
  couvertes ici.
- Pas de test automatisé du JS (pas de framework de test en place) — même
  discipline que l'US-111, DoR n°7 de l'US-219 demande une vérification
  manuelle, pas une suite automatisée.

## Pour l'oral

C'est la première preuve visuelle du principe « vue partielle, jamais de
contenu » qui définit tout le dashboard : la page ne sait montrer que ce que
des nœuds ayant eu du Wi-Fi ont bien voulu rapporter, jamais qui a écrit quoi
à qui. Le point à montrer : le message « inconnu » (aucun saut reçu) n'est pas
un bug, c'est un état représentable — exactement la garantie de
confidentialité et la contrainte réseau (relais opportunistes) que la
conception assume.
