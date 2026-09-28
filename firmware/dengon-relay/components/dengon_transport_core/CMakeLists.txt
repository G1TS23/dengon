# ---------------------------------------------------------------------------
# Composant `dengon_transport_core` — la moitié PURE du transport (US-220).
#
# Aucune dépendance ESP-IDF (pas de REQUIRES) : il ne dépend que de la libc.
# C'est ce qui lui permet de compiler pour la cible `linux` et d'être testé sur
# l'hôte, en CI, sans carte (voir test_apps/). La glue NimBLE est dans `main`.
#
# -Werror posé ici comme sur `main` : c'est NOTRE code, pas celui d'ESP-IDF.
# ---------------------------------------------------------------------------
idf_component_register(
    SRCS            "dengon_transport_core.c"
                    "dengon_adv.c"
    INCLUDE_DIRS    "include"
)

target_compile_options(${COMPONENT_LIB} PRIVATE -Wall -Wextra -Werror)
