package com.dengon.app.ui.reseau

import com.dengon.app.ble.transport.LinkId

/** Préfixe de pseudo des relais ESP32 (`node_id` `relay-…`, docs/synthese/09). */
const val PREFIXE_RELAIS = "relay-"

/** Un pair relié en ce moment, tel que l'écran réseau l'affiche. */
data class PairVu(val peerId: String, val pseudo: String?, val estRelais: Boolean)

/** Tout ce que l'écran réseau affiche (US-313). */
data class VueReseau(
    val serviceDemarre: Boolean,
    val modeEco: Boolean,
    /** Téléphones et autres pairs, hors relais. */
    val pairs: List<PairVu>,
    /** Relais atteints. */
    val relais: List<PairVu>,
)

/**
 * Construit la vue à partir de l'état du transport. Un pair est un **relais**
 * si son pseudo annoncé commence par [PREFIXE_RELAIS] : l'`ANNOUNCE` ne porte
 * pas de capacité « relais » exploitable côté téléphone.
 */
fun vueReseau(
    serviceDemarre: Boolean,
    modeEco: Boolean,
    liens: Map<LinkId, String>,
    pseudos: Map<String, String>,
): VueReseau {
    val vus = liens.values.distinct().sorted().map { peerId ->
        val pseudo = pseudos[peerId]
        PairVu(peerId, pseudo, estRelais = pseudo?.startsWith(PREFIXE_RELAIS) == true)
    }
    val (relais, pairs) = vus.partition { it.estRelais }
    return VueReseau(serviceDemarre, modeEco, pairs, relais)
}
