package com.dengon.app.ui.reseau

import com.dengon.app.ble.transport.LinkId

/**
 * Préfixes de pseudo des relais ESP32 : `relais-xxxx` est ce que le firmware
 * annonce (`dengon_relay_app.c`), `relay-…` le `node_id` du dashboard
 * (docs/synthese/09).
 */
val PREFIXES_RELAIS = listOf("relais-", "relay-")

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
 * si son pseudo annoncé commence par l'un des [PREFIXES_RELAIS] : l'`ANNOUNCE` ne porte
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
        PairVu(peerId, pseudo, estRelais = pseudo != null && PREFIXES_RELAIS.any { pseudo.startsWith(it) })
    }
    val (relais, pairs) = vus.partition { it.estRelais }
    return VueReseau(serviceDemarre, modeEco, pairs, relais)
}
