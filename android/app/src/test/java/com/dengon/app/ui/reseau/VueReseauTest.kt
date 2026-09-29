package com.dengon.app.ui.reseau

import com.dengon.app.ble.transport.LinkId
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Logique de l'écran réseau (US-313) : qui est pair, qui est relais. */
class VueReseauTest {

    @Test
    fun `un pseudo relay- classe le pair parmi les relais atteints`() {
        val vue = vueReseau(
            serviceDemarre = true,
            modeEco = false,
            liens = mapOf(LinkId(1) to "AAAA", LinkId(2) to "BBBB"),
            pseudos = mapOf("AAAA" to "alice", "BBBB" to "relay-3f2a9c"),
        )

        assertEquals(listOf("BBBB"), vue.relais.map { it.peerId })
        assertEquals(listOf("AAAA"), vue.pairs.map { it.peerId })
        assertTrue(vue.relais.single().estRelais)
        assertFalse(vue.pairs.single().estRelais)
    }

    @Test
    fun `le pseudo relais- annonce par le firmware est un relais`() {
        val vue = vueReseau(true, false, mapOf(LinkId(1) to "AAAA"), mapOf("AAAA" to "relais-9309"))
        assertEquals(listOf("AAAA"), vue.relais.map { it.peerId })
    }

    @Test
    fun `un pair sans pseudo connu reste un pair simple`() {
        val vue = vueReseau(true, false, mapOf(LinkId(1) to "AAAA"), emptyMap())
        assertEquals(null, vue.pairs.single().pseudo)
        assertTrue(vue.relais.isEmpty())
    }

    @Test
    fun `deux liens vers le meme pair ne le comptent qu une fois`() {
        val vue = vueReseau(true, false, mapOf(LinkId(1) to "AAAA", LinkId(2) to "AAAA"), emptyMap())
        assertEquals(1, vue.pairs.size)
    }

    @Test
    fun `le mode eco et l etat du service sont reportes`() {
        val vue = vueReseau(serviceDemarre = false, modeEco = true, liens = emptyMap(), pseudos = emptyMap())
        assertTrue(vue.modeEco)
        assertFalse(vue.serviceDemarre)
    }
}
