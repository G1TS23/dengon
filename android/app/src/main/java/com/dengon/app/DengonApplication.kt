package com.dengon.app

import android.app.Application
import com.dengon.app.ffi.DengonNode
import com.dengon.app.identite.IdentiteLocale

/**
 * Point d'entrée process. Détient **le** nœud dengon de l'appareil (US-302) :
 * un seul par processus, partagé par l'UI et, à l'US-306, par le service de
 * premier plan qui pilote la radio. Ouvert au premier accès.
 */
class DengonApplication : Application() {
    val noeud: DengonNode by lazy { IdentiteLocale.ouvrirNoeud(this) }
}
