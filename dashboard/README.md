# dashboard

Tableau de bord d'**observabilité** du réseau dengon : il trace le parcours et
l'état de chaque message **sans jamais voir son contenu**. S'il disparaît, la
messagerie fonctionne à l'identique.

Réf. conception : [`../docs/synthese/09-dashboard-et-donnees.md`](../docs/synthese/09-dashboard-et-donnees.md).

| Dossier | Contenu | État |
| --- | --- | --- |
| [`api/`](api/) | FastAPI — ingestion HTTPS, REST, SSE, reconstruction des projections | **squelette** (US-110) : `/healthz` + `/ingest/batch` permissif |
| `web/` | page web légère, rafraîchie par SSE | à venir (US-111) |

Le binaire de vérification de journal chaîné (`dengon-verify`, Rust) est appelé
en sous-processus par `api/` — pas encore branché (US-305 / US-310).

## Lancer l'API en local

```bash
cd api
python -m venv .venv && . .venv/bin/activate
pip install -e '.[dev]'

uvicorn app.main:app --reload         # http://127.0.0.1:8000
curl -s localhost:8000/healthz                                  # {"status":"ok"}
curl -s -XPOST localhost:8000/ingest/batch -d '{"events":[{"x":1}]}'
```

La base SQLite est créée et migrée au démarrage. Chemin réglable par
`DENGON_DASHBOARD_DB` (défaut : `api/dashboard.db`, ignoré par git).

## Tests

```bash
cd api
pip install -e '.[dev]'
ruff check .
pytest
```

`pytest` utilise une base SQLite jetable par test (aucun état partagé).
