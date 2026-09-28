#!/usr/bin/env bash
# Construit `libdengon_core.a` pour la cible native, génère les vecteurs,
# compile et exécute `host_test.c` (US-307). Utilisé par le job CI
# `cross-vectors` et reproductible en local (Linux/MSYS2 avec gcc) :
#
#   cd crates/dengon-core-embed && ./tests/c/run.sh
#
# Nécessite la toolchain nightly `esp` (ou nightly amont + rust-src, voir
# rust-toolchain.toml) et le composant `rust-src` (`rustup component add
# rust-src --toolchain esp`).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

python3 tests/c/generate_vectors.py

cargo build

cc -std=c99 -Wall -Wextra -Werror \
    -o tests/c/host_test \
    tests/c/host_test.c \
    -Ltarget/debug -ldengon_core

./tests/c/host_test
