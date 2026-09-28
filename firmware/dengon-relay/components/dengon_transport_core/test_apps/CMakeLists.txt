# ---------------------------------------------------------------------------
# Application de test Unity du composant `dengon_transport_core` (US-220).
#
# Deux cibles, le MÊME code de test :
#   - `linux`  : binaire hôte, lancé en CI (workflow firmware) — aucune carte ;
#   - `esp32`  : flashé sur une carte, « tests Unity sur cible » de l'US.
#
# EXTRA_COMPONENT_DIRS pointe sur le composant lui-même (le dossier parent).
# COMPONENTS réduit le build au strict nécessaire : sans cette ligne, la cible
# esp32 compilerait tous les composants d'ESP-IDF, Bluetooth compris.
# ---------------------------------------------------------------------------
cmake_minimum_required(VERSION 3.16)

set(EXTRA_COMPONENT_DIRS "${CMAKE_CURRENT_LIST_DIR}/..")
set(COMPONENTS main)

include($ENV{IDF_PATH}/tools/cmake/project.cmake)

project(test_dengon_transport_core)
