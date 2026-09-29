set(srcs "test_main.c" "test_ledger_file.c")
set(requires unity dengon_store)
if(NOT ${IDF_TARGET} STREQUAL "linux")
    list(APPEND srcs "test_store_cible.c")
    list(APPEND requires nvs_flash)
endif()

idf_component_register(
    SRCS            ${srcs}
    INCLUDE_DIRS    "."
    REQUIRES        ${requires}
)

target_compile_options(${COMPONENT_LIB} PRIVATE -Wall -Wextra -Werror)
