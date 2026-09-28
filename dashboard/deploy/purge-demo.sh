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
docker volume rm dengon_api_db 2>/dev/null || echo "  (déjà absent)"

echo "→ Relance (reconstruit l'image si le code a changé)…"
docker compose up -d --build

echo "→ Base repartie de zéro. Vérification :"
sleep 2
docker compose ps
