package com.dengon.app.ble.transport

import java.util.UUID

/**
 * Constantes GATT du service `dengon` — mêmes valeurs que
 * `dengon_core::protocol::consts` (`SERVICE_UUID`, `CHAR_RX_UUID`,
 * `CHAR_TX_UUID`), `docs/synthese/05-protocole-et-trame.md` §2.
 */
object GattDengon {
    val SERVICE: UUID = UUID.fromString("6d656e67-2d64-656e-676f-6e2d76310000")

    /** Écriture sans réponse, pair → nœud. */
    val CHAR_RX: UUID = UUID.fromString("6d656e67-2d64-656e-676f-6e2d76310001")

    /** Notification, nœud → pair. */
    val CHAR_TX: UUID = UUID.fromString("6d656e67-2d64-656e-676f-6e2d76310002")

    /** Descripteur standard « Client Characteristic Configuration ». */
    val CCCD: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")

    /**
     * Longueur maximale d'une valeur d'attribut GATT (spécification Bluetooth,
     * vol. 3 partie F §3.2.9) : même avec un ATT_MTU de 517, une écriture ou
     * une notification ne porte pas plus de 512 octets.
     */
    const val VALEUR_MAX: Int = 512
}

/**
 * *Manufacturer data* annoncée : les 4 premiers octets du `peerID` local
 * (`TransportConfig.localPeerId`, rustdoc du contrat).
 */
object Annonce {
    /**
     * Identifiant de fabricant `0xFFFF` : réservé par le Bluetooth SIG aux
     * tests et usages internes. dengon n'a pas d'identifiant attribué.
     */
    const val ID_FABRICANT: Int = 0xFFFF

    /** Octets du `peerID` annoncés. */
    const val PREFIXE: Int = 4

    /** Charge utile de la *manufacturer data* pour ce `peerID` (8 octets). */
    fun donnees(localPeerId: ByteArray): ByteArray {
        require(localPeerId.size == 8) { "peerID : 8 octets attendus" }
        return localPeerId.copyOfRange(0, PREFIXE)
    }

    /** Préfixe annoncé par un pair, ou `null` si l'annonce n'est pas lisible. */
    fun prefixeDistant(donnees: ByteArray?): ByteArray? =
        donnees?.takeIf { it.size >= PREFIXE }?.copyOfRange(0, PREFIXE)

    /**
     * **Règle anti-boucle de connexion** (`synthese/05` §6, rustdoc de
     * `TransportConfig::local_peer_id`) : quand deux nœuds se découvrent,
     * seul celui dont le préfixe est le plus petit (ordre des octets non
     * signés) initie la connexion GATT. Préfixes égaux : les deux initient —
     * cas rarissime (1 chance sur 2³²), au pire un lien en double.
     */
    fun doitInitier(localPeerId: ByteArray, prefixeDistant: ByteArray): Boolean {
        val local = donnees(localPeerId)
        for (i in 0 until PREFIXE) {
            val a = local[i].toInt() and 0xFF
            val b = prefixeDistant[i].toInt() and 0xFF
            if (a != b) return a < b
        }
        return true
    }
}

/**
 * Traduit le `status` d'un rappel de déconnexion GATT en [DisconnectReason].
 *
 * Codes HCI rencontrés (`BluetoothGatt` / spécification Bluetooth vol. 1
 * partie F) :
 * - `8` supervision timeout, `34` LMP/LL response timeout, `62` échec
 *   d'établissement, `133` `GATT_ERROR` → **coupure brutale** ;
 * - `19` le pair a fermé → **propre** ;
 * - `22` fermé par l'hôte local → **locale** ;
 * - `0` : le côté **serveur** GATT d'Android rend presque toujours 0, quelle
 *   que soit la cause — traité comme propre ; voir le comportement mesuré
 *   sur appareil dans `docs/suivi/modules/android-app.md`.
 *
 * Une fermeture que **nous** avons demandée est toujours locale.
 */
fun motifDeconnexion(status: Int, demandeeLocalement: Boolean): DisconnectReason = when {
    demandeeLocalement -> DisconnectReason.LOCALE
    status == 19 || status == 0 -> DisconnectReason.PROPRE
    status == 22 -> DisconnectReason.LOCALE
    else -> DisconnectReason.BRUTALE
}
