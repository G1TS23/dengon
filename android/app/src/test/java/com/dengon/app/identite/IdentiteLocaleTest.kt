package com.dengon.app.identite

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pseudo du premier lancement (US-302). Le `peerId`, lui, ne dépend plus du
 * pseudo (clés tirées par `dengon-core`) : la régression US-215 « même
 * `peerId` sur deux téléphones » est couverte par `DengonNodeIntegrationTest`.
 */
class IdentiteLocaleTest {

    @Test
    fun `le pseudo par defaut est le modele du telephone`() {
        assertEquals("Pixel 8 Pro", IdentiteLocale.pseudoParDefaut("  Pixel 8 Pro "))
    }

    @Test
    fun `modele absent ou vide - pseudo de repli`() {
        assertEquals("dengon", IdentiteLocale.pseudoParDefaut(null))
        assertEquals("dengon", IdentiteLocale.pseudoParDefaut("   "))
    }

    @Test
    fun `pseudo borne a 255 octets UTF-8 meme en caracteres larges`() {
        val pseudo = IdentiteLocale.pseudoParDefaut("伝".repeat(200))
        assertEquals(IdentiteLocale.PSEUDO_MAX, pseudo.length)
        assertTrue(pseudo.toByteArray(Charsets.UTF_8).size <= 255)
    }
}
