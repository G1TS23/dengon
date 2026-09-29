// ---------------------------------------------------------------------------
// Point d'entrée des tests Unity de dengon_ship_core (US-309).
//
// Sur `linux`, le processus sort avec un code non nul en cas d'échec (lu par
// la CI). Sur carte, le bilan se lit dans `idf.py monitor`.
// ---------------------------------------------------------------------------
#include <stdlib.h>

#include "sdkconfig.h"
#include "unity.h"

#include "tests.h"

void setUp(void) {}
void tearDown(void) {}

void
app_main(void)
{
    int echecs;

    UNITY_BEGIN();
    run_ring();
    run_policy();
    echecs = UNITY_END();

#if CONFIG_IDF_TARGET_LINUX
    exit(echecs == 0 ? EXIT_SUCCESS : EXIT_FAILURE);
#else
    (void)echecs;
#endif
}
