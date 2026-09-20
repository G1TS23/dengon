# Module : `dashboard-web` (`dashboard/web/`)

**Rôle en une phrase :** la page web du dashboard d'observabilité — liste des
messages suivis + écran de détail (parcours), pour l'instant sur données
bidon.
**Correspond à la conception :** [`docs/olivier/dashboard.md`](../../olivier/dashboard.md)
§3, §4, §8, §9 ; [`docs/synthese/09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md)
§5 (« Parcours d'un message »), §11.2 (schéma SQLite `messages`/`message_hops`).
**Dernière mise à jour :** 2026-09-20
**État :** esquisse (US-111) — page statique, données en dur, aucun appel
réseau. Pas encore branchée sur l'API (US-217, US-219).

## ⚠️ Vérification visuelle (CSS/mise en page) — non faite dans cette session

Le critère d'acceptation de l'US-111 demande un « rendu correct sur mobile
(largeur 360 px) » et sa stratégie de test est une « vérification manuelle sur
mobile + capture d'écran dans la PR ». **Aucun navigateur n'était disponible
dans l'environnement où ce code a été écrit** : l'extension Claude in Chrome a
été déclinée, et aucun binaire Chrome/Edge n'a été trouvé sur la machine (ni
dans les chemins standards, ni dans le registre `App Paths`).

Pour compenser partiellement, `app.js`/`data.js` ont été **réellement
exécutés** (pas juste relus) sous Node.js, contre un DOM minimal reconstitué
à la main (`createElement`/`appendChild`/`innerHTML`/`hashchange`, script
jetable non commité) : chargement des données, rendu de la liste,
navigation vers un détail à plusieurs sauts, cas `unknown` sans aucun saut,
id inconnu de la démo, retour à la liste. **Zéro exception, sortie texte
conforme** (dates UTC correctes, `—` partout où une donnée manque, `fanout: 0`
affiché comme `0` et non comme `—`). Complété par un recoupement automatique
(`grep`) entre les classes générées par `app.js` et les sélecteurs de
`style.css` : aucune classe orpheline des deux côtés. Ça couvre la classe de
bugs « logique JS incorrecte / faute de frappe dans un nom de classe ».

Ce qui **reste** non vérifié, parce qu'un DOM reconstitué à la main ne rend
aucun CSS : la mise en page réelle, le rendu à 360 px, les couleurs, le
comportement de la grille `auto-fill`. Reste à faire avant de clore
l'US-111 : ouvrir `dashboard/web/index.html` dans un navigateur (double-clic,
ou `file://.../index.html`), vérifier à 360 px de large (DevTools → mode
appareil), et joindre une capture d'écran à la PR.

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
  inconnu, retour liste), sortie texte conforme à l'attendu — voir
  l'avertissement en tête de fiche pour le détail. Ça couvre la logique ;
  **le rendu CSS réel dans un navigateur n'est pas vérifié** dans cette
  session — aucun navigateur disponible (extension Claude in Chrome
  déclinée, aucun binaire Chrome/Edge trouvé).
- **À faire avant de clore l'US-111** : ouvrir la page dans un navigateur,
  vérifier à 360 px de large, capture d'écran dans la PR.

## Limites connues / TODO

- Rendu jamais vérifié dans un vrai navigateur (voir avertissement en tête de
  fiche) — bloquant pour clore le DoR de l'US-111.
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
