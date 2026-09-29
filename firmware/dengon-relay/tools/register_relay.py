#!/usr/bin/env python3
"""Enregistre un relais auprès du dashboard et affiche son jeton (US-309).

Le relais n'a pas de moyen de s'enregistrer seul : l'opérateur lit son
identité sur la console série (`dash id`), la poste ici sur
`POST /api/nodes`, puis recopie le jeton rendu dans la console
(`dash token <jwt>`) :

    python3 tools/register_relay.py --node-id relay-3f2a9c --pub-sign <64 hex>

Le serveur est vérifié avec la même racine que celle embarquée dans le
firmware (`main/certs/dashboard_root.pem`, voir `main/certs/README.md`).

Limites du dashboard (US-216), pas de ce script :
- le jeton expire au bout de 24 h et ne se renouvelle pas ;
- un `node_id` déjà connu est refusé (409) : pour réémettre un jeton, il faut
  repartir d'une base vide (`dashboard/deploy/purge-demo.sh`, prévu après
  chaque démo) puis réenregistrer le relais.
Aucun jeton n'est écrit sur disque par ce script.
"""

import argparse
import json
import re
import ssl
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

RACINE_DEFAUT = (
    Path(__file__).resolve().parent.parent / "main" / "certs" / "dashboard_root.pem"
)
NODE_ID = re.compile(r"^relay-[0-9a-f]{6}$")
PUB_SIGN = re.compile(r"^[0-9a-f]{64}$")
# Seuls hôtes joignables : le dashboard de démo, ou un dashboard local de dev
# (le script ne doit pas pouvoir servir à poster ailleurs).
# Adresse publique du dashboard de démo (US-224), pas un secret ; la liste
# d'hôtes autorisés est voulue (revue PR #120, Sonar python:S1313).
HOTE_VPS = "51.255.38.214"  # NOSONAR
URL_DEFAUT = f"https://{HOTE_VPS}:8443"
HOTES_LOCAUX = ("127.0.0.1", "localhost")


def url_api(url):
    """URL de `POST /api/nodes`, reconstruite à partir d'éléments vérifiés.

    `https` vers le VPS de démo, ou `http`/`https` vers la boucle locale ;
    tout le reste est refusé (ValueError).
    """
    u = urllib.parse.urlsplit(url)
    hotes = (HOTE_VPS, *HOTES_LOCAUX)
    hote = next((h for h in hotes if h == u.hostname), None)
    if hote is None:
        raise ValueError(
            f"hôte non autorisé : {u.hostname!r} (attendu : {', '.join(hotes)})"
        )
    schema = next((s for s in ("https", "http") if s == u.scheme), None)
    if schema is None or (schema == "http" and hote not in HOTES_LOCAUX):
        raise ValueError("https obligatoire (http seulement vers la boucle locale)")
    port = int(u.port or (443 if schema == "https" else 80))
    return f"{schema}://{hote}:{port}/api/nodes", schema == "https"


def enregistrer(url, node_id, pub_sign, cafile):
    """POST /api/nodes ; rend (statut, corps JSON décodé ou texte brut)."""
    cible, tls = url_api(url)
    ctx = None
    if tls:
        ctx = ssl.create_default_context(cafile=str(cafile))
        ctx.minimum_version = ssl.TLSVersion.TLSv1_2
        # Comme mbedTLS côté firmware : le certificat fourni sert d'ancre même
        # s'il n'est pas auto-signé (intermédiaire Caddy épinglé pour un essai).
        ctx.verify_flags |= ssl.VERIFY_X509_PARTIAL_CHAIN
    corps = json.dumps(
        {"node_id": node_id, "kind": "relay", "pub_sign": pub_sign}
    ).encode()
    req = urllib.request.Request(
        cible,
        data=corps,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, context=ctx, timeout=15) as rep:
            return rep.status, json.loads(rep.read())
    except urllib.error.HTTPError as err:
        texte = err.read().decode(errors="replace")
        try:
            return err.code, json.loads(texte)
        except ValueError:
            return err.code, texte


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument(
        "--node-id", required=True, help="affiché par `dash id` (relay-xxxxxx)"
    )
    p.add_argument(
        "--pub-sign", required=True, help="clé publique Ed25519, 64 hex (`dash id`)"
    )
    p.add_argument(
        "--url", default=URL_DEFAUT, help=f"URL du dashboard (défaut : {URL_DEFAUT})"
    )
    p.add_argument(
        "--cacert",
        type=Path,
        default=RACINE_DEFAUT,
        help="racine de l'autorité du dashboard (défaut : main/certs/dashboard_root.pem)",
    )
    args = p.parse_args(argv)

    pub_sign = args.pub_sign.lower()
    if not NODE_ID.match(args.node_id):
        p.error("--node-id doit être de la forme relay-xxxxxx (6 hex)")
    if not PUB_SIGN.match(pub_sign):
        p.error("--pub-sign doit faire 64 caractères hexadécimaux")
    try:
        url_api(args.url)
    except ValueError as err:
        p.error(str(err))
    if not args.cacert.is_file():
        p.error(f"racine introuvable : {args.cacert} (voir main/certs/README.md)")

    try:
        statut, corps = enregistrer(args.url, args.node_id, pub_sign, args.cacert)
    except ssl.SSLError as err:
        print(
            f"TLS : {err} — racine {args.cacert} invalide ou pas celle du serveur",
            file=sys.stderr,
        )
        return 1
    except urllib.error.URLError as err:
        print(f"dashboard injoignable ({args.url}) : {err.reason}", file=sys.stderr)
        return 1
    if statut == 201 and isinstance(corps, dict) and "token" in corps:
        print(
            f"relais {args.node_id} enregistré. À coller dans la console du relais :\n"
        )
        print(f"dash token {corps['token']}")
        return 0
    if statut == 409:
        print(
            f"{args.node_id} est déjà enregistré (409) : le dashboard ne réémet pas de jeton.\n"
            "Repartir d'une base vide (dashboard/deploy/purge-demo.sh) puis relancer ce script.",
            file=sys.stderr,
        )
        return 1
    print(f"échec ({statut}) : {corps}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
