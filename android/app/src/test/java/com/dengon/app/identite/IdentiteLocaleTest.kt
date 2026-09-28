package com.dengon.app.identite

import com.dengon.app.ffi.generateIdentity
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

/**
 * Régression constatée sur deux vrais téléphones (US-215) : avec des pseudos
 * `appareil-xxxx`, tous les appareils avaient le même `peerId` et l'appairage
 * répondait « C'est votre propre QR ».
 */
class IdentiteLocaleTest {

    @Test
    fun `le pseudo tient en 8 octets`() {
        assertEquals("tel-49a0", IdentiteLocale.pseudoPour(byteArrayOf(0x49, 0xA0.toByte())))
        assertEquals(8, IdentiteLocale.pseudoPour(byteArrayOf(0, 0)).toByteArray().size)
    }

    @Test
    fun `deux tirages differents donnent deux peerId differents`() {
        // Les 65 536 tirages possibles : autant de peerId distincts.
        val peerIds = (0 until 65_536).map { n ->
            val pseudo = IdentiteLocale.pseudoPour(byteArrayOf((n shr 8).toByte(), n.toByte()))
            generateIdentity(pseudo).peerId
        }
        assertEquals(65_536, peerIds.toSet().size)
    }

    @Test
    fun `l ancien format appareil-xxxx faisait collisionner les peerId`() {
        // Documente la cause : seuls les 8 premiers octets du pseudo comptent.
        assertEquals(
            generateIdentity("appareil-49a0").peerId,
            generateIdentity("appareil-e275").peerId,
        )
    }

    @Test
    fun `taille d alea incorrecte refusee`() {
        assertThrows(IllegalArgumentException::class.java) { IdentiteLocale.pseudoPour(ByteArray(3)) }
    }
}
