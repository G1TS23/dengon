# Module : `dashboard/api` (FastAPI)

**Rôle en une phrase :** recevoir les batchs d'événements que les nœuds poussent
en HTTPS, et servir plus tard le parcours + l'état de chaque message.
**Correspond à la conception :** [`docs/synthese/09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md)
§2 (architecture), §3 (ingestion), §11.2 (schéma cible).
**Dernière mise à jour :** 2026-09-10
**État :** esquisse — squelette **permissif** (US-110). `/healthz` + `/ingest/batch`
qui stocke le JSON brut. Aucune validation, aucune signature, aucune projection.

Cette fiche tient aussi lieu de **note d'onboarding de l'area `dashboard-api`**
(proposition d'organisation §10.3 point 3).

## À quoi ça sert

Le dashboard **observe**, il n'agit pas : il ne route rien, n'injecte rien, ne
déchiffre rien. Les nœuds (relais ESP32, `dengon-node`) lui envoient des
**événements** — « message X relayé par le nœud N à telle heure » — quand ils
ont une fenêtre Wi-Fi. L'API les ingère, les dédup, reconstruit un statut par
message et pousse le tout aux opérateurs connectés en SSE.

Au **squelette** (cette version), seule l'ingestion existe, et elle est
**volontairement permissive** : elle accepte n'importe quel JSON et le range
tel quel. Le format d'événement n'est pas encore figé (US-108, en cours) ; le
dashboard ne doit pas l'attendre pour démarrer — cf. proposition d'organisation
§3.3.

## Structure

```
dashboard/api/
  pyproject.toml         — deps (fastapi, uvicorn) + extra `dev` (pytest, httpx, ruff) + config ruff/pytest
  uv.lock                — lock des deps (transitives + hash) ; géré par uv, fait foi en CI
  app/
    config.py            — db_path() : lit DENGON_DASHBOARD_DB (défaut dashboard.db)
    migrations.py        — MIGRATIONS : liste (version, nom, [instructions SQL]), littéraux du module
    db.py               — connect() ; run_migrations() : atomique + sûr en concurrence
    main.py             — app FastAPI ; lifespan → migrations ; routes /healthz et /ingest/batch
  tests/
    conftest.py          — fixture `client` : base SQLite jetable par test, migrée par le lifespan
    test_api.py          — 12 tests (voir plus bas)
```

## Concepts / types importants

| Élément | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `db_path()` | `app/config.py:19` | chemin du fichier SQLite, lu depuis l'env à chaque appel (pas de cache → tests simples) |
| `MIGRATIONS` | `app/migrations.py` | liste `(version, nom, [instructions])` ; littéraux du module (pas de fichier lu), versionnés, embarqués, sans I/O disque ; chaque `CREATE` en `IF NOT EXISTS` |
| `connect()` | `app/db.py` | connexion SQLite : `isolation_level=None` (transactions explicites), `WAL`, `busy_timeout=5000`, `foreign_keys=ON` |
| `run_migrations(conn)` | `app/db.py` | applique les migrations manquantes ; chaque migration dans un `BEGIN IMMEDIATE` (DDL + `INSERT schema_migrations` = tout ou rien), revérification de la version sous verrou → sûr avec `uvicorn --workers N` ; idempotent |
| `lifespan` | `app/main.py` | au démarrage de l'app : ouvre une connexion, migre, ferme |
| `GET /healthz` | `app/main.py` | route **sync** → threadpool FastAPI ; renvoie `{"status": "ok"}` |
| `_guess_event_count()` | `app/main.py` | devine le nombre d'événements (`[...]` ou `{"events": [...]}`) sans imposer de schéma ; `None` sinon, jamais de rejet |
| `_store_raw_batch()` | `app/main.py` | insertion synchrone d'un batch ; ouvre sa propre connexion (appelée dans un thread) |
| `POST /ingest/batch` | `app/main.py` | décode UTF-8 → `json.loads` (400 si non-UTF-8 ou non-JSON) → INSERT via `run_in_threadpool` → `202` ; `503` + `Retry-After` si la base est verrouillée |

## Flux principal (exemple)

```
un nœud : POST /ingest/batch   body = {"events":[{...},{...}]}
  main.ingest_batch (async)
    raw = await request.body()
    text = raw.decode("utf-8")          → 400 si non-UTF-8
    parsed = json.loads(text)           → 400 si non-JSON
    batch_id = uuid4()
    event_count = 2                     (_guess_event_count)
    await run_in_threadpool(_store_raw_batch, ...)   → INSERT dans un thread
       └─ sqlite3.OperationalError ?    → 503 + Retry-After: 1
  → 202  {"stored": true, "batch_id": "...", "event_count": 2}
```

La table `raw_batches` est une **zone d'atterrissage**. US-216 (S2) ajoutera la
table `events` normalisée, la validation de schéma et la vérification de
signature Ed25519 ; US-217 ajoutera les projections `messages` / `nodes` /
`links` / `message_hops`.

## Dépendances

- **Internes :** aucune pour l'instant. À terme : le binaire Rust `dengon-verify`
  (vérif de journal chaîné), appelé en sous-processus.
- **Externes :** `fastapi` (routes + lifespan), `uvicorn` (serveur ASGI local),
  `httpx` (client de test via `TestClient`), `pytest`, `ruff`. Gérées par
  **`uv`** (`uv.lock` = lock transitif + hash, fait foi en CI). Base :
  `sqlite3` de la stdlib — pas d'ORM, cohérent avec un squelette et avec le
  faible volume (démo de 5-8 appareils, base effacée par session, B-4).

## Décisions d'implémentation

- **Migrations maison** (`MIGRATIONS` + table `schema_migrations`) plutôt
  qu'Alembic : surdimensionné ici, une dépendance de moins.
- **SQL de migration en littéral de module**, pas en fichier `.sql` lu à
  l'exécution : versionné dans git de la même façon, mais embarqué dans le
  paquet (rien à copier au déploiement) et sans chemin de *taint*
  fichier→`executescript` (l'analyse de sécurité de SonarCloud flaggait la
  version « lecture de fichier »). Si le nombre de migrations grossit, repasser
  à des fichiers est un refactor propre.
- **`db_path()` relit l'env à chaque appel** au lieu d'un singleton : chaque
  test a sa base (`tmp_path`) sans recharger de module.
- **`202 Accepted`** et non `200` : l'ingestion est un dépôt asynchrone, pas une
  requête traitée sur le champ. Prépare la sémantique de US-216.
- **Décodage UTF-8 avant `json.loads`** (retour de revue #59) : `json.loads`
  accepterait de l'UTF-16/32, mais le stocker en `TEXT` le corromprait et US-216
  vérifiera une signature Ed25519 sur ces octets. `CANONICAL.md` impose l'UTF-8 →
  on rejette (400) ce qui n'en est pas, plutôt que `errors="replace"`.
- **I/O SQLite hors boucle d'événements** (retour de revue #59) : la route est
  `async` (pour `await request.body()`) mais l'écriture passe par
  `run_in_threadpool` — sinon un flush concurrent gèlerait la boucle et une
  sonde de liveness sur `/healthz` pourrait expirer.
- **`503` + `Retry-After` sur base verrouillée** (retour de revue #59) : SQLite
  n'a qu'un écrivain ; le perdant d'un flush simultané reçoit un signal de
  retenter, il ne perd pas son batch sur un 500.
- **Migrations atomiques et sûres en concurrence** (retour de revue #59) :
  `BEGIN IMMEDIATE` + revérification sous verrou + `CREATE … IF NOT EXISTS`.
  Un crash entre le DDL et l'enregistrement de version, ou deux workers au
  démarrage, ne cassent plus tous les démarrages suivants.

## Tests

- `tests/test_api.py` — **12 tests** : `/healthz` ; objet arbitraire (202,
  `event_count` = 3) ; tableau nu ; corps **verbatim** en base ; non-JSON → 400 ;
  **JSON non-UTF-8 → 400** ; **base verrouillée → 503 + `Retry-After`** ;
  migrations appliquées une fois ; **migrations idempotentes après DDL partiel** ;
  3 formes de payload paramétrées.
- Commande : depuis `dashboard/api/`, `uv sync --extra dev` puis
  `uv run ruff check .` et `uv run pytest` → **12 passed** (vérifié le
  2026-09-10).

## Limites connues / TODO

- **Aucune sécurité** : ni JWT, ni liste blanche, ni signature Ed25519, ni rejet
  d'un `msgID` non haché. → US-216.
- **Aucune projection** : on ne sait pas encore reconstruire un statut de
  message. → US-217.
- **Pas de SSE**, pas de routes REST de lecture. → US-218, US-219.
- **Pas de déploiement** : tourne en local via `uvicorn`. Les migrations
  tournent dans le `lifespan` — US-224 pourra les sortir en étape explicite. →
  US-224 (VPS + TLS).
- 2 `DeprecationWarning` (`httpx`/`anyio`) en local ; sans effet, absents en CI.

## Pour l'oral

Le dashboard est un **témoin**, pas un maillon : on peut l'éteindre, le réseau
de messages continue exactement pareil. Ce squelette montre le point d'entrée —
les nœuds y déposent des lots d'événements — et il est **exprès trop gentil** :
il gobe n'importe quoi. C'est un choix d'organisation : il pouvait démarrer
avant que l'équipe ait figé le format exact des événements, ce qui a permis de
travailler les trois parties du projet en parallèle.
