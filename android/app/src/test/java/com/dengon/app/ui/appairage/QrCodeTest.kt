package com.dengon.app.ui.appairage

import com.dengon.app.ffi.generateIdentity
import com.dengon.app.ffi.identityFromQrCode
import com.dengon.app.ffi.identityQrCode
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.EncodeHintType
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Le QR dessiné par l'écran se relit bien (US-215) : on le rastérise comme
 * le fait `ImageQr`, puis on le décode avec le lecteur ZXing — le même moteur
 * que l'écran de scan caméra.
 */
class QrCodeTest {

    /** Rastérise la matrice (`echelle` pixels par module, noir sur blanc) puis la décode. */
    private fun relire(contenu: String, echelle: Int = 4): String {
        val matrice = matriceQr(contenu)
        val largeur = matrice.width * echelle
        val hauteur = matrice.height * echelle
        val pixels = IntArray(largeur * hauteur) { i ->
            val x = (i % largeur) / echelle
            val y = (i / largeur) / echelle
            if (matrice[x, y]) NOIR else BLANC
        }
        val image = BinaryBitmap(HybridBinarizer(RGBLuminanceSource(largeur, hauteur, pixels)))
        // Lecteur standard, sans indice : même détection que sur une image caméra.
        return QRCodeReader().decode(image).text
    }

    @Test
    fun `le QR d identite dessine se relit a l identique`() {
        val identite = generateIdentity("alice")
        val contenu = identityQrCode(identite)
        val relu = relire(contenu)

        assertEquals(contenu, relu)
        val decodee = identityFromQrCode(relu)
        assertEquals(identite.peerId, decodee.peerId)
        assertArrayEquals(identite.pubStatic, decodee.pubStatic)
        assertArrayEquals(identite.pubSign, decodee.pubSign)
    }

    @Test
    fun `regression - le QR d alice est rendu lisible par changement de masque`() {
        val contenu = identityQrCode(generateIdentity("alice"))
        // Masque choisi par défaut par ZXing : données correctes mais repères
        // introuvables par le détecteur (constaté pendant l'US-215).
        val parDefaut = QRCodeWriter().encode(
            contenu,
            BarcodeFormat.QR_CODE,
            0,
            0,
            mapOf(
                EncodeHintType.MARGIN to 2,
                EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M,
                EncodeHintType.CHARACTER_SET to "UTF-8",
            ),
        )
        assertFalse(seRelitParDetection(parDefaut, contenu))
        assertTrue(seRelitParDetection(matriceQr(contenu), contenu))
    }

    @Test
    fun `tout QR d identite affiche se relit par detection`() {
        // Pseudos variés, dont le format réel (hexadécimal) d'IdentiteLocale.
        val pseudos = (0 until 256).map { "%08x".format(it * 16843009) } +
            (0 until 44).map { "contact $it" }
        val illisibles = pseudos.filterNot { pseudo ->
            val contenu = identityQrCode(generateIdentity(pseudo))
            seRelitParDetection(matriceQr(contenu), contenu)
        }
        assertEquals(emptyList<String>(), illisibles)
    }

    @Test
    fun `pseudo non ASCII et pseudo long passent aussi`() {
        for (pseudo in listOf("Élodie 伝言", "x".repeat(255))) {
            val contenu = identityQrCode(generateIdentity(pseudo))
            assertEquals(contenu, relire(contenu))
        }
    }

    @Test
    fun `matrice carree avec une marge blanche`() {
        val matrice = matriceQr(identityQrCode(generateIdentity("alice")))
        assertEquals(matrice.width, matrice.height)
        // Marge de 2 modules : la première ligne est entièrement blanche.
        assertTrue((0 until matrice.width).none { matrice[it, 0] })
    }

    @Test
    fun `format du code de verification`() {
        val code = "01234 56789 01234 56789 01234 56789 01234 56789 01234 56789 01234 56789"
        assertTrue(estCodeVerification(code))
        assertFalse(estCodeVerification(code.dropLast(1)))
        assertFalse(estCodeVerification(code.replace(' ', '-')))
        assertFalse(estCodeVerification(""))
        assertEquals(GROUPES_CODE * CHIFFRES_PAR_GROUPE, code.count { it.isDigit() })
    }

    @Test
    fun `le code s affiche en 3 lignes de 4 groupes`() {
        val code = (1..12).joinToString(" ") { "%05d".format(it) }
        assertEquals(
            listOf(
                "00001 00002 00003 00004",
                "00005 00006 00007 00008",
                "00009 00010 00011 00012",
            ),
            lignesCode(code),
        )
        assertEquals(6, lignesCode(code, groupesParLigne = 2).size)
    }

    private companion object {
        const val NOIR = 0xFF000000.toInt()
        const val BLANC = 0xFFFFFFFF.toInt()
    }
}
