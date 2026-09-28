package com.dengon.app.ble.transport

/**
 * Implémentation Android du contrat [Transport] (US-213).
 *
 * Cette classe porte **toutes** les règles du contrat (US-105), sans une
 * ligne d'API Android : attribution des [LinkId], file d'événements, quota,
 * fragmentation BLE, et comportement en déconnexion brutale. La radio
 * réelle ([GattRadio]) est injectée derrière [BleRadio], ce qui permet de
 * tester le contrat en JVM pur (`AndroidTransportConformiteTest`).
 *
 * # Déconnexion brutale (rustdoc de `Transport`, règles 1 à 5)
 *
 * 1. un seul `PeerDisconnected(BRUTALE)` par lien, émis au rappel de la
 *    radio (supervision timeout BLE) ;
 * 2. les trames **complètes** reçues avant la coupure sont déjà dans la file,
 *    donc livrées **avant** l'événement de fermeture ;
 * 3. un réassemblage **partiel** est jeté ;
 * 4. `send` sur le lien fermé lève [TransportException.UnknownPeer] ;
 * 5. plus aucun événement ne porte ce `LinkId` : les rappels tardifs de la
 *    radio pour cette connexion sont ignorés.
 *
 * Thread-safe : les rappels radio arrivent sur les threads Binder, `poll`
 * est appelé par la boucle du cœur. Tout passe par un seul verrou.
 */
class AndroidTransport(private val radio: BleRadio) : Transport, RappelsRadio {

    private class Lien(val pair: RadioPeer) {
        val reassembleur = Reassembleur()
    }

    private val verrou = Any()
    private var cfg: TransportConfig? = null
    private var prochainLien = 0L
    private val liens = LinkedHashMap<LinkId, Lien>()
    private val parPair = HashMap<RadioPeer, LinkId>()
    private val file = ArrayList<TransportEvent>()

    /** Nombre de liens ouverts (diagnostic, écran de debug). */
    val nbLiens: Int get() = synchronized(verrou) { liens.size }

    /**
     * Lien ouvert pour cette connexion radio, s'il existe. Sert au banc
     * d'essai (équivalent du `LinkId` rendu par `BancDEssai::connecter_un_pair`)
     * et à l'écran de debug ; le cœur, lui, ne voit que des [LinkId].
     */
    fun lienPour(pair: RadioPeer): LinkId? = synchronized(verrou) { parPair[pair] }

    override fun start(cfg: TransportConfig) {
        synchronized(verrou) {
            if (this.cfg != null) throw TransportException.AlreadyStarted()
            this.cfg = cfg
        }
        try {
            radio.demarrer(cfg, this)
        } catch (e: TransportException) {
            synchronized(verrou) { this.cfg = null }
            throw e
        }
    }

    /** Arrête la radio. Les liens ouverts sont fermés en `LOCALE`. */
    fun stop() {
        val ouverts = synchronized(verrou) {
            if (cfg == null) return
            cfg = null
            liens.keys.toList()
        }
        radio.arreter()
        synchronized(verrou) {
            for (id in ouverts) fermer(id, DisconnectReason.LOCALE)
        }
    }

    override fun poll(): List<TransportEvent> = synchronized(verrou) {
        if (cfg == null && file.isEmpty()) return emptyList()
        val sortie = file.toList()
        file.clear()
        sortie
    }

    override fun send(peerLinkId: LinkId, bytes: ByteArray) {
        val (pair, taille) = synchronized(verrou) {
            if (cfg == null) throw TransportException.NotStarted()
            verifierTaille(bytes)
            val lien = liens[peerLinkId] ?: throw TransportException.UnknownPeer(peerLinkId)
            lien.pair to radio.chargeUtile(lien.pair)
        }
        // Hors verrou : la radio peut rappeler `deconnecte` pendant l'écriture.
        for (morceau in FragmentationBle.decouper(bytes, taille)) {
            if (!radio.ecrire(pair, morceau)) throw TransportException.UnknownPeer(peerLinkId)
        }
    }

    override fun broadcast(bytes: ByteArray) {
        val ouverts = synchronized(verrou) {
            if (cfg == null) throw TransportException.NotStarted()
            verifierTaille(bytes)
            liens.keys.toList()
        }
        for (id in ouverts) {
            // « Au mieux » : un pair qui décroche n'empêche pas les autres.
            try {
                send(id, bytes)
            } catch (_: TransportException) {
                // lien fermé entre-temps : condition normale
            }
        }
    }

    private fun verifierTaille(bytes: ByteArray) {
        if (bytes.size > FragmentationBle.TRAME_MAX) {
            throw TransportException.FrameTooLarge(bytes.size, FragmentationBle.TRAME_MAX)
        }
    }

    // --- Rappels de la radio ---------------------------------------------

    override fun connecte(pair: RadioPeer, rssi: Short?) {
        val refuse = synchronized(verrou) {
            val c = cfg ?: return
            if (pair in parPair) return // rappel en double : un seul lien par connexion
            if (liens.size >= c.maxConnections) {
                true
            } else {
                val id = LinkId(prochainLien++)
                liens[id] = Lien(pair)
                parPair[pair] = id
                file += TransportEvent.PeerConnected(id, rssi)
                false
            }
        }
        // Quota atteint : on refuse la connexion au lieu de saturer (ESP32 / A-4).
        if (refuse) radio.deconnecter(pair)
    }

    override fun morceauRecu(pair: RadioPeer, morceau: ByteArray) {
        synchronized(verrou) {
            val id = parPair[pair] ?: return // règle 5 : lien inconnu ou fermé
            val lien = liens[id] ?: return
            val trame = try {
                lien.reassembleur.ajouter(morceau)
            } catch (_: TransportException) {
                // Morceau invalide ou trame trop grande : le réassemblage est
                // jeté, le lien reste ouvert (le pair enverra la suite).
                null
            }
            if (trame != null) file += TransportEvent.FrameReceived(id, trame)
        }
    }

    override fun deconnecte(pair: RadioPeer, motif: DisconnectReason) {
        synchronized(verrou) {
            val id = parPair[pair] ?: return // règle 1 : un seul événement par lien
            fermer(id, motif)
        }
    }

    /** À appeler sous verrou. */
    private fun fermer(id: LinkId, motif: DisconnectReason) {
        val lien = liens.remove(id) ?: return
        parPair.remove(lien.pair)
        lien.reassembleur.abandonner() // règle 3
        file += TransportEvent.PeerDisconnected(id, motif) // après les trames déjà reçues (règle 2)
    }
}
