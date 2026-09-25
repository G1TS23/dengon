# Module : `dashboard-web` (`dashboard/web/`)

**Rôle en une phrase :** la page web du dashboard d'observabilité — liste des
messages suivis + écran de détail (parcours), pour l'instant sur données
bidon.
**Correspond à la conception :** [`docs/olivier/dashboard.md`](../../olivier/dashboard.md)
§3, §4, §8, §9 ; [`docs/synthese/09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md)
§5 (« Parcours d'un message »), §11.2 (schéma SQLite `messages`/`message_hops`).
**Dernière mise à jour :** 2026-09-25 (vérification visuelle)
**État :** fait (US-111) — page statique, données en dur, aucun appel
réseau. Pas encore branchée sur l'API (US-217, US-219).

## Vérification visuelle à 360 px — faite le 2026-09-25

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

## À quoi ça sert

Le dashboard observe le réseau sans jamais transporter de messages ni voir de
contenu (`docs/olivier/dashboard.md` §2). Cette US livre les deux premiers
écrans côté web, **débranchés de toute API** (S1) pour ne pas attendre
`dashboard/api` (US-110/US-216/US-217, sprint 2) : une liste des messages
suivis, et le détail du parcours d'un message (suite de sauts par nœuds
anonymisés). Sert aussi de base visuelle que l'US-219 (« écran parcours d'un
message », S2) viendra brancher sur l'API réelle et le flux SSE.

## Structure

```
dashboard/web/
  index.html   — squelette de page : bandeau de portée + <main id="app">
  style.css    — mobile-first (360 px), variables CSS clair/sombre
  data.js      — données bidon (window.DENGON_DASHBOARD_DATA), forme alignée
                 sur le schéma SQLite (messages / message_hops)
  app.js       — routage par hash (#/message/<id>), rendu liste + détail
```

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `window.DENGON_DASHBOARD_DATA` | `data.js:16` | `{ messages: [...], hops: { [msg_log_id]: [...] } }` — même forme que les tables `messages`/`message_hops` de `docs/synthese/09` §11.2. Inclut volontairement un message sans aucun saut (`unknown`) et un saut aux champs radio incomplets (`rssi`/`fanout` absents). |
| `route()` | `app.js:~180` | Lit `window.location.hash` : `#/message/<id>` → écran détail, sinon → liste. Réécrit `#app` à chaque changement (`hashchange`) et à l'ouverture. |
| `renderListe()` | `app.js:~110` | Construit une carte par message, triée par activité la plus récente. |
| `renderDetail(msgLogId)` | `app.js:~150` | En-tête (id, statut, dates, latence) + timeline verticale des sauts. Si l'id est inconnu de la démo, affiche un état « introuvable » plutôt que de planter. |
| `texteOuTiret(valeur)` | `app.js:~35` | `null`/`undefined`/`""` → `"—"` ; **`0` est préservé** (ex. `fanout: 0`) — piège classique du JS (`valeur \|\| "—"` aurait aussi effacé les zéros légitimes), évité par une comparaison stricte à `null`/`undefined`. |

## Flux principal (exemple)

1. Ouverture de `index.html` (`file://` ou servi statiquement) → `app.js`
   s'exécute, hash vide → `renderListe()` affiche les 5 messages bidon,
   triés par dernière activité.
2. Clic sur une carte → navigue vers `#/message/<msg_log_id>` →
   `hashchange` → `renderDetail(id)` affiche l'en-tête et la timeline des
   sauts connus (ou l'état « aucun saut remonté » si `hops[id]` est absent).
3. Lien « Retour à la liste » → `#/` → `renderListe()`.

Aucune étape ne fait de requête réseau : `data.js` est un `<script>` classique
(pas un `fetch`/`XHR` d'un fichier JSON, qui échouerait en `file://` à cause de
CORS).

## Dépendances

- **Internes :** aucune (page volontairement débranchée de `dashboard/api`,
  qui n'existe pas encore sur `main`).
- **Externes :** aucune. Pas de framework, pas de CDN, pas de build — un
  navigateur suffit à ouvrir `index.html`.

## Décisions d'implémentation

- **`data.js` en `<script src>`, pas un `.json` chargé en `fetch`** : un
  `fetch()`/`XMLHttpRequest` vers un fichier local est bloqué par CORS dans la
  plupart des navigateurs quand la page est ouverte en `file://` (critère
  d'acceptation explicite de l'US-111). Un `<script>` classique n'a pas cette
  restriction.
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

- **Aucun test automatisé commité** — la stratégie de test de l'US-111 est une
  vérification manuelle (DoR n°7). `app.js`/`data.js` exécutés une fois sous
  Node.js contre un DOM reconstitué à la main (script jetable, non commité) :
  0 exception sur 5 écrans (liste, détail à sauts, détail sans saut, id
  inconnu, retour liste), sortie texte conforme à l'attendu.
- **Rendu réel à 360 px (2026-09-25)** : vérifié dans Chromium
  (`chrome-headless-shell`, `--window-size=360,…`, `file://`), 4 captures
  dans [`../assets/us-111/`](../assets/us-111/) — voir la section
  « Vérification visuelle » en tête de fiche.
- **SonarCloud (PR #70, 2026-09-25)** : 3 *code smells* `MINOR` signalés sur
  `app.js` (`javascript:S7781`, `S7750`, `S6594`) et corrigés — voir le
  journal du 2026-09-25. Comportement inchangé, pas de nouveau test ajouté
  (déjà couvert par l'exécution Node.js manuelle décrite ci-dessus).

## Limites connues / TODO

- Mode sombre non vérifié à 360 px (seulement à ~500 px, voir en tête de
  fiche).
- Pas de branchement sur l'API réelle (US-217/US-219) : tout est en dur dans
  `data.js`.
- Pas de rafraîchissement automatique (SSE) — hors périmètre de l'US-111,
  prévu par la conception pour un sprint ultérieur.
- Pas de carte réseau, flotte de relais, intégrité des journaux ni recherche
  de logs (`docs/synthese/09` §5) : ces écrans sont des USs séparées, pas
  couvertes ici.

## Pour l'oral

C'est la première preuve visuelle du principe « vue partielle, jamais de
contenu » qui définit tout le dashboard : la page ne sait montrer que ce que
des nœuds ayant eu du Wi-Fi ont bien voulu rapporter, jamais qui a écrit quoi
à qui. Le point à montrer : le message « inconnu » (aucun saut reçu) n'est pas
un bug, c'est un état représentable — exactement la garantie de
confidentialité et la contrainte réseau (relais opportunistes) que la
conception assume.
