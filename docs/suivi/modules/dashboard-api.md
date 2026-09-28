# Module : `dashboard/api` (FastAPI)

**Rôle en une phrase :** recevoir les batchs d'événements que les nœuds poussent
en HTTPS, les valider, et servir plus tard le parcours + l'état de chaque
message.
**Correspond à la conception :** [`docs/synthese/09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md)
§2 (architecture), §3 (ingestion), §11.2 (schéma cible).
**Dernière mise à jour :** 2026-09-28
**État :** `/healthz` + `/ingest/batch` **validé** (schéma + JWT + signature
Ed25519 + dédup, US-216) + `/api/nodes` (enregistrement, US-216). Pas encore
de projections (`messages`/`links`/`message_hops`), pas de SSE.

Cette fiche tient aussi lieu de **note d'onboarding de l'area `dashboard-api`**
(proposition d'organisation §10.3 point 3).

## À quoi ça sert

Le dashboard **observe**, il n'agit pas : il ne route rien, n'injecte rien, ne
déchiffre rien. Les nœuds (relais ESP32, `dengon-node`) lui envoient des
**événements** — « message X relayé par le nœud N à telle heure » — quand ils
ont une fenêtre Wi-Fi. L'API les valide, les dédup, reconstruit un statut par
message et pousse le tout aux opérateurs connectés en SSE.

Le squelette permissif de l'US-110 (accepter n'importe quel JSON, le stocker
tel quel) a servi le temps que le format d'événement se stabilise (US-107/
US-108) ; l'US-216 le remplace par un pipeline qui **rejette** ce qui ne
respecte pas le contrat.

## Structure

```
dashboard/api/
  pyproject.toml         — deps (fastapi, uvicorn) + extra `dev` (pytest, httpx, ruff) + config ruff/pytest
  uv.lock                — lock des deps (transitives + hash) ; géré par uv, fait foi en CI
  app/
    config.py            — db_path(), max_batch_bytes(), jwt_secret() : lisent l'env à chaque appel (jwt_secret sans défaut)
    migrations.py        — MIGRATIONS : liste (version, nom, [instructions SQL]), littéraux du module ; v2 = nodes + events
    db.py               — connect() ; run_migrations() : atomique + sûr en concurrence
    canonical.py         — canonical_json()/event_id() : copie volontaire de contracts/tools/catalogue.py (voir décisions)
    schemas.py            — batch_validator() : jsonschema Draft202012Validator contre contracts/events/batch.schema.json
    auth.py               — create_token()/node_id_from_authorization_header() : JWT HS256 courts (24h)
    ingest.py             — pipeline complet de validation (voir Flux principal)
    main.py             — app FastAPI ; lifespan → migrations + connexion partagée + jwt_secret() ; routes /healthz, /ingest/batch, /api/nodes
  tests/
    conftest.py          — fixture `client` : base SQLite jetable par test, migrée par le lifespan, secret JWT de test
    test_api.py          — 34 tests (voir plus bas)
```

## Concepts / types importants

| Élément | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `db_path()` | `app/config.py` | chemin du fichier SQLite, lu depuis l'env à chaque appel (pas de cache → tests simples) |
| `max_batch_bytes()` | `app/config.py` | taille max acceptée pour `/ingest/batch`, lue depuis l'env à chaque appel (défaut 2 MiB) |
| `jwt_secret()` | `app/config.py` | secret HMAC pour les JWT courts des nœuds ; **pas de défaut** (contrairement à `max_batch_bytes()`) — un défaut serait une clé maîtresse publique ; validé au démarrage (`lifespan`) |
| `MIGRATIONS` | `app/migrations.py` | liste `(version, nom, [instructions])` ; littéraux du module (pas de fichier lu), versionnés, embarqués, sans I/O disque ; chaque `CREATE` en `IF NOT EXISTS` ; v2 ajoute `nodes`/`events` + index |
| `canonical_json()`/`event_id()` | `app/canonical.py` | sérialisation canonique + `event_id = SHA-256(node_id ‖ seq_be_u64)` ; copie intentionnelle de `contracts/tools/catalogue.py` (`CANONICAL.md` : deux implémentations doivent converger indépendamment) |
| `batch_validator()` | `app/schemas.py` | `Draft202012Validator` sur `batch.schema.json`, résolution du `$ref` vers `envelope.schema.json` via `referencing.Registry` (lu par chemin filesystem, pas d'import cross-paquet — voir décisions) ; `lru_cache` |
| `create_token()`/`node_id_from_authorization_header()` | `app/auth.py` | JWT HS256, TTL par défaut 24h ; la seconde lève `InvalidToken` sur tout échec (en-tête absent/malformé, expiré, signature invalide, claim `node_id` manquant) |
| `ingest_batch()` | `app/ingest.py` | pipeline complet : parse JSON → schéma → `node_id` batch == `node_id` JWT → nœud whitelisté → signature Ed25519 → `event_id` recalculé → insertion idempotente (`INSERT OR IGNORE`) |
| `IngestError` | `app/ingest.py` | exception portant `status_code`/`detail`, traduite en réponse HTTP par la route |
| `connect()` | `app/db.py` | connexion SQLite : `isolation_level=None` (transactions explicites), `check_same_thread=False` (réutilisée depuis le threadpool), `busy_timeout=5000` **posé avant** `WAL` (voir décisions), `foreign_keys=ON` |
| `_set_wal_mode_with_retry()` | `app/db.py` | bascule en WAL avec re-tentatives manuelles courtes — `busy_timeout` seul ne protège pas ce PRAGMA de façon fiable (voir décisions) |
| `LockedConnection` | `app/db.py` | couple connexion SQLite et verrou dans un seul objet (`execute()`/`close()`) — remplace les attributs séparés `db_conn`/`db_lock` (voir décisions) |
| `run_migrations(conn)` | `app/db.py` | applique les migrations manquantes ; ensemble déjà-appliquées calculé une fois avant la boucle ; chaque migration dans un `BEGIN IMMEDIATE` (DDL + `INSERT schema_migrations` = tout ou rien), revérification **fraîche** de la version sous verrou → sûr avec `uvicorn --workers N`, **vérifié avec de vrais process OS** (voir Tests) ; idempotent |
| `lifespan` | `app/main.py` | au démarrage : `jwt_secret()` + `max_batch_bytes()` validés tôt, ouvre **une** connexion (dans le `try`, voir décisions), migre, la garde sur `app.state.db` (`LockedConnection`, fermée à l'arrêt) |
| `GET /healthz` | `app/main.py` | route **sync** → threadpool FastAPI ; renvoie `{"status": "ok"}` |
| `_read_limited_body()` | `app/main.py` | lit `request.stream()` par morceaux, coupe dès que `max_batch_bytes()` est dépassé (`Content-Length` en fast-path, comptage réel sinon) ; vide le flux restant avant de lever dans les **deux** branches |
| `POST /ingest/batch` | `app/main.py` | JWT vérifié **avant** de lire le corps (voir décisions) → corps borné (413 si trop gros) → `run_in_threadpool(ingest.ingest_batch, …)` → `202` avec `new_event_count` ; `ingest.IngestError` → `exc.status_code`/`exc.detail` ; `503` + `Retry-After` si la base est verrouillée |
| `POST /api/nodes` | `app/main.py` | enregistre un nœud (`node_id`/`kind`/`pub_sign` hex 32 octets), upsert dans `nodes` (`whitelisted=1` — pas d'auth opérateur, voir Limites), renvoie `{"node_id", "token"}` (201) |

## Flux principal (exemple)

```
un nœud : POST /ingest/batch   Authorization: Bearer <jwt>   body = {batch signé}
  main.ingest_batch_route (async)
    jwt_node_id = node_id_from_authorization_header(...)   → InvalidToken → 401 (corps PAS encore lu)
    raw = await _read_limited_body(request, limite)        → 413 si trop gros
    await run_in_threadpool(ingest.ingest_batch, db, jwt_node_id, raw)
       parsed = _parse_json(raw)                → 400 si UTF-8/JSON invalide
       body = _validate_schema(parsed)          → 400 si hors du schéma batch.schema.json
       body["node_id"] == jwt_node_id ?         → 401 sinon
       pub_sign = _lookup_node_pub_sign(...)     → 401 si nœud inconnu/non whitelisté
       _verify_signature(body, pub_sign)        → 401 si signature Ed25519 invalide
       _verify_event_ids(body)                  → 400 si event_id recalculé != envoyé
       new_count = _insert_events(...)          → INSERT OR IGNORE par event_id, idempotent
          └─ sqlite3.OperationalError ?          → 503 + Retry-After: 1
  → 202  {"batch_id": "...", "node_id": "...", "event_count": 2, "new_event_count": 2}
```

`raw_batches` (US-110) n'est plus écrite depuis l'US-216 — `/ingest/batch`
valide et route directement vers `events`. La table reste dans le schéma
(vide) plutôt qu'un `DROP TABLE` risqué sur une table déjà déployée. US-217
ajoutera les projections `messages` / `links` / `message_hops`.

## Dépendances

- **Internes :** `contracts/events/{batch,envelope}.schema.json` — lus par
  chemin filesystem relatif (`contracts/` n'est pas un paquet Python
  installable, voir décisions), pas d'import cross-paquet. À terme : le
  binaire Rust `dengon-verify` (vérif de journal chaîné), appelé en
  sous-processus (hors périmètre US-216, voir Limites).
- **Externes :** `fastapi`, `uvicorn`, `httpx`/`pytest`/`ruff` (dev). Ajoutées
  pour l'US-216 : `jsonschema`+`referencing` (validation de schéma, mêmes
  bornes de version que `contracts/pyproject.toml`), `pynacl` (signature
  Ed25519, idem `contracts/tools/validate.py`), `pyjwt` (JWT HS256). Gérées
  par **`uv`** (`uv.lock` = lock transitif + hash, fait foi en CI). Base :
  `sqlite3` de la stdlib — pas d'ORM, cohérent avec le faible volume (démo de
  5-8 appareils, base effacée par session, B-4).

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
- **Round 5 (retours d'OswinFreyr) :**
  - **Liste des modules attendus DÉRIVÉE de `git ls-files`** au lieu d'être
    codée en dur dans `dashboard.yml` : une liste figée ne détecte rien de
    plus qu'elle-même — un sous-paquet ajouté sous `app/` (ex.
    `app/routes/`) resterait absent des deux listes à la fois, exactement le
    cas que cette étape était censée détecter.
  - **`LockedConnection.locked()`** (context manager) ajouté à côté
    d'`execute()` : `execute()` ne tient le verrou que le temps d'une
    instruction — correct pour l'`INSERT` actuel, mais un futur `SELECT`
    (`db.execute(...).fetchall()`, US-217) lirait les lignes hors verrou.
    `locked()` expose la connexion pour toute la durée du bloc `with`, pour
    un `SELECT` suivi d'un `fetchall()` ou une séquence de plusieurs
    instructions liées.
  - **`_read_limited_body` vide le flux jusqu'à `_DRAIN_CAP_BYTES` (1 MiB),
    puis abandonne**, au lieu de vider sans borne : un client qui envoie en
    continu occupait la coroutine indéfiniment, la limite de taille ne
    bornait plus que la mémoire, pas le travail fourni. Passé le cap,
    uvicorn ferme la connexion plutôt que de livrer un 413 propre — compromis
    accepté. La branche de comptage réel continue désormais **la même**
    boucle `async for` au lieu de rappeler `request.stream()` (qui peut lever
    `RuntimeError("Stream consumed")` si tout le corps est arrivé en un seul
    message ASGI) — supprime le `except RuntimeError` qui rattrapait plus
    large que ce seul cas.
  - **`connect()` ferme la connexion si une étape après `sqlite3.connect()`
    échoue** (ex. `_set_wal_mode_with_retry` épuise ses tentatives) : avant,
    le fichier restait ouvert sans référence, personne ne le fermant —
    même classe de fuite que celle corrigée au round 4 pour `lifespan`,
    déplacée d'un cran plus tôt.
  - **`_set_wal_mode_with_retry` ne re-tente que sur `SQLITE_BUSY`/
    `SQLITE_LOCKED`**, pas sur n'importe quelle `OperationalError` : un
    `disk I/O error` ou `unable to open database file` ne se résout pas en
    attendant 1 s, et les rattraper masquait un vrai problème derrière 20
    tentatives inutiles.
  - **`test_migrations_are_safe_across_processes` : la barrière aligne les N
    process avant `connect()`**, pas seulement avant `run_migrations()` —
    c'est `connect()` (le passage en WAL) qui est la race concurrente
    qu'on veut forcer à coup sûr, pas seulement par chance selon l'ordre de
    démarrage `spawn`. `result_queue.get(timeout=5)` remplace
    `get_nowait()` (un résultat mis un peu tard faisait échouer le test sur
    un `queue.Empty` opaque plutôt que sur l'assertion), et les process
    encore vivants sont `terminate()`-és si le test échoue par timeout.
- **Round 6 (retours d'OswinFreyr) :**
  - **`_is_retryable_lock_error` comparait le mauvais niveau de code
    d'erreur** : `sqlite_errorcode` est le code **étendu** (`2067` pour
    `SQLITE_CONSTRAINT_UNIQUE`, vérifié en local), pas le code de base à
    comparer directement à `SQLITE_BUSY`/`SQLITE_LOCKED`. Sans le masque
    `& 0xFF`, le filtre du round 5 laissait passer sans re-tentative
    `SQLITE_BUSY_RECOVERY` (261) — précisément ce que SQLite renvoie quand
    un autre process récupère le WAL, le scénario `--workers N` que ce
    retry vise en premier lieu.
  - **`LockedConnection.locked()` fait un `rollback()` si le bloc `with`
    lève en pleine transaction** : sans ça, une exception entre `BEGIN` et
    `COMMIT` laissait la transaction ouverte sur la connexion **partagée** —
    les écritures suivantes s'y seraient agrégées sans jamais être
    commitées, et le `BEGIN` suivant aurait levé `cannot start a
    transaction within a transaction`. Personne n'utilise encore `locked()`
    (ajouté au round 5 pour un futur `SELECT`), mais c'est le piège qu'un
    appelant (US-217) aurait rencontré.
  - **Test ajouté pour le chemin de drain** (`_DRAIN_CAP_BYTES`,
    continuation dans la boucle `async for`, `_drain_bounded`) : aucun test
    existant ne l'exerçait, puisque `TestClient` (httpx) livre toujours le
    corps en un seul message ASGI, y compris via un générateur chunked. Le
    nouveau test pilote l'ASGI directement (`anyio.run(app, scope, receive,
    send)`), avec un `receive` qui renvoie le corps en petits morceaux
    (`more_body=True`) bien au-delà de `limit + _DRAIN_CAP_BYTES`, et
    vérifie que la lecture s'arrête à la borne plutôt que de tout
    consommer — confirmé détecter la régression en désactivant
    temporairement la borne (le test échoue alors sur `1034 < 1034`).
  - **CI `dashboard.yml` : échoue si `git ls-files 'app/*.py'` ne renvoie
    rien** (répertoire de travail changé, motif mal écrit…) — avant, la
    boucle ne s'exécutait jamais et l'étape restait verte sans avoir rien
    vérifié. `app/**/*.py` retiré (redondant : `*` traverse déjà les `/`
    dans un pathspec git).
- **Round 7 (retour d'OswinFreyr) :** `except (UnicodeDecodeError,
  json.JSONDecodeError, RecursionError)` élargi à `except (ValueError,
  RecursionError)` dans `ingest_batch` — `UnicodeDecodeError` et
  `json.JSONDecodeError` héritent tous deux de `ValueError`, et depuis
  Python 3.11 (`sys.int_max_str_digits`, limite anti-DoS à 4300 chiffres),
  `json.loads` lève un `ValueError` **générique** (pas `JSONDecodeError`)
  sur un littéral entier trop long. Vérifié en local :
  `json.loads('1'*5000)` → `ValueError: Exceeds the limit (4300 digits)...`.
  Un batch de ~5 Ko avec un tel littéral (très en dessous de la limite de
  taille) traversait l'ancien `except` et produisait un 500 au lieu du 400
  attendu — même classe de bug que le `RecursionError` déjà traité au
  round 2. Nouveau test `test_ingest_rejects_a_huge_integer_literal`.
- **Round 8 (retour d'OswinFreyr) :**
  - **Le drain du fast-path `Content-Length` saute désormais quand le
    client envoie `Expect: 100-continue`** (curl le fait par défaut
    au-delà de 1 MiB) : lire `request.stream()` avant ce point déclenche
    l'envoi de `100 Continue` par uvicorn, invitant le client à téléverser
    un corps qu'on s'apprête à rejeter — bande passante perdue, et le
    client peut voir une erreur d'envoi au lieu du 413 propre. Nouveau test
    `test_ingest_skips_drain_when_client_expects_100_continue` (espionne
    `_drain_bounded`, vérifie qu'il n'est pas appelé avec l'en-tête, l'est
    sans).
  - **`run_migrations` : le `ROLLBACK` du bloc `except` est maintenant
    gardé par `conn.in_transaction`**, même garde que
    `LockedConnection.locked()` : SQLite annule lui-même la transaction sur
    certaines erreurs (`SQLITE_FULL`, `SQLITE_IOERR`, `SQLITE_NOMEM`), et un
    `ROLLBACK` explicite sans garde levait alors `OperationalError: cannot
    rollback - no transaction is active`, masquant l'erreur d'origine dans
    `__context__`. Nouveau test
    `test_migration_error_survives_a_transaction_sqlite_already_closed`,
    qui simule le cas avec une migration dont une instruction fait
    `COMMIT` avant qu'une instruction invalide ne lève — confirmé détecter
    la régression (sans la garde, l'erreur `cannot rollback` remonte à la
    place de la vraie erreur).
- **US-216 (ingestion validée) :**
  - **JWT vérifié avant lecture du corps** : une requête non authentifiée
    n'atteint jamais `_read_limited_body` — évite de déclencher un
    `100 Continue`/le streaming du corps pour un batch qui sera rejeté de
    toute façon. A obligé à ajouter un en-tête `Authorization` valide aux
    tests de limite de taille hérités des rounds précédents (413/drain/
    100-continue/chunked), qui vérifiaient jusque-là seulement le comportement
    de lecture du corps, pas l'auth.
  - **`contracts/` lu par chemin filesystem plutôt qu'importé** :
    restructurer `contracts/pyproject.toml` en paquet installable aurait été
    un changement plus large et plus risqué que de lire les fichiers JSON du
    schéma directement (`Path(__file__).resolve().parents[3] / "contracts" /
    "events"`). Seules les petites fonctions `canonical_json`/`event_id` sont
    dupliquées (`app/canonical.py`), pas le schéma — la partie la plus
    volumineuse et la plus sujette à divergence reste une source unique.
  - **`jwt_secret()` sans valeur par défaut**, contrairement à
    `max_batch_bytes()` : un secret par défaut serait une clé maîtresse
    connue de tous les déploiements, exactement ce qu'un jeton court est
    censé empêcher. Échoue au démarrage (`lifespan`), même logique que
    `max_batch_bytes()`.
  - **`event_id` recalculé côté serveur**, pas seulement validé en format
    (64 hex) par le schéma : un `event_id` forgé mais bien formé pourrait
    empoisonner la déduplication (masquer un vrai événement derrière un faux
    `event_id` qui collisionne). Vérifié indépendamment de la signature —
    `test_ingest_rejects_tampered_event_id` tamper l'`event_id` **avant** de
    signer, pour prouver que la vérification ne se contente pas de la
    signature.
  - **Message identique pour "nœud inconnu" et "nœud existant mais non
    whitelisté"** (401 générique) : distinguer les deux ne renseignerait
    qu'un attaquant essayant de deviner des `node_id` valides. Écart vs
    conception : `docs/synthese/09` §3 prévoit une quarantaine + alerte
    opérateur pour un nœud inconnu, traité ici comme un rejet direct — pas
    d'écran opérateur pour lever une quarantaine dans ce périmètre (voir
    `03-ecarts-conception.md`).
  - **`POST /api/nodes` non authentifié** (pas d'auth opérateur) : correspond
    à la description de déploiement de `docs/synthese/09-dashboard-et-donnees.md`
    §7, mais documenté dans le docstring de la route et dans
    `03-ecarts-conception.md` comme un vrai manque avant déploiement réel —
    aucune US du backlog actuel ne couvre l'auth opérateur/admin.
  - **`raw_batches` gardée mais plus écrite** plutôt que supprimée par
    migration : une table déjà déployée qu'on `DROP` est un risque de plus
    pour un gain nul (elle est vide et inoffensive).

## Tests

- `tests/test_api.py` — **34 tests**. Hérités et adaptés (auth ajoutée aux
  tests de taille/drain) : `/healthz` ; démarrage refusé sur
  `DENGON_DASHBOARD_MAX_BATCH_BYTES` malformé ; **démarrage refusé si
  `DENGON_DASHBOARD_JWT_SECRET` absent** (nouveau) ; corps trop gros → 413
  (fast-path `Content-Length`) / drain sauté avec `Expect: 100-continue` /
  idem en chunked sans `Content-Length` / dans la limite → 202 ; une seule
  connexion ouverte pour 5 écritures ; 20 écritures concurrentes sans
  collision ni perte (vérifié sur `events`, plus `raw_batches`) ; base
  verrouillée → 503 ; connexion fermée sous une écriture → 503 ; migrations
  appliquées une fois / idempotentes après DDL partiel / sûres avec de vrais
  process OS (`versions == [1, 2]` désormais) ; drain borné via appel ASGI
  direct.
  Nouveaux pour l'US-216 : `test_register_node_returns_a_usable_token` ;
  `test_register_node_rejects_malformed_bodies` (5 cas paramétrés) ;
  `test_ingest_rejects_missing_authorization` /
  `_malformed_authorization_header` / `_expired_or_forged_jwt` ;
  `test_ingest_rejects_unregistered_node` (node_id bien formé mais absent de
  `nodes` — piège trouvé en rédigeant le test : un `node_id` mal formé
  déclenche le rejet de schéma avant même d'atteindre la whitelist,
  corrigé) ; `test_ingest_rejects_node_id_mismatch_between_jwt_and_batch` ;
  `test_ingest_rejects_forged_signature` ;
  `test_ingest_rejects_batch_failing_schema_validation` ;
  `test_ingest_rejects_non_json_body` ;
  `test_ingest_rejects_a_huge_integer_literal` ;
  `test_ingest_rejects_tampered_event_id` (tamper **avant** signature, pour
  prouver que la vérification d'`event_id` est indépendante de celle de la
  signature) ; `test_ingest_accepts_a_valid_signed_batch_and_stores_it` ;
  `test_ingest_is_idempotent_on_exact_replay` (`new_event_count` = 0 au
  rejeu, vérifié aussi par `SELECT COUNT(*)`) ;
  `test_ingest_accepts_multiple_events_in_one_batch`.
- Commande : depuis `dashboard/api/`, `uv sync --extra dev` puis
  `uv run ruff check .` (« All checks passed! ») et `uv run pytest` →
  **34 passed** (vérifié le 2026-09-28). Bug trouvé en écrivant les tests :
  `test_ingest_rejects_unregistered_node` utilisait d'abord un `node_id` au
  mauvais format (`relay-inconnu`), qui échoue la validation de schéma (400)
  avant d'atteindre le code testé (401 attendu) — corrigé en utilisant un
  `node_id` bien formé mais non enregistré (`relay-9a9a9a`).

## Limites connues / TODO

- **Aucune projection** : on ne sait pas encore reconstruire un statut de
  message. → US-217.
- **Pas de SSE**, pas de routes REST de lecture. → US-218, US-219.
- **`POST /api/nodes` sans auth opérateur** : n'importe qui peut enregistrer
  un nœud et obtenir un JWT valide. Aucune US actuelle ne couvre l'auth
  admin/opérateur — écart consigné dans `03-ecarts-conception.md`.
- **Pas de vérification de journal chaîné** : `dengon-verify` n'est pas
  appelé, `integrity` reste toujours `'unverified'`. → hors périmètre US-216.
- **Nœud inconnu traité comme un 401 direct**, pas comme la quarantaine +
  alerte décrite par la conception (`docs/synthese/09` §3) — écart consigné.
- **Pas de déploiement** : tourne en local via `uvicorn`. Les migrations
  tournent dans le `lifespan` — US-224 pourra les sortir en étape explicite. →
  US-224 (VPS + TLS).
- 2 `DeprecationWarning` (`httpx`/`anyio`) en local ; sans effet, absents en CI.

## Pour l'oral

Le dashboard est un **témoin**, pas un maillon : on peut l'éteindre, le réseau
de messages continue exactement pareil. Le point d'entrée — `/ingest/batch` —
est passé d'un squelette **exprès trop gentil** (US-110, qui gobait n'importe
quoi pour démarrer avant que le format d'événement soit figé) à un pipeline
qui **rejette** tout ce qui ne respecte pas le contrat : schéma JSON, double
authentification (JWT court + signature Ed25519 — le JWT est un filtre
rapide, la signature est la vraie preuve cryptographique), et un
`event_id` recalculé côté serveur pour empêcher qu'un `event_id` forgé
n'empoisonne la déduplication.
