package com.dengon.app.identite

import android.content.Context
import android.os.Build
import android.util.Log
import com.dengon.app.ffi.DengonNode
import java.io.File

/**
 * Nœud dengon de cet appareil (US-302), ouvert sur le vrai `dengon-core`.
 *
 * L'identité (clés X25519 + Ed25519) est tirée par le cœur Rust au premier
 * lancement puis relue de son coffre chiffré (`<filesDir>/dengon/identity.vault`,
 * clé : [CleCoffre]) : le `peerId` est stable d'un lancement à l'autre et ne
 * dépend plus du pseudo. L'ancien contournement de l'US-215 (pseudo
 * hexadécimal aléatoire, pour que deux téléphones n'aient pas le même
 * `peerId` du bouchon) n'a plus lieu d'être.
 */
object IdentiteLocale {

    private const val TAG = "IdentiteLocale"
    private const val DOSSIER = "dengon"

    /** Nom du coffre dans [DOSSIER] (`VAULT_FILE` de `dengon-ffi`). */
    internal const val COFFRE = "identity.vault"

    /** Longueur maximale du pseudo par défaut, en caractères (≤ 255 octets UTF-8). */
    const val PSEUDO_MAX = 60

    private const val PSEUDO_REPLI = "dengon"

    /**
     * Ouvre le nœud. Le pseudo ne sert qu'au premier lancement : ensuite, le
     * coffre fait foi.
     *
     * Fait des E/S (Keystore, fichiers, génération d'identité au premier
     * lancement) : à ne pas appeler sur le thread principal
     * ([com.dengon.app.DengonApplication] l'ouvre en arrière-plan).
     */
    fun ouvrirNoeud(context: Context): DengonNode {
        val dossier = File(context.filesDir, DOSSIER)
        val source = object : SourceCleCoffre {
            override fun existe() = CleCoffre.existe(context)
            override fun cle() = CleCoffre.cle(context)
            override fun oublier() = CleCoffre.oublier(context)
        }
        val cle = cleDuCoffre(dossier, source) { cause ->
            Log.w(TAG, "clé du coffre perdue : identité réinitialisée", cause)
        }
        return DengonNode.open(dossier.absolutePath, cle, pseudoParDefaut(Build.MODEL))
    }

    /**
     * La clé du coffre de [dossier], en repartant d'une identité neuve si
     * l'ancienne est devenue illisible (revue PR #109) :
     *
     * - clé enregistrée mais irrécupérable (clé du Keystore effacée ou
     *   invalidée) : clé et coffre sont oubliés, [surReinitialisation] est
     *   prévenu, une nouvelle identité sera tirée ;
     * - aucune clé enregistrée mais un coffre présent (préférences effacées
     *   seules) : ce coffre ne s'ouvrira plus jamais, il est supprimé.
     *
     * Sans cela, `DengonNode.open` échouerait à chaque lancement, sans issue.
     * Le prix : un nouveau `peerId`, que les contacts devront ré-appairer.
     */
    internal fun cleDuCoffre(
        dossier: File,
        source: SourceCleCoffre,
        surReinitialisation: (Throwable) -> Unit,
    ): ByteArray {
        if (!source.existe()) File(dossier, COFFRE).delete()
        return try {
            source.cle()
        } catch (e: CleCoffre.CleIrrecuperable) {
            surReinitialisation(e)
            source.oublier()
            File(dossier, COFFRE).delete()
            source.cle()
        }
    }

    /**
     * Pseudo du premier lancement : le modèle du téléphone (« Pixel 8 Pro »),
     * ce qui suffit à distinguer les appareils pendant une démonstration.
     */
    fun pseudoParDefaut(modele: String?): String =
        modele?.trim()?.take(PSEUDO_MAX)?.takeIf { it.isNotEmpty() } ?: PSEUDO_REPLI
}

/** Ce dont [IdentiteLocale.cleDuCoffre] a besoin de [CleCoffre] (remplaçable en test). */
internal interface SourceCleCoffre {
    fun existe(): Boolean
    fun cle(): ByteArray
    fun oublier()
}
