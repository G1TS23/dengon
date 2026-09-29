package com.dengon.app.ui.conversations

import com.dengon.app.ffi.Message
import com.dengon.app.ffi.MessageStatus

/**
 * Libellé affiché pour chaque statut, d'après
 * `docs/synthese/07-cycle-de-vie-et-statuts.md` §1 (colonne « Statut (UI) »).
 *
 * `READ` (« Lu ») est réservé v2 (décision A-10) : le libellé existe pour que
 * l'écran reste exhaustif, mais le cœur ne l'émettra pas au MVP.
 */
fun libelleStatut(statut: MessageStatus): String = when (statut) {
    MessageStatus.QUEUED -> "En attente"
    MessageStatus.IN_FLIGHT -> "Parti"
    MessageStatus.DELIVERED -> "Distribué"
    MessageStatus.READ -> "Lu"
    MessageStatus.EXPIRED -> "Échec"
    MessageStatus.CANCELLED -> "Annulé"
}

/**
 * Un message sortant est **en échec** quand le cœur l'a abandonné (TTL
 * dépassé, `EXPIRED`) : l'UI l'affiche « Échec » et propose « Renvoyer »
 * plutôt que de le laisser croire en cours d'acheminement (US-313).
 */
val Message.enEchec: Boolean get() = outgoing && status == MessageStatus.EXPIRED
