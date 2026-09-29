// dengon_console — console série du relais (US-308) : `ledger`, `restart`,
// `relay`. Voir dengon_console.c.
#pragma once

#include "esp_err.h"

/** Enregistre les commandes et démarre la REPL sur l'UART de la console. */
esp_err_t dengon_console_start(void);
