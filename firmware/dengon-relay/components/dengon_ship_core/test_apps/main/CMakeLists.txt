idf_component_register(
    SRCS            "test_main.c" "test_ring.c" "test_policy.c"
    INCLUDE_DIRS    "."
    REQUIRES        unity dengon_ship_core
)

target_compile_options(${COMPONENT_LIB} PRIVATE -Wall -Wextra -Werror)
