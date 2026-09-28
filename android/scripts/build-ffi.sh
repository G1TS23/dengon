#!/usr/bin/env bash
# Construit le pont Rust ↔ Kotlin de l'application (US-302).
#
#   bindings : régénère android/app/src/main/java/com/dengon/app/ffi/dengon.kt
#              depuis crates/dengon-ffi/src/dengon.udl. Ce fichier est
#              VERSIONNÉ (Android Studio sous Windows compile sans Rust) ;
#              la CI vérifie qu'il n'a pas dérivé du `.udl`.
#   android  : libdengon_ffi.so pour arm64-v8a (téléphones) et x86_64
#              (émulateur), déposées dans app/src/main/jniLibs/ (non versionné).
#              Demande cargo-ndk et un NDK (ANDROID_NDK_HOME, ou
#              ANDROID_HOME/ndk/<version>).
#   hote     : libdengon_ffi.so pour la machine courante (target/debug/),
#              chargée par JNA dans les tests JVM (testDebugUnitTest).
#
# Sans argument : les trois, dans cet ordre. Depuis WSL ou Linux (la CI) ;
# pas depuis Windows.
#
# Usage : android/scripts/build-ffi.sh [bindings] [android] [hote]
set -euo pipefail

RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$RACINE"

ABIS=(arm64-v8a x86_64)

bindings() {
    echo "==> bindings Kotlin"
    # --no-format : sortie identique d'une machine à l'autre (pas de ktlint
    # requis), condition pour que la CI puisse comparer au fichier versionné.
    cargo run --quiet -p dengon-ffi --features bindgen --bin uniffi-bindgen -- \
        generate crates/dengon-ffi/src/dengon.udl \
        --language kotlin \
        --config crates/dengon-ffi/uniffi.toml \
        --out-dir android/app/src/main/java \
        --no-format
}

android() {
    echo "==> libdengon_ffi.so : ${ABIS[*]}"
    local cibles=()
    for abi in "${ABIS[@]}"; do cibles+=(-t "$abi"); done
    cargo ndk "${cibles[@]}" -o android/app/src/main/jniLibs \
        build -p dengon-ffi --release
}

hote() {
    echo "==> libdengon_ffi.so : hôte (tests JVM)"
    cargo build -p dengon-ffi
}

etapes=("$@")
[[ ${#etapes[@]} -eq 0 ]] && etapes=(bindings android hote)
for etape in "${etapes[@]}"; do
    case "$etape" in
        bindings | android | hote) "$etape" ;;
        *) echo "étape inconnue : $etape (bindings | android | hote)" >&2; exit 2 ;;
    esac
done
