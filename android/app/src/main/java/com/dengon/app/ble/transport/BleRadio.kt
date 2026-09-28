package com.dengon.app.ble.transport

/**
 * Ce qu'[AndroidTransport] demande à la radio : la frontière entre la logique
 * du contrat (testée en JVM pur) et les API Android BLE ([GattRadio]).
 *
 * Une [RadioPeer] désigne une **connexion GATT** telle que la radio la voit
 * (adresse + rôle local). Elle n'est jamais exposée au cœur, qui ne voit que
 * des [LinkId].
 */
interface BleRadio {
    /** Démarre serveur GATT / annonce / scan selon `cfg`. */
    @Throws(TransportException::class)
    fun demarrer(cfg: TransportConfig, rappels: RappelsRadio)

    /** Arrête tout et ferme les connexions (sans rappel de déconnexion). */
    fun arreter()

    /** Octets utilisables par écriture GATT sur cette connexion (ATT_MTU − 3). */
    fun chargeUtile(pair: RadioPeer): Int

    /**
     * Met en file un morceau à envoyer. `false` si la connexion n'existe plus
     * côté radio.
     */
    fun ecrire(pair: RadioPeer, morceau: ByteArray): Boolean

    /** Ferme une connexion (quota dépassé, politique locale). */
    fun deconnecter(pair: RadioPeer)
}

/** Rappels de la radio vers [AndroidTransport]. Appelés sur n'importe quel thread. */
interface RappelsRadio {
    /** Connexion prête à échanger (MTU négocié, notifications activées). */
    fun connecte(pair: RadioPeer, rssi: Short?)

    /** Un morceau est arrivé (écriture sur `CHAR_RX` ou notification `CHAR_TX`). */
    fun morceauRecu(pair: RadioPeer, morceau: ByteArray)

    /** La connexion est tombée. */
    fun deconnecte(pair: RadioPeer, motif: DisconnectReason)
}

/** Une connexion GATT vue par la radio. */
data class RadioPeer(val adresse: String, val role: Role) {
    enum class Role {
        /** Nous sommes client GATT : nous avons initié la connexion. */
        CENTRAL,

        /** Nous sommes serveur GATT : le pair s'est connecté à nous. */
        PERIPHERAL,
    }
}
