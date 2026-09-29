# ---------------------------------------------------------------------------
# Composant `dengon_store` — le `Store` du relais (US-308).
#
# Deux moitiés :
#   - dengon_ledger_file.c : C pur (stdio), compilé pour TOUTES les cibles,
#     y compris `linux` où les tests Unity tournent en CI sans carte ;
#   - dengon_store.c       : NVS + littlefs, cible matérielle seulement.
#
# La dépendance littlefs vient du registre de composants ESP-IDF
# (idf_component.yml, joltwallet/littlefs) : elle est téléchargée au premier
# `idf.py build` (réseau requis, puis mise en cache dans managed_components/).
# ---------------------------------------------------------------------------
set(srcs "dengon_ledger_file.c")
set(requires "")
if(NOT ${IDF_TARGET} STREQUAL "linux")
    list(APPEND srcs "dengon_store.c")
    list(APPEND requires nvs_flash esp_hw_support bootloader_support joltwallet__littlefs)
endif()

idf_component_register(
    SRCS            ${srcs}
    INCLUDE_DIRS    "include"
    REQUIRES        ${requires}
)

target_compile_options(${COMPONENT_LIB} PRIVATE -Wall -Wextra -Werror)
