# ---------------------------------------------------------------------------
# Composant `main` — tout le code applicatif du squelette US-114.
#
# -Werror est posé UNIQUEMENT sur notre composant, jamais globalement : un
# `idf_build_set_property(COMPILE_OPTIONS -Werror APPEND)` casserait la
# compilation des composants ESP-IDF eux-mêmes, qui portent leurs propres
# avertissements assumés. Ainsi, « idf.py build propre, sans warning bloquant »
# (critère d'acceptation n°1 de l'US) devient une garantie vérifiée par la
# machine sur NOTRE code, et seulement sur lui.
# ---------------------------------------------------------------------------
idf_component_register(
    SRCS            "main.c"
                    "dengon_gatt.c"
                    "dengon_peer_id.c"
    INCLUDE_DIRS    "."
    # bt             : NimBLE (host BLE + contrôleur) — docs/synthese/08 §3
    # nvs_flash      : obligatoire AVANT d'allumer le contrôleur BT (calibration PHY)
    # esp_hw_support : esp_mac.h -> esp_efuse_mac_get_default() (bouchon peerID)
    # mbedtls        : SHA-256 du bouchon peerID
    REQUIRES        bt nvs_flash esp_hw_support mbedtls
)

target_compile_options(${COMPONENT_LIB} PRIVATE -Wall -Wextra -Werror)
