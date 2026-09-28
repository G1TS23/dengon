#!/usr/bin/env bash
# Purge la base de démo dengon-dashboard et relance les services propres
# (US-224 : « repartir d'une base propre en une commande »).
#
# Ne touche PAS aux volumes Caddy (`dengon_caddy_data`/`dengon_caddy_config`) :
# les supprimer régénérerait le certificat auto-signé de la CA interne à
# chaque purge, ce qui obligerait à revalider le certificat dans le
# navigateur à chaque session de démo — coût sans bénéfice, la base
# applicative est ce qui doit repartir de zéro, pas le TLS.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

echo "→ Arrêt des services…"
docker compose down

echo "→ Suppression du volume de base (dengon_api_db)…"
# `docker volume inspect` distingue explicitement « volume absent » de toute
# autre erreur — corrigé après revue de la PR #97 (constat OswinFreyr) :
# `docker volume rm ... 2>/dev/null || echo "(déjà absent)"` avalait AUSSI
# une erreur « volume encore utilisé » (ex. conteneur résiduel après un
# `docker compose down` incomplet), et le script enchaînait quand même sur
# `up -d --build`, qui réutilisait alors silencieusement l'ancien volume —
# ratant le but même du script (« repartir d'une base propre »).
if docker volume inspect dengon_api_db >/dev/null 2>&1; then
  docker volume rm dengon_api_db
else
  echo "  (déjà absent)"
fi

echo "→ Relance (reconstruit l'image si le code a changé)…"
docker compose up -d --build

echo "→ Base repartie de zéro. Vérification (attente de /healthz) :"
# Poll plutôt qu'un `sleep` fixe (corrigé après revue de la PR #97, constat
# OswinFreyr) : un `sleep 2` ne vérifie rien — trop court sur une image
# reconstruite (VPS chargé par les conteneurs des autres groupes), trop long
# sinon. `docker compose exec` interroge l'API depuis l'intérieur du réseau
# Compose, sans dépendre du port Caddy publié ni du TLS auto-signé.
tentatives=0
until docker compose exec -T api python3 -c "import urllib.request as u; u.urlopen('http://127.0.0.1:8000/healthz', timeout=2)" >/dev/null 2>&1; do
  tentatives=$((tentatives + 1))
  if [ "$tentatives" -ge 15 ]; then
    echo "  /healthz ne répond toujours pas après ${tentatives} tentatives." >&2
    docker compose ps
    exit 1
  fi
  sleep 1
done
echo "  /healthz répond."
docker compose ps
