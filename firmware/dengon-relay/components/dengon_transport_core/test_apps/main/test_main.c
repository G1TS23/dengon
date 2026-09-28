// ---------------------------------------------------------------------------
// Point d'entrée des tests Unity de dengon_transport_core (US-220).
//
// Sur la cible `linux`, le processus doit SORTIR avec un code non nul en cas
// d'échec : c'est ce que lit la CI. Sur carte, on imprime le bilan et on rend
// la main — le résultat se lit dans `idf.py monitor`.
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
    run_conformite();
    run_specifique();
    run_adv();
    echecs = UNITY_END();

#if CONFIG_IDF_TARGET_LINUX
    exit(echecs == 0 ? EXIT_SUCCESS : EXIT_FAILURE);
#else
    (void)echecs;
#endif
}
