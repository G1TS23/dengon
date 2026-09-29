# ---------------------------------------------------------------------------
# Composant `dengon_ship_core` — la moitié PURE de l'export vers le
# dashboard (US-309) : buffer ring borné et politique de réessai.
#
# Même règle que dengon_transport_core : aucune dépendance ESP-IDF (pas de
# REQUIRES), donc compilable pour la cible `linux` et testé sur l'hôte en CI
# (test_apps/). Le Wi-Fi, le client HTTPS et la tâche d'envoi sont dans
# `main` (dengon_wifi.c, dengon_ship.c).
# ---------------------------------------------------------------------------
idf_component_register(
    SRCS            "dengon_ring.c"
                    "dengon_ship_policy.c"
    INCLUDE_DIRS    "include"
)

target_compile_options(${COMPONENT_LIB} PRIVATE -Wall -Wextra -Werror)
