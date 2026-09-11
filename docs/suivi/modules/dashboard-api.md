# Module : `dashboard/api` (FastAPI)

**Rôle en une phrase :** recevoir les batchs d'événements que les nœuds poussent
en HTTPS, et servir plus tard le parcours + l'état de chaque message.
**Correspond à la conception :** [`docs/synthese/09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md)
§2 (architecture), §3 (ingestion), §11.2 (schéma cible).
**Dernière mise à jour :** 2026-09-11
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
    config.py            — db_path(), max_batch_bytes() : lisent l'env à chaque appel
    migrations.py        — MIGRATIONS : liste (version, nom, [instructions SQL]), littéraux du module
    db.py               — connect() ; run_migrations() : atomique + sûr en concurrence
    main.py             — app FastAPI ; lifespan → migrations + connexion partagée ; routes /healthz et /ingest/batch
  tests/
    conftest.py          — fixture `client` : base SQLite jetable par test, migrée par le lifespan
    test_api.py          — 16 tests (voir plus bas)
```

## Concepts / types importants

| Élément | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `db_path()` | `app/config.py` | chemin du fichier SQLite, lu depuis l'env à chaque appel (pas de cache → tests simples) |
| `max_batch_bytes()` | `app/config.py` | taille max acceptée pour `/ingest/batch`, lue depuis l'env à chaque appel (défaut 2 MiB) |
| `MIGRATIONS` | `app/migrations.py` | liste `(version, nom, [instructions])` ; littéraux du module (pas de fichier lu), versionnés, embarqués, sans I/O disque ; chaque `CREATE` en `IF NOT EXISTS` |
| `connect()` | `app/db.py` | connexion SQLite : `isolation_level=None` (transactions explicites), `check_same_thread=False` (réutilisée depuis le threadpool), `WAL`, `busy_timeout=5000`, `foreign_keys=ON` |
| `run_migrations(conn)` | `app/db.py` | applique les migrations manquantes ; ensemble déjà-appliquées calculé une fois avant la boucle ; chaque migration dans un `BEGIN IMMEDIATE` (DDL + `INSERT schema_migrations` = tout ou rien), revérification **fraîche** de la version sous verrou → sûr avec `uvicorn --workers N` ; idempotent |
| `lifespan` | `app/main.py` | au démarrage : ouvre **une** connexion, migre, la garde sur `app.state.db_conn` + `app.state.db_lock` (fermée à l'arrêt) |
| `GET /healthz` | `app/main.py` | route **sync** → threadpool FastAPI ; renvoie `{"status": "ok"}` |
| `_guess_event_count()` | `app/main.py` | devine le nombre d'événements (`[...]` ou `{"events": [...]}`) sans imposer de schéma ; `None` sinon, jamais de rejet |
| `_read_limited_body()` | `app/main.py` | lit `request.stream()` par morceaux, coupe dès que `max_batch_bytes()` est dépassé (`Content-Length` en fast-path, comptage réel sinon) |
| `_store_raw_batch()` | `app/main.py` | insertion sur la connexion **partagée** de l'app, sous `db_lock` |
| `_decode_parse_and_store()` | `app/main.py` | décode UTF-8 + `json.loads` + stockage, **en un seul aller-retour threadpool** — le décodage/parsing est le travail CPU dominant d'un gros batch, pas juste l'INSERT |
| `POST /ingest/batch` | `app/main.py` | lit le corps borné (413 si trop gros) → `run_in_threadpool(_decode_parse_and_store, …)` → `202` ; `400` si UTF-8/JSON invalide, `503` + `Retry-After` si la base est verrouillée |

## Flux principal (exemple)

```
un nœud : POST /ingest/batch   body = {"events":[{...},{...}]}
  main.ingest_batch (async)
    raw = await _read_limited_body(request, max_batch_bytes())  → 413 si trop gros
    await run_in_threadpool(_decode_parse_and_store, conn, lock, raw, ...)
       text = raw.decode("utf-8")          → UnicodeDecodeError → 400
       parsed = json.loads(text)           → JSONDecodeError → 400
       event_count = 2                     (_guess_event_count)
       batch_id = uuid4()
       _store_raw_batch(conn, lock, ...)   → INSERT sous le verrou, connexion partagée
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
- **Corps borné (`_read_limited_body`, 2 MiB par défaut)** (retour de revue
  #59) : `request.body()` n'a pas de limite native — un nœud buggé ou
  malveillant pourrait épuiser la mémoire du process (et `/healthz` avec
  lui). Lecture en flux plutôt que `body()` : rejet rapide sur
  `Content-Length`, comptage réel sinon (couvre l'absence de l'en-tête ou un
  mensonge dessus).
- **Décodage + parsing regroupés avec l'écriture dans un seul
  `run_in_threadpool`** (retour de revue #59) : décoder l'UTF-8 et parser le
  JSON sont le travail CPU dominant d'un gros batch, plus que l'INSERT ; les
  laisser sur la boucle d'événements aurait annulé l'intérêt du passage en
  thread pour `/healthz`.
- **Connexion SQLite unique, réutilisée** (retour de revue #59) : ouverte au
  démarrage (`lifespan`), stockée sur `app.state.db_conn`. Ouvrir un fichier
  + poser 3 `PRAGMA` à chaque `POST` est un coût redondant sous charge
  réelle. `check_same_thread=False` + `threading.Lock` (`app.state.db_lock`)
  parce que la connexion est maintenant utilisée depuis le threadpool, donc
  depuis un thread différent de celui qui l'a ouverte — le verrou protège
  l'objet Python `Connection`, pas SQLite (qui ne fait qu'un écrivain de
  toute façon). Ne protège que la concurrence **intra-process** : plusieurs
  `uvicorn --workers` restent chacun leur connexion/verrou, couverts par le
  `BEGIN IMMEDIATE` des migrations et le `busy_timeout` des écritures.
- **`PRAGMA busy_timeout` seul, `timeout=` retiré de `connect()`** (retour
  de revue #59) : les deux réglaient le même délai (5000 ms = 5.0 s), un des
  deux était mort et aurait pu diverger silencieusement d'un futur
  changement de l'autre.

## Tests

- `tests/test_api.py` — **16 tests** : `/healthz` ; objet arbitraire (202,
  `event_count` = 3) ; tableau nu ; corps **verbatim** en base ; non-JSON → 400 ;
  **JSON non-UTF-8 → 400** ; **corps trop gros → 413** / **dans la limite → 202** ;
  **une seule connexion ouverte pour 5 écritures** (compteur sur `connect()`
  monkeypatché) ; **20 écritures concurrentes sans collision ni perte**
  (`ThreadPoolExecutor`) ; **base verrouillée → 503 + `Retry-After`** ;
  migrations appliquées une fois ; **migrations idempotentes après DDL partiel** ;
  3 formes de payload paramétrées.
- Commande : depuis `dashboard/api/`, `uv sync --extra dev` puis
  `uv run ruff check .`, `uv run ruff format --check .` et `uv run pytest` →
  **16 passed** (vérifié le 2026-09-11). Test de concurrence rejoué 5 fois de
  suite sans échec.

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
