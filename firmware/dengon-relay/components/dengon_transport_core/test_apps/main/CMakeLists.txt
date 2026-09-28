idf_component_register(
    SRCS            "test_main.c"
                    "test_conformite.c"
                    "test_specifique.c"
                    "test_adv.c"
                    "banc.c"
    INCLUDE_DIRS    "."
    REQUIRES        unity dengon_transport_core
)

target_compile_options(${COMPONENT_LIB} PRIVATE -Wall -Wextra -Werror)
