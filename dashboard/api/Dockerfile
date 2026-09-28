# Contexte de build : la RACINE DU DÉPÔT, pas `dashboard/api/` — l'ingestion
# (US-216, pas encore mergée au moment de l'US-224 mais anticipée ici) lit
# `contracts/events/*.schema.json` par chemin filesystem relatif à la racine
# du dépôt, pas par import Python (voir docs/suivi/modules/dashboard-api.md
# §Dépendances). D'où `COPY contracts …` ci-dessous, et
# `docker build -f dashboard/api/Dockerfile .` depuis la racine (voir
# docs/suivi/modules/deploiement-vps.md).
FROM python:3.13-slim

# Version épinglée, la même que `.github/workflows/dashboard.yml`
# (`astral-sh/setup-uv`) — un `uv` différent en CI et en déploiement serait
# une source de divergence de résolution de dépendances silencieuse.
RUN pip install --no-cache-dir uv==0.9.25

WORKDIR /srv/dengon

# Manifeste + lock d'abord, séparément du code : le cache Docker ne
# réinstalle les dépendances que si l'un des deux change, pas à chaque
# modification de app/*.py.
COPY dashboard/api/pyproject.toml dashboard/api/uv.lock dashboard/api/
RUN cd dashboard/api && uv sync --frozen --no-install-project
# --no-install-project : même raison qu'en CI (dashboard.yml) — les imports
# se font via PYTHONPATH (WORKDIR ci-dessous), pas via une installation du
# paquet `app`.

COPY contracts ./contracts
COPY dashboard/api/app dashboard/api/app

WORKDIR /srv/dengon/dashboard/api
ENV PATH="/srv/dengon/dashboard/api/.venv/bin:${PATH}"

EXPOSE 8000
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8000"]
