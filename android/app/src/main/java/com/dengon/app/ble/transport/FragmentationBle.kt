package com.dengon.app.ble.transport

/**
 * Fragmentation **BLE** (L1) : découpe d'une trame applicative en morceaux
 * qui tiennent dans une écriture GATT, et réassemblage.
 *
 * C'est la seule fragmentation à la charge du transport (contrat US-105) ; la
 * fragmentation *protocole* (paquet > `FRAG_SIZE`) est faite par
 * `dengon-core::protocol`.
 *
 * Format d'un morceau (proposition, à aligner avec NimBLE US-220 — consignée
 * dans `docs/suivi/03-ecarts-conception.md`) :
 *
 * ```text
 * morceau = en-tête:u8 ‖ données
 * en-tête : bit 7 = SUITE (d'autres morceaux suivent), bits 0-6 = 0
 * ```
 *
 * Une trame = une suite de morceaux dont seul le dernier n'a pas SUITE. GATT
 * garantit l'ordre sur une connexion, il n'y a donc ni numéro ni longueur
 * totale. Une trame vide est un morceau sans données.
 */
object FragmentationBle {
    /** Bit « d'autres morceaux suivent ». */
    const val SUITE: Int = 0x80

    /** Octets d'en-tête par morceau. */
    const val EN_TETE: Int = 1

    /**
     * Taille maximale d'une trame réassemblée. Borne la mémoire d'un
     * réassemblage (un pair ne peut pas nous faire accumuler sans fin) ;
     * largement au-dessus d'un fragment protocole (`FRAG_SIZE` 440 + en-têtes)
     * et d'une enveloppe (`ENVELOPE_MAX_BYTES` 4096).
     */
    const val TRAME_MAX: Int = 8192

    /** Charge utile par écriture GATT pour un ATT_MTU donné (en-tête ATT : 3 o). */
    fun chargeUtile(attMtu: Int): Int = (attMtu - 3).coerceAtLeast(EN_TETE + 1)

    /** Découpe `trame` en morceaux d'au plus `taille` octets (en-tête compris). */
    fun decouper(trame: ByteArray, taille: Int): List<ByteArray> {
        require(taille > EN_TETE) { "taille de morceau trop petite : $taille" }
        val donneesParMorceau = taille - EN_TETE
        if (trame.isEmpty()) return listOf(byteArrayOf(0))
        val morceaux = ArrayList<ByteArray>((trame.size + donneesParMorceau - 1) / donneesParMorceau)
        var debut = 0
        while (debut < trame.size) {
            val fin = minOf(debut + donneesParMorceau, trame.size)
            val morceau = ByteArray(EN_TETE + fin - debut)
            morceau[0] = (if (fin < trame.size) SUITE else 0).toByte()
            trame.copyInto(morceau, destinationOffset = EN_TETE, startIndex = debut, endIndex = fin)
            morceaux += morceau
            debut = fin
        }
        return morceaux
    }
}

/**
 * Réassemblage des morceaux d'**un** lien. Pas thread-safe : protégé par le
 * verrou d'[AndroidTransport].
 */
class Reassembleur {
    private val tampon = java.io.ByteArrayOutputStream()

    /** Un réassemblage est-il en cours (morceaux reçus sans le dernier) ? */
    val enCours: Boolean get() = tampon.size() > 0 || attendSuite

    private var attendSuite = false

    /**
     * Ajoute un morceau. Rend la trame complète si c'était le dernier, `null`
     * sinon.
     *
     * @throws TransportException.FrameTooLarge si la trame dépasse
     *   [FragmentationBle.TRAME_MAX] (le réassemblage est alors abandonné).
     * @throws TransportException.Backend si le morceau est vide ou porte des
     *   bits réservés.
     */
    fun ajouter(morceau: ByteArray): ByteArray? {
        if (morceau.isEmpty()) {
            abandonner()
            throw TransportException.Backend("morceau BLE vide")
        }
        val entete = morceau[0].toInt() and 0xFF
        if (entete and FragmentationBle.SUITE.inv() and 0xFF != 0) {
            abandonner()
            throw TransportException.Backend("en-tête de morceau BLE inconnu : 0x%02x".format(entete))
        }
        val donnees = morceau.size - FragmentationBle.EN_TETE
        if (tampon.size() + donnees > FragmentationBle.TRAME_MAX) {
            val taille = tampon.size() + donnees
            abandonner()
            throw TransportException.FrameTooLarge(taille, FragmentationBle.TRAME_MAX)
        }
        tampon.write(morceau, FragmentationBle.EN_TETE, donnees)
        if (entete and FragmentationBle.SUITE != 0) {
            attendSuite = true
            return null
        }
        val trame = tampon.toByteArray()
        abandonner()
        return trame
    }

    /** Jette un réassemblage partiel (coupure : règle 3 du contrat). */
    fun abandonner() {
        tampon.reset()
        attendSuite = false
    }
}
