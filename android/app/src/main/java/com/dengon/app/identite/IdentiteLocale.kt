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
 *
 * Deuxième défaut trouvé en revue (PR #94) : le correctif `tel-xxxx` ne
 * faisait encore varier que 2 des 8 octets du `peerId` — le préfixe `tel-`
 * occupait sans le vouloir les 4 premiers, constants, des 8 octets où seule
 * la partie aléatoire compte. Sur 8 octets utiles, seuls 2 étaient
 * réellement aléatoires (2^16 valeurs). Le préfixe est abandonné : le pseudo
 * est maintenant purement hexadécimal, ses 8 octets UTF-8 tombant tous dans
 * la fenêtre du `peerId`. Encodés en hexadécimal (2 caractères par octet
 * aléatoire), ces 8 octets ne représentent que 4 octets de source aléatoire
 * — 2^32 valeurs possibles, contre 2^16 avant.
 */
object IdentiteLocale {

    private const val FICHIER = "dengon_identite"
    private const val CLE_PSEUDO = "pseudo"

    /** Octets de source aléatoire (8 caractères hexadécimaux en sortie). */
    const val OCTETS_ALEATOIRES = 4

    /** Identité de cet appareil ; stable d'un lancement à l'autre. */
    fun identite(context: Context): Identity = generateIdentity(pseudo(context))

    /**
     * Pseudo provisoire, purement hexadécimal, exactement 8 octets : la
     * totalité tombe dans le `peerId` du bouchon, sans octet gaspillé sur un
     * préfixe constant.
     */
    fun pseudoPour(aleatoire: ByteArray): String {
        require(aleatoire.size == OCTETS_ALEATOIRES) { "attendu $OCTETS_ALEATOIRES octets" }
        return aleatoire.joinToString("") { "%02x".format(it) }
    }

    private fun pseudo(context: Context): String {
        val prefs = context.getSharedPreferences(FICHIER, Context.MODE_PRIVATE)
        prefs.getString(CLE_PSEUDO, null)?.let { return it }
        val pseudo = pseudoPour(ByteArray(OCTETS_ALEATOIRES).also { SecureRandom().nextBytes(it) })
        prefs.edit().putString(CLE_PSEUDO, pseudo).apply()
        return pseudo
    }
}
