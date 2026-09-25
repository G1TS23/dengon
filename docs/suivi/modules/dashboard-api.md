# Module : `dashboard/api` (FastAPI)

**Rôle en une phrase :** recevoir les batchs d'événements que les nœuds poussent
en HTTPS, et servir plus tard le parcours + l'état de chaque message.
**Correspond à la conception :** [`docs/synthese/09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md)
§2 (architecture), §3 (ingestion), §11.2 (schéma cible).
**Dernière mise à jour :** 2026-09-25
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
    test_api.py          — 21 tests (voir plus bas)
```

## Concepts / types importants

| Élément | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `db_path()` | `app/config.py` | chemin du fichier SQLite, lu depuis l'env à chaque appel (pas de cache → tests simples) |
| `max_batch_bytes()` | `app/config.py` | taille max acceptée pour `/ingest/batch`, lue depuis l'env à chaque appel (défaut 2 MiB) |
| `MIGRATIONS` | `app/migrations.py` | liste `(version, nom, [instructions])` ; littéraux du module (pas de fichier lu), versionnés, embarqués, sans I/O disque ; chaque `CREATE` en `IF NOT EXISTS` |
| `connect()` | `app/db.py` | connexion SQLite : `isolation_level=None` (transactions explicites), `check_same_thread=False` (réutilisée depuis le threadpool), `busy_timeout=5000` **posé avant** `WAL` (voir décisions), `foreign_keys=ON` |
| `_set_wal_mode_with_retry()` | `app/db.py` | bascule en WAL avec re-tentatives manuelles courtes — `busy_timeout` seul ne protège pas ce PRAGMA de façon fiable (voir décisions) |
| `LockedConnection` | `app/db.py` | couple connexion SQLite et verrou dans un seul objet (`execute()`/`close()`) — remplace les attributs séparés `db_conn`/`db_lock` (voir décisions) |
| `run_migrations(conn)` | `app/db.py` | applique les migrations manquantes ; ensemble déjà-appliquées calculé une fois avant la boucle ; chaque migration dans un `BEGIN IMMEDIATE` (DDL + `INSERT schema_migrations` = tout ou rien), revérification **fraîche** de la version sous verrou → sûr avec `uvicorn --workers N`, **vérifié avec de vrais process OS** (voir Tests) ; idempotent |
| `lifespan` | `app/main.py` | au démarrage : ouvre **une** connexion (dans le `try`, voir décisions), migre, la garde sur `app.state.db` (`LockedConnection`, fermée à l'arrêt) |
| `GET /healthz` | `app/main.py` | route **sync** → threadpool FastAPI ; renvoie `{"status": "ok"}` |
| `_guess_event_count()` | `app/main.py` | devine le nombre d'événements (`[...]` ou `{"events": [...]}`) sans imposer de schéma ; `None` sinon, jamais de rejet |
| `_read_limited_body()` | `app/main.py` | lit `request.stream()` par morceaux, coupe dès que `max_batch_bytes()` est dépassé (`Content-Length` en fast-path, comptage réel sinon) ; vide le flux restant avant de lever dans les **deux** branches |
| `_store_raw_batch()` | `app/main.py` | insertion via `LockedConnection.execute()` |
| `_decode_parse_and_store()` | `app/main.py` | décode UTF-8 + `json.loads` + stockage, **en un seul aller-retour threadpool** — le décodage/parsing est le travail CPU dominant d'un gros batch, pas juste l'INSERT |
| `POST /ingest/batch` | `app/main.py` | lit le corps borné (413 si trop gros) → `run_in_threadpool(_decode_parse_and_store, …)` → `202` ; `400` si UTF-8/JSON invalide, `503` + `Retry-After` si la base est verrouillée |

## Flux principal (exemple)

```
un nœud : POST /ingest/batch   body = {"events":[{...},{...}]}
  main.ingest_batch (async)
    raw = await _read_limited_body(request, limite)   → 413 si trop gros
    await run_in_threadpool(_decode_parse_and_store, db, raw, ...)
       text = raw.decode("utf-8")          → UnicodeDecodeError → 400
       parsed = json.loads(text)           → JSONDecodeError → 400
       event_count = 2                     (_guess_event_count)
       batch_id = uuid4()
       _store_raw_batch(db, ...)           → INSERT via LockedConnection (verrou + connexion couplés)
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
  démarrage (`lifespan`), stockée sur `app.state.db` (`LockedConnection`
  depuis le round 4, voir plus bas). Ouvrir un fichier + poser les `PRAGMA` à
  chaque `POST` est un coût redondant sous charge réelle.
  `check_same_thread=False` + le verrou de `LockedConnection`
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
- **`max_batch_bytes()` validée une fois au démarrage** (`lifespan`), en plus
  d'être relue à chaque requête (retour de revue #59, round 2) : une valeur
  malformée de `DENGON_DASHBOARD_MAX_BATCH_BYTES` (ex. `"2MB"`) échoue tout de
  suite au démarrage plutôt que de faire planter chaque `POST /ingest/batch`
  avec un 500 sans rapport apparent avec la config.
- **`RecursionError` ajoutée à la clause d'exception 400** de `ingest_batch`
  (retour de revue #59, round 2) : un JSON très imbriqué fait planter
  `json.loads` par dépassement de la pile Python plutôt que de lever
  `JSONDecodeError` — reproduit en local, il faut ~10000 niveaux (pas 2000)
  pour un corps de ~20 Ko. Sans le fix : 500 brut au lieu du 400 attendu pour
  un corps invalide.
- **`sqlite3.ProgrammingError` ajoutée à la clause 503** (retour de revue
  #59, round 2) : si `conn.close()` (arrêt du `lifespan`) entre en course
  avec une écriture encore en cours sur le threadpool, SQLite lève
  `ProgrammingError` ("Cannot operate on a closed database"), pas
  `OperationalError` — reproduit en fermant `app.state.db` avant un
  `POST` (`app.state.db_conn` avant le round 4).
- **`BEGIN IMMEDIATE` des migrations volontairement hors du `try/except`
  qui fait `ROLLBACK`** (retour de revue #59, round 2) : si le verrou n'est
  pas obtenu avant `busy_timeout`, aucune transaction n'est ouverte — y
  inclure `BEGIN IMMEDIATE` lèverait une seconde erreur ("no transaction is
  active") qui masquerait la vraie cause. Le docstring du module est corrigé
  en conséquence : « sûr » veut dire jamais appliquée deux fois / jamais à
  moitié, pas « démarre toujours ».
- **`_read_limited_body` vide le flux avant de lever `_BodyTooLarge`** sur le
  fast-path `Content-Length` (retour de revue #59, round 2) : sans ça, le
  corps trop volumineux reste non lu sur la connexion au moment du 413, ce
  qui peut forcer uvicorn/h11 à couper la connexion keep-alive au lieu de
  livrer la réponse proprement.
- **`.github/workflows/dashboard.yml` : filtre de chemin déplacé du
  déclencheur vers un `if:` de job** (`dorny/paths-filter`, retour de revue
  #59, round 2) — même piège documenté et déjà corrigé pour `core.yml`
  (`docs/suivi/03-ecarts-conception.md`) : un filtre `on.pull_request.paths`
  empêche GitHub de créer un check du tout pour une PR hors `dashboard/**`,
  qui resterait bloquée sur « En attente » si ce check devient requis.
- **`connect()` pose `busy_timeout` AVANT `journal_mode = WAL`, avec
  re-tentatives manuelles sur ce dernier** (retour de revue #59, round 4,
  point d'OswinFreyr — révélé en écrivant un vrai test multi-process) :
  basculer en WAL prend un verrou distinct du verrou d'écriture habituel, et
  `busy_timeout` ne le protège pas de façon fiable — plusieurs process qui
  ouvrent le même fichier neuf en même temps (`uvicorn --workers N` au tout
  premier démarrage) peuvent chacun lever `sqlite3.OperationalError:
  database is locked` ici, même avec `busy_timeout` déjà réglé. Piège SQLite
  connu, résolu par une courte boucle de re-tentative (`_set_wal_mode_with_retry`,
  20 × 50 ms).
- **`LockedConnection` remplace `app.state.db_conn` + `app.state.db_lock`**
  (retour de revue #59, round 4, point d'OswinFreyr) : l'invariant « jamais
  la connexion sans le verrou » n'existait qu'en commentaire — une future
  route (US-217) aurait pu appeler `db_conn.execute(...)` directement en
  oubliant le verrou. `execute()` est maintenant la seule façon d'utiliser la
  connexion depuis l'extérieur du module, le verrou est tenu structurellement.
- **`connect()`/`run_migrations()` déplacés dans le `try` du `lifespan`**
  (retour de revue #59, round 4, point d'OswinFreyr) : avant, une migration
  qui échoue (ex. `OperationalError` après `busy_timeout`) laissait la
  connexion fuiter, `conn.close()` n'étant jamais atteint.
- **`_read_limited_body` vide aussi le flux dans la branche de comptage réel
  (sans `Content-Length`)** (retour de revue #59, round 4, point
  d'OswinFreyr) : le fix du round 2 ne couvrait que la branche
  `Content-Length` ; l'oubli laissait la connexion avec du corps non lu au
  moment du 413 dans le cas chunked/malveillant, justement celui que ce
  garde-fou vise en premier lieu.
- **`max_batch_bytes()` lue une seule fois par requête** dans une variable
  locale (retour de revue #59, round 4, point d'OswinFreyr) : appelée deux
  fois avant (une pour la limite, une pour le message d'erreur), un
  changement de la variable d'environnement entre les deux aurait pu faire
  annoncer une limite différente de celle réellement appliquée.
- **CI `dashboard.yml` : étape de build du wheel ajoutée**, en plus des tests
  qui importent `app` via `sys.path` (retour de revue #59, round 4, point
  d'OswinFreyr) : `packages = ["app"]` de `pyproject.toml` n'était vérifié par
  rien — un sous-paquet ajouté sous `app/` sans mise à jour de cette liste
  romprait l'installation réelle sans que la CI le voie. Vérifié en ajoutant
  volontairement un sous-module non déclaré : absent du wheel construit,
  l'étape l'aurait détecté.

## Tests

- `tests/test_api.py` — **21 tests** : `/healthz` ; **démarrage refusé sur
  `DENGON_DASHBOARD_MAX_BATCH_BYTES` malformé** (`RuntimeError` propagée par
  `lifespan`, retour de revue #59, round 4) ; objet arbitraire (202,
  `event_count` = 3) ; tableau nu ; corps **verbatim** en base ; non-JSON → 400 ;
  **JSON non-UTF-8 → 400** ; **JSON très imbriqué (10000 niveaux) → 400, pas
  500** ; **corps trop gros → 413** (fast-path `Content-Length`, avec preuve
  qu'aucune ligne n'est stockée) / **idem en chunked sans `Content-Length`**
  (le cas malveillant réel — le premier test seul n'exerçait que le
  fast-path, retour de revue #59, round 2) / **dans la limite → 202** ;
  **une seule connexion ouverte pour 5 écritures** (compteur sur `connect()`
  monkeypatché) ; **20 écritures concurrentes sans collision ni perte**,
  vérifié par un vrai `SELECT COUNT(*)` (pas seulement l'unicité des
  `batch_id`, retour de revue #59, round 2) ; **base verrouillée → 503 +
  `Retry-After`** ; **connexion fermée sous une écriture → 503 +
  `Retry-After`** (pas 500) ; migrations appliquées une fois ; **migrations
  idempotentes après DDL partiel** ; **migrations sûres avec de vrais process
  OS** (5 `multiprocessing.Process`, pas juste des threads — retour de revue
  #59, round 4, point d'OswinFreyr : a révélé le bug de `busy_timeout`/`WAL`
  documenté plus haut) ; 3 formes de payload paramétrées.
- Commande : depuis `dashboard/api/`, `uv sync --extra dev` puis
  `uv run ruff check .` et `uv run pytest` → **21 passed** (vérifié le
  2026-09-25). Chaque nouveau bug (RecursionError, ProgrammingError) reproduit
  d'abord en isolant le code sans le fix, confirmé absent avec.

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
