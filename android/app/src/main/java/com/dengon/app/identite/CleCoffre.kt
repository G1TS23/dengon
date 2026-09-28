package com.dengon.app.identite

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Clé du coffre d'identité (US-302) : les 32 octets qui déchiffrent
 * `identity.vault`, où `dengon-core` range les clés privées de l'appareil.
 *
 * Tirée une fois au premier lancement, puis rangée **enveloppée** dans les
 * SharedPreferences : chiffrée AES-GCM par une clé du Keystore Android, qui ne
 * quitte jamais le matériel sécurisé du téléphone
 * (`docs/synthese/06-securite.md`). Copier le répertoire de l'app sur une
 * autre machine ne suffit donc pas à ouvrir le coffre.
 *
 * Pas de test JVM : le Keystore n'existe que sur un appareil.
 */
object CleCoffre {

    private const val KEYSTORE = "AndroidKeyStore"
    private const val ALIAS = "dengon-coffre"
    private const val FICHIER = "dengon_coffre"
    private const val CLE_ENVELOPPEE = "cle_enveloppee"
    private const val TRANSFORMATION = "AES/GCM/NoPadding"
    private const val LONGUEUR_CLE = 32
    private const val LONGUEUR_IV = 12
    private const val BITS_TAG = 128

    /** La clé du coffre ; créée et enregistrée au premier appel. */
    fun cle(context: Context): ByteArray {
        val prefs = context.getSharedPreferences(FICHIER, Context.MODE_PRIVATE)
        prefs.getString(CLE_ENVELOPPEE, null)?.let {
            return desenvelopper(Base64.decode(it, Base64.NO_WRAP))
        }
        val cle = ByteArray(LONGUEUR_CLE).also { SecureRandom().nextBytes(it) }
        // `commit` et non `apply` : la clé doit être sur disque avant que le
        // coffre chiffré avec elle n'y soit, sinon un arrêt entre les deux
        // laisserait un coffre que plus rien n'ouvre.
        check(prefs.edit().putString(CLE_ENVELOPPEE, Base64.encodeToString(envelopper(cle), Base64.NO_WRAP)).commit()) {
            "clé du coffre non enregistrée"
        }
        return cle
    }

    /** `iv (12 o) ‖ AES-GCM(cle)`, par la clé du Keystore. */
    private fun envelopper(cle: ByteArray): ByteArray {
        val chiffre = Cipher.getInstance(TRANSFORMATION).apply { init(Cipher.ENCRYPT_MODE, cleKeystore()) }
        return chiffre.iv + chiffre.doFinal(cle)
    }

    private fun desenvelopper(enveloppe: ByteArray): ByteArray {
        val iv = enveloppe.copyOfRange(0, LONGUEUR_IV)
        val chiffre = Cipher.getInstance(TRANSFORMATION).apply {
            init(Cipher.DECRYPT_MODE, cleKeystore(), GCMParameterSpec(BITS_TAG, iv))
        }
        return chiffre.doFinal(enveloppe, LONGUEUR_IV, enveloppe.size - LONGUEUR_IV)
    }

    private fun cleKeystore(): SecretKey {
        val keystore = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (keystore.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val generateur = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        generateur.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generateur.generateKey()
    }
}
