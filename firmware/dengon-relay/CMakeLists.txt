# ---------------------------------------------------------------------------
# Projet ESP-IDF du relais dengon (US-114).
#
# Le nom du projet donne le nom des artefacts : build/dengon-relay.{elf,bin}.
# `firmware.yml` et la procédure de flash de la fiche de suivi s'appuient sur
# ces noms — les changer casse les deux.
#
# Cible : ESP32-WROOM-32E (Xtensa LX6, 4 Mo de flash, pas de PSRAM). Elle n'est
# PAS fixée ici mais par CONFIG_IDF_TARGET dans sdkconfig.defaults, pour que
# `idf.py set-target` reste la seule façon d'en changer.
#
# Référence : docs/synthese/08-relais-esp32.md §2-3.
# ---------------------------------------------------------------------------
cmake_minimum_required(VERSION 3.16)

include($ENV{IDF_PATH}/tools/cmake/project.cmake)

project(dengon-relay)
