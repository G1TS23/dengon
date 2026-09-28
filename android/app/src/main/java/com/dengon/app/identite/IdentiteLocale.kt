package com.dengon.app.identite

import android.content.Context
import com.dengon.app.ffi.Identity
import com.dengon.app.ffi.generateIdentity
import java.security.SecureRandom

/**
 * Identité locale **provisoire** de l'appareil (US-215), en attendant la vraie
 * (keypair + coffre, US-205 branché par US-306).
 *
 * Le bouchon `generateIdentity(pseudo)` dérive les clés **du pseudo**, et le
 * `peerId` n'est fait que de ses **8 premiers octets**. D'où deux contraintes,
 * vérifiées en test :
 * - un pseudo aléatoire par installation (deux téléphones au même pseudo
 *   auraient la même identité) ;
 * - la partie aléatoire **dans les 8 premiers octets** : le premier essai,
 *   `appareil-xxxx`, donnait le même `peerId` (« appareil ») à tous les
 *   téléphones, et l'appairage les prenait pour un seul appareil — constaté
 *   sur un Pixel 8 Pro et un Galaxy A16 (« C'est votre propre QR »).
 */
object IdentiteLocale {

    private const val FICHIER = "dengon_identite"
    private const val CLE_PSEUDO = "pseudo"

    /** Octets aléatoires du pseudo (4 caractères hexadécimaux). */
    const val OCTETS_ALEATOIRES = 2

    /** Identité de cet appareil ; stable d'un lancement à l'autre. */
    fun identite(context: Context): Identity = generateIdentity(pseudo(context))

    /**
     * Pseudo provisoire `tel-xxxx` : exactement 8 octets, donc la partie
     * aléatoire tombe entière dans le `peerId` du bouchon.
     */
    fun pseudoPour(aleatoire: ByteArray): String {
        require(aleatoire.size == OCTETS_ALEATOIRES) { "attendu $OCTETS_ALEATOIRES octets" }
        return "tel-" + aleatoire.joinToString("") { "%02x".format(it) }
    }

    private fun pseudo(context: Context): String {
        val prefs = context.getSharedPreferences(FICHIER, Context.MODE_PRIVATE)
        prefs.getString(CLE_PSEUDO, null)?.let { return it }
        val pseudo = pseudoPour(ByteArray(OCTETS_ALEATOIRES).also { SecureRandom().nextBytes(it) })
        prefs.edit().putString(CLE_PSEUDO, pseudo).apply()
        return pseudo
    }
}
