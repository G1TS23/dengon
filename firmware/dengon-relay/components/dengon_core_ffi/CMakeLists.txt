# ---------------------------------------------------------------------------
# Composant `dengon_core_ffi` — lie libdengon_core.a (Rust, dengon-core) au
# firmware (US-308).
#
# Rien n'est compilé ici : c'est un composant d'interface qui expose
#   - le header versionné crates/dengon-core-ffi/include/dengon_core.h
#     (généré par cbindgen, US-307) ;
#   - l'archive libdengon_core.a cross-compilée pour xtensa par
#     crates/dengon-core-embed (toolchain `esp` installée par espup).
#
# L'archive n'est PAS versionnée (artefact de build, 1,5 Mo) : la produire
# d'abord avec `tools/build_core.sh` (ou l'étape dédiée du job CI firmware).
# Chemin surchargeable : `idf.py -D DENGON_CORE_LIB=/chemin/libdengon_core.a build`.
# ---------------------------------------------------------------------------
get_filename_component(DENGON_REPO "${CMAKE_CURRENT_LIST_DIR}/../../../.." ABSOLUTE)

set(DENGON_CORE_LIB
    "${DENGON_REPO}/crates/dengon-core-embed/target/xtensa-esp32-none-elf/release/libdengon_core.a"
    CACHE FILEPATH "Archive libdengon_core.a pour xtensa-esp32")

if(NOT EXISTS "${DENGON_CORE_LIB}")
    message(FATAL_ERROR
        "libdengon_core.a introuvable : ${DENGON_CORE_LIB}\n"
        "La produire d'abord : firmware/dengon-relay/tools/build_core.sh "
        "(toolchain `esp` via espup, voir le README du firmware).")
endif()

idf_component_register(
    INCLUDE_DIRS    "${DENGON_REPO}/crates/dengon-core-ffi/include"
)

# `malloc`/`free`/`abort` que l'archive importe viennent de newlib.
add_prebuilt_library(dengon_core_rust "${DENGON_CORE_LIB}" PRIV_REQUIRES newlib)
target_link_libraries(${COMPONENT_LIB} INTERFACE dengon_core_rust)
