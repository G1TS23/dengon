package com.dengon.app.ble.transport

/**
 * Radio de test : remplace [GattRadio] pour exercer [AndroidTransport] en JVM
 * pur. Elle joue aussi le rôle du **pair** : `faireRecevoir` découpe la trame
 * avec [FragmentationBle] comme le ferait l'autre téléphone, ce qui fait
 * passer chaque test par le vrai réassemblage.
 */
class FauxRadio(var chargeUtileParDefaut: Int = 20) : BleRadio {

    var rappels: RappelsRadio? = null
        private set
    var demarree = false
        private set
    var echecAuDemarrage: TransportException? = null

    /** Connexions vivantes côté radio. */
    val connectees = mutableSetOf<RadioPeer>()

    /** Connexions dont l'écriture échoue (pair parti entre-temps). */
    val muettes = mutableSetOf<RadioPeer>()

    /** Morceaux écrits, par connexion. */
    val ecrits = mutableMapOf<RadioPeer, MutableList<ByteArray>>()

    /** Connexions fermées à la demande du transport. */
    val deconnecteesLocalement = mutableListOf<RadioPeer>()

    private var numero = 0

    override fun demarrer(cfg: TransportConfig, rappels: RappelsRadio) {
        echecAuDemarrage?.let { throw it }
        this.rappels = rappels
        demarree = true
    }

    override fun arreter() {
        demarree = false
        connectees.clear()
    }

    override fun chargeUtile(pair: RadioPeer): Int = chargeUtileParDefaut

    override fun ecrire(pair: RadioPeer, morceau: ByteArray): Boolean {
        if (pair !in connectees || pair in muettes) return false
        ecrits.getOrPut(pair) { mutableListOf() } += morceau
        return true
    }

    override fun deconnecter(pair: RadioPeer) {
        connectees -= pair
        deconnecteesLocalement += pair
    }

    // --- Actions du « pair » -------------------------------------------------

    fun nouveauPair(role: RadioPeer.Role = RadioPeer.Role.PERIPHERAL): RadioPeer =
        RadioPeer("AA:BB:CC:00:00:%02X".format(++numero), role)

    fun connecter(pair: RadioPeer, rssi: Short? = -60) {
        connectees += pair
        rappels!!.connecte(pair, rssi)
    }

    fun couper(pair: RadioPeer, motif: DisconnectReason) {
        connectees -= pair
        rappels!!.deconnecte(pair, motif)
    }

    fun faireRecevoir(pair: RadioPeer, trame: ByteArray) {
        for (morceau in FragmentationBle.decouper(trame, chargeUtileParDefaut)) {
            rappels!!.morceauRecu(pair, morceau)
        }
    }

    /** Trame réassemblée à partir de ce qu'on a écrit vers `pair`. */
    fun trameEcrite(pair: RadioPeer): ByteArray? {
        val r = Reassembleur()
        var derniere: ByteArray? = null
        for (m in ecrits[pair].orEmpty()) r.ajouter(m)?.let { derniere = it }
        return derniere
    }
}
