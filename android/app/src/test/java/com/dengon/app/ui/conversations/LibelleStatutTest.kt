package com.dengon.app.ui.conversations

import com.dengon.app.ffi.MessageStatus
import java.time.ZoneId
import java.time.ZonedDateTime
import org.junit.Assert.assertEquals
import org.junit.Test

/** Aides d'affichage de la refonte US-321 : horodatage, initiale, icône de statut. */
class LibelleStatutTest {

    private val zone = ZoneId.of("Europe/Paris")

    private fun ms(annee: Int, mois: Int, jour: Int, h: Int, min: Int): Long =
        ZonedDateTime.of(annee, mois, jour, h, min, 0, 0, zone).toInstant().toEpochMilli()

    private val maintenant = ms(2026, 9, 29, 18, 0)

    @Test
    fun `aujourd'hui l'horodatage est l'heure seule`() {
        assertEquals("14:32", formaterHorodatage(ms(2026, 9, 29, 14, 32), maintenant, zone))
    }

    @Test
    fun `hier est indique en toutes lettres`() {
        assertEquals("Hier 23:59", formaterHorodatage(ms(2026, 9, 28, 23, 59), maintenant, zone))
    }

    @Test
    fun `plus ancien donne le jour et l'heure`() {
        val texte = formaterHorodatage(ms(2026, 9, 12, 9, 5), maintenant, zone)
        assertEquals(true, texte.startsWith("12 ") && texte.endsWith(" 09:05"))
    }

    @Test
    fun `heure inconnue donne une chaine vide`() {
        assertEquals("", formaterHorodatage(0L, maintenant, zone))
    }

    @Test
    fun `l'initiale saute les espaces et les symboles`() {
        assertEquals("A", initiale("alice"))
        assertEquals("B", initiale("  @bob"))
        assertEquals("?", initiale(""))
        assertEquals("?", initiale("   "))
    }

    @Test
    fun `chaque statut a une icone, et l'echec est distinct`() {
        assertEquals(IconeStatut.EN_COURS, iconeStatut(MessageStatus.QUEUED))
        assertEquals(IconeStatut.ENVOYE, iconeStatut(MessageStatus.IN_FLIGHT))
        assertEquals(IconeStatut.DISTRIBUE, iconeStatut(MessageStatus.DELIVERED))
        assertEquals(IconeStatut.ECHEC, iconeStatut(MessageStatus.EXPIRED))
        assertEquals(IconeStatut.ANNULE, iconeStatut(MessageStatus.CANCELLED))
        MessageStatus.values().forEach { iconeStatut(it) } // exhaustif : pas d'exception
    }
}
