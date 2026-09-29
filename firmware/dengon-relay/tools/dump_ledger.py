#!/usr/bin/env python3
"""Extrait le journal chaîné d'une capture de la console série du relais (US-308).

La commande console `ledger` imprime `ledger.old` puis `ledger.bin` en
hexadécimal entre `DENGON-LEDGER-BEGIN` et `DENGON-LEDGER-END`. Ce script
relit une capture (`idf.py monitor | tee capture.log`, ou un copier-coller),
reconstitue les octets et les écrit dans un fichier que `dengon-verify` lit
tel quel :

    python3 tools/dump_ledger.py capture.log -o ledger.bin
    cargo run -p dengon-verify -- --pubkey <clé imprimée au boot> ledger.bin

Si la capture contient plusieurs exports, le DERNIER est retenu.
"""

import argparse
import re
import sys

DEBUT = "DENGON-LEDGER-BEGIN"
FIN = "DENGON-LEDGER-END"
# Préfixes de log ESP-IDF et séquences de couleur ANSI éventuels.
ANSI = re.compile(r"\x1b\[[0-9;]*m")
HEX = re.compile(r"^[0-9a-f]+$")


def extraire(lignes):
    """Octets du dernier export complet, ou None."""
    dernier = None
    courant = None
    for brute in lignes:
        ligne = ANSI.sub("", brute).strip()
        if ligne.endswith(DEBUT):
            courant = bytearray()
        elif ligne.endswith(FIN):
            if courant is not None:
                dernier = bytes(courant)
            courant = None
        elif courant is not None and ligne:
            if not HEX.match(ligne) or len(ligne) % 2:
                raise ValueError(f"ligne d'export illisible : {ligne!r}")
            courant.extend(bytes.fromhex(ligne))
    return dernier


def main():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("capture", help="capture de la console série (- pour stdin)")
    p.add_argument("-o", "--sortie", required=True, help="fichier .bin à écrire")
    args = p.parse_args()

    source = sys.stdin if args.capture == "-" else open(args.capture, encoding="utf-8", errors="replace")
    with source:
        octets = extraire(source)
    if octets is None:
        sys.exit(f"aucun export {DEBUT}…{FIN} complet dans {args.capture}")
    with open(args.sortie, "wb") as f:
        f.write(octets)
    print(f"{len(octets)} octets écrits dans {args.sortie}")


if __name__ == "__main__":
    main()
