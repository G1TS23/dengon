# Contexte de build : la RACINE DU DÉPÔT, pas `dashboard/api/` — l'ingestion
# (US-216, pas encore mergée au moment de l'US-224 mais anticipée ici) lit
# `contracts/events/*.schema.json` par chemin filesystem relatif à la racine
# du dépôt, pas par import Python (voir docs/suivi/modules/dashboard-api.md
# §Dépendances). D'où `COPY contracts …` ci-dessous, et
# `docker build -f dashboard/api/Dockerfile .` depuis la racine (voir
# docs/suivi/modules/deploiement-vps.md).
FROM python:3.13-slim

# `--only-binary :all:` : refuse toute distribution source (sdist) pour uv
# lui-même — une sdist peut exécuter un `setup.py` arbitraire à
# l'installation (SonarCloud docker:S8541). uv publie des wheels pour toutes
# les plateformes courantes, donc rien à construire ici de toute façon.
#
# Version épinglée, la même que `.github/workflows/dashboard.yml`
# (`astral-sh/setup-uv`) — un `uv` différent en CI et en déploiement serait
# une source de divergence de résolution de dépendances silencieuse.
RUN pip install --no-cache-dir --only-binary :all: uv==0.9.25

# Utilisateur non privilégié (SonarCloud docker:S6471 — l'image `python`
# tourne root par défaut) : `uvicorn` n'a besoin d'écrire nulle part une
# fois démarré, root ne lui sert à rien. Créé tôt, activé (`USER`) tout à la
# fin, une fois que tous les `COPY`/`RUN` qui écrivent dans l'image sont
# faits.
RUN groupadd --system app && useradd --system --gid app --no-create-home app

WORKDIR /srv/dengon

# Manifeste + lock d'abord, séparément du code : le cache Docker ne
# réinstalle les dépendances que si l'un des deux change, pas à chaque
# modification de app/*.py.
COPY dashboard/api/pyproject.toml dashboard/api/uv.lock dashboard/api/
# `WORKDIR`, pas `cd` (SonarCloud docker:S6597 — un `cd` en tête de `RUN` ne
# persiste que pour cette instruction, `WORKDIR` le fait pour toutes les
# suivantes, et Sonar le préfère pour cette raison même quand une seule
# commande en dépend ici). `--no-build` : même raison que
# `--only-binary :all:` ci-dessus, côté résolution `uv` cette fois — même
# flag déjà utilisé par `.github/workflows/dashboard.yml`.
WORKDIR /srv/dengon/dashboard/api
RUN uv sync --frozen --no-install-project --no-build
# --no-install-project : même raison qu'en CI (dashboard.yml) — les imports
# se font via PYTHONPATH (WORKDIR ci-dessous), pas via une installation du
# paquet `app`.

WORKDIR /srv/dengon
COPY contracts ./contracts
COPY dashboard/api/app dashboard/api/app

WORKDIR /srv/dengon/dashboard/api
ENV PATH="/srv/dengon/dashboard/api/.venv/bin:${PATH}"

RUN chown -R app:app /srv/dengon
# `/data` : point de montage du volume SQLite (voir docker-compose.yml).
# Créé et donné à `app` ICI, dans l'image — un volume nommé, à sa toute
# première création, hérite du contenu/des permissions du chemin correspondant
# dans l'image plutôt que d'appartenir à root par défaut. Sans ça, `app`
# (non-root, voir ci-dessus) ne pourrait pas écrire le fichier SQLite au
# démarrage (`sqlite3.OperationalError: unable to open database file`,
# constaté en testant l'image après être passé en non-root).
RUN mkdir -p /data && chown app:app /data
USER app

EXPOSE 8000
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8000"]
