#!/usr/bin/env sh
# ---------------------------------------------------------------------------
# Produit libdengon_core.a pour le firmware (US-308) : cross-compilation de
# crates/dengon-core-embed vers xtensa-esp32-none-elf.
#
# Prérequis (une fois) : `espup install --targets esp32` (toolchain Rust `esp`,
# ~2 Go), puis `. ~/export-esp.sh` dans le shell courant — ce script le
# source lui-même s'il le trouve.
#
# Sortie : crates/dengon-core-embed/target/xtensa-esp32-none-elf/release/
#          libdengon_core.a — là où components/dengon_core_ffi la cherche.
# ---------------------------------------------------------------------------
set -eu

racine=$(cd "$(dirname "$0")/../../.." && pwd)
if [ -f "$HOME/export-esp.sh" ]; then
    # shellcheck disable=SC1091
    . "$HOME/export-esp.sh"
fi

cd "$racine/crates/dengon-core-embed"
cargo +esp build --release --target xtensa-esp32-none-elf
ls -l target/xtensa-esp32-none-elf/release/libdengon_core.a
