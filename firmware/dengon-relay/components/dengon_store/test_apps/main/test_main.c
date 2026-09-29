// ---------------------------------------------------------------------------
// Point d'entrée des tests Unity de dengon_store (US-308).
//
// Sur `linux`, le processus sort avec un code non nul en cas d'échec (lu par
// la CI). Sur carte, on monte d'abord NVS et littlefs, puis le bilan se lit
// dans `idf.py monitor`.
// ---------------------------------------------------------------------------
#include <stdlib.h>

#include "sdkconfig.h"
#include "unity.h"

#include "tests.h"

#if !CONFIG_IDF_TARGET_LINUX
#include "dengon_store.h"
#include "nvs_flash.h"
#endif

void setUp(void) {}
void tearDown(void) {}

void
app_main(void)
{
    int echecs;

#if !CONFIG_IDF_TARGET_LINUX
    ESP_ERROR_CHECK(nvs_flash_init());
    ESP_ERROR_CHECK(dengon_store_init());
#endif

    UNITY_BEGIN();
    run_ledger_file();
#if !CONFIG_IDF_TARGET_LINUX
    run_store_cible();
#endif
    echecs = UNITY_END();

#if CONFIG_IDF_TARGET_LINUX
    exit(echecs == 0 ? EXIT_SUCCESS : EXIT_FAILURE);
#else
    (void)echecs;
#endif
}
