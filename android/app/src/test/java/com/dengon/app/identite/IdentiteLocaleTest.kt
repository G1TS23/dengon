package com.dengon.app.identite

import com.dengon.app.ffi.generateIdentity
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

/**
 * Régression constatée sur deux vrais téléphones (US-215) : avec des pseudos
 * `appareil-xxxx`, tous les appareils avaient le même `peerId` et l'appairage
 * répondait « C'est votre propre QR ».
 *
 * Revue PR #94 : le correctif `tel-xxxx` ne faisait encore varier que 2 des 8
 * octets du `peerId` (`tel-` restait constant) — 2^16 valeurs possibles.
 * Le pseudo est maintenant purement hexadécimal, sans préfixe.
 */
class IdentiteLocaleTest {

    @Test
    fun `le pseudo tient en 8 octets, sans prefixe constant`() {
        assertEquals(
            "49a00102",
            IdentiteLocale.pseudoPour(byteArrayOf(0x49, 0xA0.toByte(), 0x01, 0x02)),
        )
        assertEquals(8, IdentiteLocale.pseudoPour(ByteArray(4)).toByteArray().size)
    }

    @Test
    fun `les octets de l ancien prefixe 'tel-' varient desormais le peerId`() {
        // Avant, ces deux octets valaient toujours 't','e' : aucune variation
        // possible. Ce sont maintenant les deux premiers octets aléatoires,
        // au même titre que les deux autres.
        val peerIds = (0 until 65_536).map { n ->
            val aleatoire = byteArrayOf((n shr 8).toByte(), n.toByte(), 0, 0)
            generateIdentity(IdentiteLocale.pseudoPour(aleatoire)).peerId
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
