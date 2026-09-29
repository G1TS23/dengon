package com.dengon.app.ble.transport

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.random.Random

/** Fragmentation BLE (L1) et utilitaires d'annonce, en JVM pur. */
class FragmentationBleTest {

    private fun recoller(morceaux: List<ByteArray>): ByteArray? {
        val r = Reassembleur()
        var trame: ByteArray? = null
        for (m in morceaux) {
            assertNull("trame rendue avant le dernier morceau", trame)
            trame = r.ajouter(m)
        }
        assertFalse(r.enCours)
        return trame
    }

    @Test
    fun `aller-retour pour des tailles et des charges utiles tirees au hasard`() {
        val alea = Random(213)
        repeat(500) {
            val trame = ByteArray(alea.nextInt(0, 3000)).also { alea.nextBytes(it) }
            val charge = alea.nextInt(2, GattDengon.VALEUR_MAX + 1)
            val morceaux = FragmentationBle.decouper(trame, charge)
            assertTrue(morceaux.all { it.size <= charge })
            assertArrayEquals(trame, recoller(morceaux))
        }
    }

    @Test
    fun `trame vide - un seul morceau sans donnees`() {
        val morceaux = FragmentationBle.decouper(byteArrayOf(), 20)
        assertEquals(1, morceaux.size)
        assertArrayEquals(byteArrayOf(0), morceaux[0])
        assertArrayEquals(byteArrayOf(), recoller(morceaux))
    }

    @Test
    fun `en-tete - SUITE sur tous les morceaux sauf le dernier`() {
        val morceaux = FragmentationBle.decouper(ByteArray(50), 20)
        assertEquals(3, morceaux.size)
        assertEquals(listOf(0x80, 0x80, 0x00), morceaux.map { it[0].toInt() and 0xFF })
    }

    @Test
    fun `charge utile selon le MTU`() {
        assertEquals(20, FragmentationBle.chargeUtile(23))
        assertEquals(514, FragmentationBle.chargeUtile(517))
        assertEquals(2, FragmentationBle.chargeUtile(3))
        assertThrows(IllegalArgumentException::class.java) { FragmentationBle.decouper(ByteArray(3), 1) }
    }

    @Test
    fun `reassemblage borne a TRAME_MAX puis reutilisable`() {
        val r = Reassembleur()
        val morceau = ByteArray(501).also { it[0] = FragmentationBle.SUITE.toByte() }
        val e = assertThrows(TransportException.FrameTooLarge::class.java) {
            repeat(20) { r.ajouter(morceau) }
        }
        assertEquals(FragmentationBle.TRAME_MAX, e.max)
        assertFalse(r.enCours)
        assertArrayEquals(byteArrayOf(9), r.ajouter(byteArrayOf(0, 9)))
    }

    @Test
    fun `morceau vide ou bits reserves - erreur et reassemblage abandonne`() {
        val r = Reassembleur()
        r.ajouter(byteArrayOf(0x80.toByte(), 1))
        assertTrue(r.enCours)
        assertThrows(TransportException.Backend::class.java) { r.ajouter(byteArrayOf()) }
        assertFalse(r.enCours)
        r.ajouter(byteArrayOf(0x80.toByte(), 1))
        assertThrows(TransportException.Backend::class.java) { r.ajouter(byteArrayOf(0x01, 1)) }
        assertFalse(r.enCours)
    }

    // --- Annonce et règle anti-boucle -----------------------------------------

    @Test
    fun `annonce - 4 premiers octets du peerID`() {
        val id = byteArrayOf(1, 2, 3, 4, 5, 6, 7, 8)
        assertArrayEquals(byteArrayOf(1, 2, 3, 4), Annonce.donnees(id))
        assertArrayEquals(byteArrayOf(1, 2, 3, 4), Annonce.prefixeDistant(byteArrayOf(1, 2, 3, 4, 99)))
        assertNull(Annonce.prefixeDistant(byteArrayOf(1, 2)))
        assertNull(Annonce.prefixeDistant(null))
        assertThrows(IllegalArgumentException::class.java) { Annonce.donnees(ByteArray(4)) }
    }

    @Test
    fun `regle anti-boucle - un seul des deux noeuds initie`() {
        val alea = Random(7)
        repeat(1000) {
            val a = ByteArray(8).also { alea.nextBytes(it) }
            val b = ByteArray(8).also { alea.nextBytes(it) }
            if (a.copyOf(4).contentEquals(b.copyOf(4))) return@repeat
            val aInitie = Annonce.doitInitier(a, Annonce.donnees(b))
            val bInitie = Annonce.doitInitier(b, Annonce.donnees(a))
            assertTrue("exactement un initiateur", aInitie != bInitie)
        }
    }

    @Test
    fun `regle anti-boucle - octets compares sans signe`() {
        val petit = byteArrayOf(0x7F, 0, 0, 0, 0, 0, 0, 0)
        val grand = byteArrayOf(0x80.toByte(), 0, 0, 0, 0, 0, 0, 0) // négatif en Kotlin
        assertTrue(Annonce.doitInitier(petit, Annonce.donnees(grand)))
        assertFalse(Annonce.doitInitier(grand, Annonce.donnees(petit)))
        assertTrue("préfixes égaux : les deux initient", Annonce.doitInitier(petit, Annonce.donnees(petit)))
    }

    @Test
    fun `annonce d un relais reconnue a son octet de flags`() {
        val relais = byteArrayOf(0x93.toByte(), 0x09, 0xE5.toByte(), 0x5E, 0x05) // préfixe + RELAY|ACCEPTS_CONN
        val telephone = byteArrayOf(0xF1.toByte(), 0xD6.toByte(), 0xA8.toByte(), 0x60)
        val courrier = byteArrayOf(1, 2, 3, 4, 0x02) // COURIER seul
        assertTrue(Annonce.estRelais(relais))
        assertFalse(Annonce.estRelais(telephone))
        assertFalse(Annonce.estRelais(courrier))
        assertFalse(Annonce.estRelais(null))
    }

    @Test
    fun `motif de deconnexion selon le status GATT`() {
        assertEquals(DisconnectReason.LOCALE, motifDeconnexion(8, demandeeLocalement = true))
        assertEquals(DisconnectReason.BRUTALE, motifDeconnexion(8, demandeeLocalement = false))
        assertEquals(DisconnectReason.BRUTALE, motifDeconnexion(133, demandeeLocalement = false))
        assertEquals(DisconnectReason.PROPRE, motifDeconnexion(19, demandeeLocalement = false))
        assertEquals(DisconnectReason.PROPRE, motifDeconnexion(0, demandeeLocalement = false))
        assertEquals(DisconnectReason.LOCALE, motifDeconnexion(22, demandeeLocalement = false))
    }
}
