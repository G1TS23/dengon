# Module : `dashboard/api` (FastAPI)

**Rôle en une phrase :** recevoir les batchs d'événements que les nœuds poussent
en HTTPS, et servir plus tard le parcours + l'état de chaque message.
**Correspond à la conception :** [`docs/synthese/09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md)
§2 (architecture), §3 (ingestion), §11.2 (schéma cible).
**Dernière mise à jour :** 2026-09-09
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
  pyproject.toml         — deps (fastapi, uvicorn) + dev (pytest, httpx, ruff) + config ruff/pytest
  requirements-dev.txt   — versions épinglées installées par la CI
  app/
    config.py            — db_path() : lit DENGON_DASHBOARD_DB (défaut dashboard.db)
    migrations.py        — MIGRATIONS : liste (version, nom, sql) ; le SQL est un littéral du module
    db.py               — connect() ; run_migrations() : applique MIGRATIONS, trace dans schema_migrations
    main.py             — app FastAPI ; lifespan → migrations ; routes /healthz et /ingest/batch
  tests/
    conftest.py          — fixture `client` : base SQLite jetable par test, migrée par le lifespan
    test_api.py          — 6 tests (voir plus bas)
```

## Concepts / types importants

| Élément | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `db_path()` | `app/config.py:19` | chemin du fichier SQLite, lu depuis l'env à chaque appel (pas de cache → tests simples) |
| `MIGRATIONS` | `app/migrations.py:38` | liste `(version, nom, sql)` ; le SQL est un **littéral du module** (pas un fichier lu), donc versionné, embarqué dans le paquet, sans I/O disque |
| `connect()` | `app/db.py:19` | connexion SQLite, `row_factory = Row`, `WAL`, `foreign_keys = ON` |
| `run_migrations(conn)` | `app/db.py:38` | applique les migrations manquantes dans l'ordre de version, insère dans `schema_migrations`, renvoie les versions appliquées ; idempotent |
| `lifespan` | `app/main.py:31` | au démarrage de l'app : ouvre une connexion, migre, ferme |
| `GET /healthz` | `app/main.py:47` | renvoie `{"status": "ok"}` |
| `_guess_event_count()` | `app/main.py:52` | devine le nombre d'événements (`[...]` ou `{"events": [...]}`) sans imposer de schéma ; `None` sinon, jamais de rejet |
| `POST /ingest/batch` | `app/main.py:69` | lit le corps brut → `json.loads` (400 si invalide) → `INSERT` dans `raw_batches` → `202` avec `batch_id` |

## Flux principal (exemple)

```
un nœud : POST /ingest/batch   body = {"events":[{...},{...}]}
  main.ingest_batch
    raw = await request.body()
    json.loads(raw)            → OK (sinon 400)
    batch_id = uuid4()
    event_count = 2            (_guess_event_count)
    INSERT INTO raw_batches (batch_id, received_ms, remote_addr, content_type, event_count, body)
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
  `httpx` (client de test via `TestClient`), `pytest`, `ruff`. Base :
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
- **`errors="replace"` au moment de stocker le corps** : `json.loads` a déjà
  garanti que c'est de l'UTF-(8/16/32) valide ; le garde-fou évite juste un
  crash sur un cas exotique.

## Tests

- `tests/test_api.py` — 6 tests : `/healthz` ; objet arbitraire accepté (202,
  `event_count` = 3) ; tableau nu accepté ; corps stocké **verbatim** en base ;
  non-JSON → 400 ; migrations appliquées une seule fois (`[1]`).
- Commande : depuis `dashboard/api/`, `pip install -e '.[dev]'` puis `ruff check .`
  et `pytest` → **6 passed** (vérifié le 2026-09-09, Python 3.14 en local ;
  la CI tourne en 3.11).

## Limites connues / TODO

- **Aucune sécurité** : ni JWT, ni liste blanche, ni signature Ed25519, ni rejet
  d'un `msgID` non haché. → US-216.
- **Aucune projection** : on ne sait pas encore reconstruire un statut de
  message. → US-217.
- **Pas de SSE**, pas de routes REST de lecture. → US-218, US-219.
- **Pas de déploiement** : tourne en local via `uvicorn`. → US-224 (VPS + TLS).
- 2 `DeprecationWarning` (`httpx`/`anyio`) sous Python 3.14 en local ; sans
  effet, absents en 3.11.

## Pour l'oral

Le dashboard est un **témoin**, pas un maillon : on peut l'éteindre, le réseau
de messages continue exactement pareil. Ce squelette montre le point d'entrée —
les nœuds y déposent des lots d'événements — et il est **exprès trop gentil** :
il gobe n'importe quoi. C'est un choix d'organisation : il pouvait démarrer
avant que l'équipe ait figé le format exact des événements, ce qui a permis de
travailler les trois parties du projet en parallèle.
