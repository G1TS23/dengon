# ---------------------------------------------------------------------------
# Application de test Unity du composant `dengon_ship_core` (US-309).
#
# Même schéma que dengon_transport_core/test_apps : le même code pour la cible
# `linux` (CI, sans carte) et pour `esp32`.
# ---------------------------------------------------------------------------
cmake_minimum_required(VERSION 3.16)

set(EXTRA_COMPONENT_DIRS "${CMAKE_CURRENT_LIST_DIR}/..")
set(COMPONENTS main)

include($ENV{IDF_PATH}/tools/cmake/project.cmake)

project(test_dengon_ship_core)
