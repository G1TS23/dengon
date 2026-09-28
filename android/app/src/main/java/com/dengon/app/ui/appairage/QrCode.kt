package com.dengon.app.ui.appairage

import android.util.Log
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.EncodeHintType
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.ReaderException
import com.google.zxing.common.BitMatrix
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel

/** Nombre de masques QR possibles (ISO/IEC 18004 : 0 à 7). */
private const val MASQUES_QR = 8

/**
 * Matrice du QR code de `contenu`, un module par cellule (taille minimale :
 * c'est l'écran qui met à l'échelle). Marge de 2 modules, correction
 * d'erreur `M` (≈ 15 %) : suffisant pour un écran de téléphone net.
 *
 * **Garantie de lisibilité** : certains contenus produisent, avec le masque
 * choisi par défaut, un QR dont le détecteur de motifs de ZXing ne trouve pas
 * les repères — constaté pour l'identité de test « alice » (les données sont
 * correctes, mais aucune détection, quelle que soit l'échelle). Le scanner
 * caméra utilise ce même détecteur : un tel QR serait illisible pour de bon.
 * On vérifie donc que la matrice se relit **par détection**, et sinon on
 * essaie les 8 masques jusqu'à en trouver un lisible. Si aucun des 8 ne l'est
 * (jamais constaté sur les ~300 identités testées), on revient à la matrice
 * par défaut en le signalant (`Log.w`) : elle s'affiche quand même — mieux
 * vaut un QR qui a une petite chance d'être lisible qu'aucun — mais le
 * scanner caméra, qui utilise ce même détecteur, ne la lira pas.
 *
 * Java pur (`zxing-core`) : testable en JVM, sans appareil.
 */
fun matriceQr(contenu: String): BitMatrix {
    val parDefaut = encoder(contenu, masque = null)
    if (seRelitParDetection(parDefaut, contenu)) return parDefaut
    val lisible = (0 until MASQUES_QR).asSequence()
        .map { encoder(contenu, masque = it) }
        .firstOrNull { seRelitParDetection(it, contenu) }
    if (lisible != null) return lisible
    Log.w(TAG, "QR probablement illisible par le scanner caméra : aucun des 8 masques ne se relit par détection")
    return parDefaut
}

private const val TAG = "QrCode"

private fun encoder(contenu: String, masque: Int?): BitMatrix {
    val options = buildMap<EncodeHintType, Any> {
        put(EncodeHintType.MARGIN, 2)
        put(EncodeHintType.ERROR_CORRECTION, ErrorCorrectionLevel.M)
        put(EncodeHintType.CHARACTER_SET, "UTF-8")
        if (masque != null) put(EncodeHintType.QR_MASK_PATTERN, masque)
    }
    return QRCodeWriter().encode(contenu, BarcodeFormat.QR_CODE, 0, 0, options)
}

/**
 * `true` si la matrice, rastérisée à `echelle` pixels par module, est
 * retrouvée **et** décodée par le lecteur ZXing standard (détection des
 * motifs de repérage, comme sur une image de caméra) et rend `contenu`.
 */
internal fun seRelitParDetection(matrice: BitMatrix, contenu: String, echelle: Int = 4): Boolean {
    val largeur = matrice.width * echelle
    val hauteur = matrice.height * echelle
    val pixels = IntArray(largeur * hauteur) { i ->
        if (matrice[(i % largeur) / echelle, (i / largeur) / echelle]) NOIR else BLANC
    }
    val image = BinaryBitmap(HybridBinarizer(RGBLuminanceSource(largeur, hauteur, pixels)))
    return try {
        QRCodeReader().decode(image).text == contenu
    } catch (e: ReaderException) {
        false
    }
}

private const val NOIR = 0xFF000000.toInt()
private const val BLANC = 0xFFFFFFFF.toInt()

/** Nombre de groupes du code de vérification (`powl/04` §2.3). */
const val GROUPES_CODE = 12

/** Chiffres par groupe. */
const val CHIFFRES_PAR_GROUPE = 5

private val FORMAT_CODE = Regex("^\\d{5}( \\d{5}){11}$")

/** `true` si `code` a la forme contractuelle : 12 groupes de 5 chiffres séparés par une espace. */
fun estCodeVerification(code: String): Boolean = FORMAT_CODE.matches(code)

/**
 * Découpe le code en lignes de `groupesParLigne` groupes, pour une lecture
 * croisée à voix haute (3 lignes de 4 groupes par défaut).
 */
fun lignesCode(code: String, groupesParLigne: Int = 4): List<String> =
    code.split(' ').filter { it.isNotEmpty() }.chunked(groupesParLigne) { it.joinToString(" ") }
