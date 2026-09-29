#pragma once

/* Répertoire des fichiers de test : le dossier courant sur l'hôte, la
   partition littlefs sur carte. */
#if CONFIG_IDF_TARGET_LINUX
#define TEST_DIR "."
#else
#define TEST_DIR "/lfs"
#endif

void run_ledger_file(void);
void run_store_cible(void);
