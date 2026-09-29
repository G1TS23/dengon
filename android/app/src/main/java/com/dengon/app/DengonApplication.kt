package com.dengon.app

import android.app.Application
import android.util.Log
import com.dengon.app.ffi.DengonNode
import com.dengon.app.identite.IdentiteLocale
import kotlin.concurrent.thread

/**
 * Point d'entrée process. Détient **le** nœud dengon de l'appareil (US-302) :
 * un seul par processus, partagé par l'UI et par le service de premier plan
 * qui pilote la radio (`TransportActif` → `Maillage`, US-306).
 *
 * L'ouverture (Keystore, fichiers, identité tirée au premier lancement) est
 * lancée dès [onCreate] sur un thread de fond (revue PR #109, risque d'ANR) :
 * quand l'activité lit [noeud], il est en général déjà prêt ; sinon, le
 * `lazy` synchronisé la fait attendre la fin de l'ouverture en cours, sans
 * en lancer une seconde. Rendre l'accès lui-même asynchrone (état « ouverture »
 * dans l'UI) reste à faire.
 */
class DengonApplication : Application() {

    val noeud: DengonNode by lazy { IdentiteLocale.ouvrirNoeud(this) }

    override fun onCreate() {
        super.onCreate()
        thread(name = "dengon-ouverture") {
            // Un échec ici n'est pas fatal : le premier accès depuis l'UI
            // retentera l'ouverture, et c'est lui qui remontera l'erreur.
            runCatching { noeud }.onFailure { Log.w(TAG, "ouverture anticipée du nœud en échec", it) }
        }
    }

    private companion object {
        const val TAG = "DengonApplication"
    }
}
