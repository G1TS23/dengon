package com.dengon.app.identite

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.security.InvalidKeyException

/**
 * Pseudo du premier lancement (US-302). Le `peerId`, lui, ne dépend plus du
 * pseudo (clés tirées par `dengon-core`) : la régression US-215 « même
 * `peerId` sur deux téléphones » est couverte par `DengonNodeIntegrationTest`.
 */
class IdentiteLocaleTest {

    @get:Rule
    val dossier = TemporaryFolder()

    /** [CleCoffre] en mémoire : `perdue` simule une clé du Keystore effacée. */
    private class FausseSource(var enregistree: ByteArray?, var perdue: Boolean = false) : SourceCleCoffre {
        var oubliee = false
        override fun existe() = enregistree != null
        override fun cle(): ByteArray {
            enregistree?.let { if (perdue) throw CleCoffre.CleIrrecuperable(InvalidKeyException()) else return it }
            return ByteArray(32) { 7 }.also { enregistree = it }
        }
        override fun oublier() {
            enregistree = null
            perdue = false
            oubliee = true
        }
    }

    private fun coffre(): File = File(dossier.root, IdentiteLocale.COFFRE).apply { writeText("chiffré") }

    @Test
    fun `cle enregistree lisible - coffre conserve`() {
        val coffre = coffre()
        val source = FausseSource(ByteArray(32) { 1 })
        var alerte: Throwable? = null
        val cle = IdentiteLocale.cleDuCoffre(dossier.root, source) { alerte = it }
        assertArrayEquals(ByteArray(32) { 1 }, cle)
        assertTrue(coffre.exists())
        assertNull(alerte)
    }

    @Test
    fun `cle du Keystore perdue - identite reinitialisee au lieu de planter`() {
        val coffre = coffre()
        val source = FausseSource(ByteArray(32) { 1 }, perdue = true)
        var alerte: Throwable? = null
        val cle = IdentiteLocale.cleDuCoffre(dossier.root, source) { alerte = it }
        assertTrue(source.oubliee)
        assertFalse("le coffre illisible doit disparaître", coffre.exists())
        assertArrayEquals(ByteArray(32) { 7 }, cle)
        assertTrue(alerte is CleCoffre.CleIrrecuperable)
    }

    @Test
    fun `coffre sans cle enregistree - coffre orphelin supprime`() {
        val coffre = coffre()
        val source = FausseSource(enregistree = null)
        IdentiteLocale.cleDuCoffre(dossier.root, source) {}
        assertFalse(coffre.exists())
        assertTrue(source.existe())
    }

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
