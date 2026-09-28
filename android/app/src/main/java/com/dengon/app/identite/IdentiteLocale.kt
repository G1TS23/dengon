package com.dengon.app.identite

import android.content.Context
import android.os.Build
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

    private const val DOSSIER = "dengon"

    /** Longueur maximale du pseudo par défaut, en caractères (≤ 255 octets UTF-8). */
    const val PSEUDO_MAX = 60

    private const val PSEUDO_REPLI = "dengon"

    /**
     * Ouvre le nœud. Le pseudo ne sert qu'au premier lancement : ensuite, le
     * coffre fait foi.
     */
    fun ouvrirNoeud(context: Context): DengonNode = DengonNode.open(
        File(context.filesDir, DOSSIER).absolutePath,
        CleCoffre.cle(context),
        pseudoParDefaut(Build.MODEL),
    )

    /**
     * Pseudo du premier lancement : le modèle du téléphone (« Pixel 8 Pro »),
     * ce qui suffit à distinguer les appareils pendant une démonstration.
     */
    fun pseudoParDefaut(modele: String?): String =
        modele?.trim()?.take(PSEUDO_MAX)?.takeIf { it.isNotEmpty() } ?: PSEUDO_REPLI
}
