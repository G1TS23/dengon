# ---------------------------------------------------------------------------
# Composant `main` — application du relais : glue NimBLE du transport
# (US-220), table GATT et annonce (US-114), démo.
#
# La sémantique du transport (LinkId, file d'événements, erreurs) n'est PAS
# ici mais dans components/dengon_transport_core, composant pur testé sur
# l'hôte. `main` n'en est que la traduction vers NimBLE.
#
# -Werror est posé UNIQUEMENT sur nos composants, jamais globalement : un
# `idf_build_set_property(COMPILE_OPTIONS -Werror APPEND)` casserait la
# compilation des composants ESP-IDF eux-mêmes, qui portent leurs propres
# avertissements assumés.
# ---------------------------------------------------------------------------
idf_component_register(
    SRCS            "main.c"
                    "transport_nimble.c"
                    "dengon_gatt.c"
                    "dengon_peer_id.c"
                    "dengon_demo.c"
    INCLUDE_DIRS    "."
    # bt                    : NimBLE (host BLE + contrôleur) — docs/synthese/08 §3
    # nvs_flash             : obligatoire AVANT d'allumer le contrôleur BT (calibration PHY)
    # esp_hw_support        : esp_mac.h -> esp_efuse_mac_get_default() (bouchon peerID)
    # mbedtls               : SHA-256 du bouchon peerID
    # dengon_transport_core : sémantique du contrat Transport (C pur)
    # esp_driver_gpio       : bouton BOOT de la démo
    # esp_rom               : CRC32 de la démo
    REQUIRES        bt nvs_flash esp_hw_support mbedtls dengon_transport_core
                    esp_driver_gpio esp_rom
)

target_compile_options(${COMPONENT_LIB} PRIVATE -Wall -Wextra -Werror)
