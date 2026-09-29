package com.dengon.app.ui.conversations

import com.dengon.app.ffi.Message
import com.dengon.app.ffi.MessageStatus
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

/**
 * Libellé affiché pour chaque statut, d'après
 * `docs/synthese/07-cycle-de-vie-et-statuts.md` §1 (colonne « Statut (UI) »),
 * reformulé en langage courant (US-321) : le sens reste celui du tableau, le
 * vocabulaire est celui d'une messagerie.
 *
 * `READ` (« Lu ») est réservé v2 (décision A-10) : le libellé existe pour que
 * l'écran reste exhaustif, mais le cœur ne l'émettra pas au MVP.
 */
fun libelleStatut(statut: MessageStatus): String = when (statut) {
    MessageStatus.QUEUED -> "En attente"
    MessageStatus.IN_FLIGHT -> "Envoyé"
    MessageStatus.DELIVERED -> "Distribué"
    MessageStatus.READ -> "Lu"
    MessageStatus.EXPIRED -> "Échec"
    MessageStatus.CANCELLED -> "Annulé"
}

/**
 * Pictogramme d'un statut. Toujours affiché **avec** le texte de
 * [libelleStatut] : la couleur ni l'icône ne portent seules l'information.
 */
enum class IconeStatut { EN_COURS, ENVOYE, DISTRIBUE, ECHEC, ANNULE }

fun iconeStatut(statut: MessageStatus): IconeStatut = when (statut) {
    MessageStatus.QUEUED -> IconeStatut.EN_COURS
    MessageStatus.IN_FLIGHT -> IconeStatut.ENVOYE
    MessageStatus.DELIVERED, MessageStatus.READ -> IconeStatut.DISTRIBUE
    MessageStatus.EXPIRED -> IconeStatut.ECHEC
    MessageStatus.CANCELLED -> IconeStatut.ANNULE
}

/**
 * Un message sortant est **en échec** quand le cœur l'a abandonné (TTL
 * dépassé, `EXPIRED`) : l'UI l'affiche « Échec » et propose « Renvoyer »
 * plutôt que de le laisser croire en cours d'acheminement (US-313).
 */
val Message.enEchec: Boolean get() = outgoing && status == MessageStatus.EXPIRED

private val FormatHeure = DateTimeFormatter.ofPattern("HH:mm", Locale.FRANCE)
private val FormatJour = DateTimeFormatter.ofPattern("d MMM HH:mm", Locale.FRANCE)

/**
 * Horodatage lisible d'un message : « 14:32 » aujourd'hui, « Hier 14:32 »,
 * sinon « 12 sept. 14:32 ». `sentMs` est l'heure murale du cœur (epoch, ms) ;
 * `0` (inconnu) donne une chaîne vide plutôt qu'un « 1 janv. 1970 ».
 * [maintenantMs] et [zone] sont injectés pour que la fonction reste testable.
 */
fun formaterHorodatage(
    sentMs: Long,
    maintenantMs: Long = System.currentTimeMillis(),
    zone: ZoneId = ZoneId.systemDefault(),
): String {
    if (sentMs <= 0L) return ""
    val date = Instant.ofEpochMilli(sentMs).atZone(zone)
    val jour = date.toLocalDate()
    val aujourdHui = Instant.ofEpochMilli(maintenantMs).atZone(zone).toLocalDate()
    return when (jour) {
        aujourdHui -> date.format(FormatHeure)
        aujourdHui.minusDays(1) -> "Hier ${date.format(FormatHeure)}"
        else -> date.format(FormatJour)
    }
}

/** Initiale affichée dans la pastille d'une conversation (« ? » si pseudo vide). */
fun initiale(pseudo: String): String =
    pseudo.trim().firstOrNull { it.isLetterOrDigit() }?.uppercaseChar()?.toString() ?: "?"
