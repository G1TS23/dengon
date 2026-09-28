package com.dengon.app.ble.transport

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Comportements d'[AndroidTransport] **hors** de la suite partagée :
 * la règle 3 (trame partielle jetée), que `conformance.rs` renvoie
 * explicitement aux bancs matériels d'US-213 ; la fragmentation BLE à
 * l'envoi ; le quota ; l'arrêt ; les rappels radio anormaux.
 */
class AndroidTransportTest {

    private val radio = FauxRadio()
    private val t = AndroidTransport(radio)
    private val cfg = TransportConfig(localPeerId = ByteArray(8))

    private fun demarreAvecUnPair(): Pair<RadioPeer, LinkId> {
        t.start(cfg)
        val pair = radio.nouveauPair()
        radio.connecter(pair)
        return pair to t.lienPour(pair)!!
    }

    // --- Règle 3 du contrat -------------------------------------------------

    @Test
    fun `regle 3 - une trame partielle est jetee a la coupure`() {
        val (pair, lien) = demarreAvecUnPair()
        t.poll()
        val morceaux = FragmentationBle.decouper(ByteArray(100) { it.toByte() }, 20)
        morceaux.dropLast(1).forEach { radio.rappels!!.morceauRecu(pair, it) }

        radio.couper(pair, DisconnectReason.BRUTALE)

        val evenements = t.poll()
        assertTrue(evenements.none { it is TransportEvent.FrameReceived })
        assertEquals(listOf(TransportEvent.PeerDisconnected(lien, DisconnectReason.BRUTALE)), evenements)
    }

    @Test
    fun `regle 3 - le partiel d un ancien lien ne contamine pas le nouveau`() {
        val (pair, _) = demarreAvecUnPair()
        FragmentationBle.decouper(ByteArray(100), 20).first().let { radio.rappels!!.morceauRecu(pair, it) }
        radio.couper(pair, DisconnectReason.BRUTALE)

        radio.connecter(pair) // même adresse, nouvelle connexion
        val nouveau = t.lienPour(pair)!!
        radio.faireRecevoir(pair, "neuve".toByteArray())
        val trames = t.poll().filterIsInstance<TransportEvent.FrameReceived>()
        assertEquals(1, trames.size)
        assertEquals(nouveau, trames[0].peerLinkId)
        assertArrayEquals("neuve".toByteArray(), trames[0].bytes)
    }

    // --- Fragmentation BLE à l'envoi -------------------------------------------

    @Test
    fun `send decoupe selon la charge utile et le pair recolle la trame`() {
        val (pair, lien) = demarreAvecUnPair()
        val trame = ByteArray(1000) { (it * 7).toByte() }
        t.send(lien, trame)

        val morceaux = radio.ecrits.getValue(pair)
        assertEquals(53, morceaux.size) // 19 octets de données par morceau de 20
        assertTrue(morceaux.all { it.size <= 20 })
        assertArrayEquals(trame, radio.trameEcrite(pair))
    }

    @Test
    fun `trames trop grandes refusees sans toucher la radio`() {
        val (pair, lien) = demarreAvecUnPair()
        val trop = ByteArray(FragmentationBle.TRAME_MAX + 1)
        val e = assertThrows(TransportException.FrameTooLarge::class.java) { t.send(lien, trop) }
        assertEquals(FragmentationBle.TRAME_MAX, e.max)
        assertThrows(TransportException.FrameTooLarge::class.java) { t.broadcast(trop) }
        assertNull(radio.ecrits[pair])
    }

    @Test
    fun `echec d ecriture en cours d envoi - UnknownPeer`() {
        val (pair, lien) = demarreAvecUnPair()
        radio.muettes += pair
        assertThrows(TransportException.UnknownPeer::class.java) { t.send(lien, "x".toByteArray()) }
    }

    @Test
    fun `broadcast au mieux - un pair muet n empeche pas les autres`() {
        t.start(cfg)
        val muet = radio.nouveauPair().also { radio.connecter(it) }
        val vivant = radio.nouveauPair().also { radio.connecter(it) }
        radio.muettes += muet

        t.broadcast("à tous".toByteArray())

        assertArrayEquals("à tous".toByteArray(), radio.trameEcrite(vivant))
        assertNull(radio.ecrits[muet])
    }

    // --- Quota et rappels anormaux -----------------------------------------------

    @Test
    fun `au dela du quota la connexion est refusee`() {
        t.start(cfg.copy(maxConnections = 1))
        val premier = radio.nouveauPair().also { radio.connecter(it) }
        val second = radio.nouveauPair().also { radio.connecter(it) }

        assertEquals(1, t.poll().count { it is TransportEvent.PeerConnected })
        assertEquals(listOf(second), radio.deconnecteesLocalement)
        assertNull(t.lienPour(second))
        assertEquals(1, t.nbLiens)
        assertTrue(t.lienPour(premier) != null)
    }

    @Test
    fun `connexion signalee deux fois - un seul lien`() {
        val (pair, lien) = demarreAvecUnPair()
        radio.rappels!!.connecte(pair, -50)
        assertEquals(lien, t.lienPour(pair))
        assertEquals(1, t.poll().count { it is TransportEvent.PeerConnected })
    }

    @Test
    fun `deconnexion signalee deux fois - un seul evenement`() {
        val (pair, _) = demarreAvecUnPair()
        t.poll()
        radio.couper(pair, DisconnectReason.BRUTALE)
        radio.rappels!!.deconnecte(pair, DisconnectReason.PROPRE)
        assertEquals(1, t.poll().size)
    }

    @Test
    fun `morceau invalide jete - le lien reste utilisable`() {
        val (pair, lien) = demarreAvecUnPair()
        t.poll()
        radio.rappels!!.morceauRecu(pair, byteArrayOf(0x41, 1, 2)) // bit réservé
        radio.rappels!!.morceauRecu(pair, byteArrayOf()) // vide
        radio.faireRecevoir(pair, "ok".toByteArray())

        val evenements = t.poll()
        assertEquals(1, evenements.size)
        assertEquals(TransportEvent.FrameReceived(lien, "ok".toByteArray()), evenements[0])
    }

    @Test
    fun `morceau d une connexion inconnue ignore`() {
        t.start(cfg)
        radio.rappels!!.morceauRecu(radio.nouveauPair(), byteArrayOf(0, 1))
        assertTrue(t.poll().isEmpty())
    }

    @Test
    fun `rappels avant start ignores`() {
        radio.demarrer(cfg, t) // rappels branchés sans que le transport soit démarré
        radio.connecter(radio.nouveauPair())
        assertTrue(t.poll().isEmpty())
        assertEquals(0, t.nbLiens)
    }

    // --- Démarrage et arrêt -------------------------------------------------------

    @Test
    fun `echec de la radio au demarrage - le transport reste redemarrable`() {
        radio.echecAuDemarrage = TransportException.Backend("Bluetooth désactivé")
        assertThrows(TransportException.Backend::class.java) { t.start(cfg) }
        assertThrows(TransportException.NotStarted::class.java) { t.broadcast(byteArrayOf()) }

        radio.echecAuDemarrage = null
        t.start(cfg)
        assertTrue(radio.demarree)
    }

    @Test
    fun `stop ferme les liens en LOCALE puis plus rien`() {
        val (_, lien) = demarreAvecUnPair()
        t.poll()
        t.stop()

        assertFalse(radio.demarree)
        assertEquals(listOf(TransportEvent.PeerDisconnected(lien, DisconnectReason.LOCALE)), t.poll())
        assertTrue(t.poll().isEmpty())
        assertThrows(TransportException.NotStarted::class.java) { t.send(lien, byteArrayOf(1)) }
        t.stop() // idempotent
    }

    @Test
    fun `affichage des evenements et des liens`() {
        assertEquals("link#3", LinkId(3).toString())
        assertEquals("FrameReceived(link#1, 2 o)", TransportEvent.FrameReceived(LinkId(1), byteArrayOf(1, 2)).toString())
        assertEquals(
            TransportEvent.FrameReceived(LinkId(1), byteArrayOf(1)).hashCode(),
            TransportEvent.FrameReceived(LinkId(1), byteArrayOf(1)).hashCode(),
        )
        assertThrows(IllegalArgumentException::class.java) { TransportConfig(localPeerId = ByteArray(4)) }
        assertEquals(cfg, TransportConfig(localPeerId = ByteArray(8)))
    }
}
