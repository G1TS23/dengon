package com.dengon.app.ffi

import com.dengon.app.ble.Maillage
import com.dengon.app.ble.PeerIdOctets
import com.dengon.app.ble.peerIdDeLAnnonce
import com.dengon.app.ble.transport.AndroidTransport
import com.dengon.app.ble.transport.BleRadio
import com.dengon.app.ble.transport.DisconnectReason
import com.dengon.app.ble.transport.RadioPeer
import com.dengon.app.ble.transport.RappelsRadio
import com.dengon.app.ble.transport.TransportConfig
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import java.io.File
import java.nio.file.Files

/**
 * Test d'intégration Kotlin ↔ Rust (US-302) : deux vrais `DengonNode`
 * (Alice, Bob), à travers JNA et libdengon_ffi.so — aucun bouchon.
 *
 * La radio est remplacée par [pomper], qui recopie les trames de
 * `takeOutgoing` de l'un vers `onBytesReceived` de l'autre — ou, depuis
 * l'US-306, par le vrai chemin de l'app : `Maillage` + `AndroidTransport`
 * sur une radio de test ([RadioReliee]).
 */
class DengonNodeIntegrationTest {

    private lateinit var dossier: File
    private val ouverts = mutableListOf<DengonNode>()

    @Before
    fun preparer() {
        FfiNatif.exiger()
        dossier = Files.createTempDirectory("dengon-ffi").toFile()
    }

    @After
    fun nettoyer() {
        ouverts.forEach { it.close() }
        if (::dossier.isInitialized) dossier.deleteRecursively()
    }

    private fun ouvrir(nom: String, cle: ByteArray = CLE): DengonNode =
        DengonNode.open(File(dossier, nom).absolutePath, cle, nom).also { ouverts += it }

    /** Recopie les trames dans les deux sens jusqu'au silence. */
    private fun pomper(a: DengonNode, b: DengonNode) {
        val idA = a.localIdentity().peerId
        val idB = b.localIdentity().peerId
        repeat(32) {
            val deA = a.takeOutgoing()
            val deB = b.takeOutgoing()
            if (deA.isEmpty() && deB.isEmpty()) return
            deA.forEach {
                assertEquals(idB, it.peerId)
                b.onBytesReceived(idA, it.frame)
            }
            deB.forEach {
                assertEquals(idA, it.peerId)
                a.onBytesReceived(idB, it.frame)
            }
        }
        fail("les deux nœuds échangent encore après 32 tours")
    }

    @Test
    fun `alice envoie un message chiffre et bob recoit l evenement`() {
        val alice = ouvrir("alice")
        val bob = ouvrir("bob")
        val idAlice = alice.localIdentity().peerId
        val idBob = bob.localIdentity().peerId

        // Appairage (ce que fait l'écran QR), puis connexion radio.
        alice.addContact(identityFromQrCode(identityQrCode(bob.localIdentity())))
        bob.addContact(identityFromQrCode(identityQrCode(alice.localIdentity())))
        alice.onPeerConnected(idBob)
        bob.onPeerConnected(idAlice)
        pomper(alice, bob)
        assertTrue(alice.pollEvents().contains(NodeEvent.PeerConnected(idBob)))
        bob.pollEvents()

        val msgUuid = alice.sendMessage(idBob, "bonjour Bob")
        pomper(alice, bob)

        val recus = bob.pollEvents().filterIsInstance<NodeEvent.MessageReceived>()
        assertEquals(1, recus.size)
        val message = recus.single().message
        assertEquals("bonjour Bob", message.body)
        assertEquals(idAlice, message.authorPeerId)
        assertFalse(message.outgoing)

        val statuts = alice.pollEvents()
            .filterIsInstance<NodeEvent.StatusChanged>()
            .filter { it.msgUuid == msgUuid }
            .map { it.status }
        // Scénario 1 du DoD : parti, puis distribué par l'accusé de Bob (US-306).
        assertEquals(listOf(MessageStatus.IN_FLIGHT, MessageStatus.DELIVERED), statuts)

        // Même conversation des deux côtés.
        val convBob = bob.listConversations().single()
        assertEquals(alice.listConversations().single().convId, convBob.convId)
        assertEquals(listOf("bonjour Bob"), bob.listMessages(convBob.convId).map { it.body })
    }

    /**
     * Le chemin complet de l'app (US-306), radio exceptée : deux vrais nœuds,
     * chacun derrière son `AndroidTransport` et son [Maillage], reliés par
     * deux [RadioReliee] (morceaux de 20 o : la fragmentation BLE est
     * exercée). Aucun `peerID` n'est donné à la main : chaque côté l'apprend
     * par l'`ANNOUNCE` de l'autre.
     */
    @Test
    fun `deux maillages relies par la radio vont jusqu a DELIVERED`() {
        val alice = ouvrir("alice")
        val bob = ouvrir("bob")
        val idAlice = alice.localIdentity().peerId
        val idBob = bob.localIdentity().peerId
        alice.addContact(bob.localIdentity())
        bob.addContact(alice.localIdentity())

        val radioA = RadioReliee()
        val radioB = RadioReliee()
        radioA.autre = radioB
        radioB.autre = radioA
        val tA = AndroidTransport(radioA).apply { start(TransportConfig(PeerIdOctets.depuisBase32(idAlice))) }
        val tB = AndroidTransport(radioB).apply { start(TransportConfig(PeerIdOctets.depuisBase32(idBob))) }
        val mA = Maillage(tA, alice)
        val mB = Maillage(tB, bob)
        fun tourner() = repeat(20) {
            mA.traiter(tA.poll())
            mB.traiter(tB.poll())
        }

        radioA.connecter()
        radioB.connecter()
        tourner()
        assertEquals(idBob, mA.pairs.values.single())
        assertEquals(idAlice, mB.pairs.values.single())
        assertTrue(alice.pollEvents().contains(NodeEvent.PeerConnected(idBob)))
        bob.pollEvents()

        val msgUuid = alice.sendMessage(idBob, "par la radio")
        mA.vider()
        tourner()

        val recu = bob.pollEvents().filterIsInstance<NodeEvent.MessageReceived>().single().message
        assertEquals("par la radio", recu.body)
        assertEquals(idAlice, recu.authorPeerId)
        val statuts = alice.pollEvents()
            .filterIsInstance<NodeEvent.StatusChanged>()
            .filter { it.msgUuid == msgUuid }
            .map { it.status }
        assertEquals(listOf(MessageStatus.IN_FLIGHT, MessageStatus.DELIVERED), statuts)

        radioA.couper()
        radioB.couper()
        tourner()
        assertTrue(mA.pairs.isEmpty())
        assertTrue(alice.pollEvents().contains(NodeEvent.PeerDisconnected(idBob)))
    }

    @Test
    fun `le peerID base32 du FFI se decode en l identifiant du paquet`() {
        val alice = ouvrir("alice")
        val octets = PeerIdOctets.depuisBase32(alice.localIdentity().peerId)
        // `sender_id` d'un paquet L3 : octets 12 à 20 (voir protocol::codec).
        assertTrue(octets.contentEquals(alice.announceFrame().copyOfRange(12, 20)))
        assertEquals(alice.localIdentity().peerId, peerIdDeLAnnonce(alice.announceFrame()))
        assertNull(peerIdDeLAnnonce("pas un ANNOUNCE".toByteArray()))
    }

    @Test
    fun `l identite survit a la reouverture du coffre`() {
        val premiere = ouvrir("alice").localIdentity()
        val seconde = ouvrir("alice").localIdentity()
        assertEquals(premiere.peerId, seconde.peerId)
        assertTrue(premiere.pubStatic.contentEquals(seconde.pubStatic))
    }

    @Test
    fun `une mauvaise cle de coffre est refusee`() {
        ouvrir("alice")
        try {
            ouvrir("alice", cle = ByteArray(32) { 1 })
            fail("coffre ouvert avec une mauvaise clé")
        } catch (e: DengonException.Internal) {
            // attendu
        }
    }

    @Test
    fun `ecrire a un inconnu leve UnknownPeer`() {
        val alice = ouvrir("alice")
        val inconnu = generateIdentity("carol").peerId
        try {
            alice.sendMessage(inconnu, "x")
            fail("envoi à un pair inconnu accepté")
        } catch (e: DengonException.UnknownPeer) {
            // attendu
        }
    }

    private companion object {
        val CLE = ByteArray(32) { 7 }
    }
}

/**
 * Radio de test à un seul pair : chaque morceau écrit est livré tel quel à
 * la radio [autre], comme une écriture GATT arriverait de l'autre côté.
 */
private class RadioReliee : BleRadio {
    lateinit var autre: RadioReliee
    private var rappels: RappelsRadio? = null
    private val pair = RadioPeer("AA:BB:CC:DD:EE:FF", RadioPeer.Role.PERIPHERAL)

    override fun demarrer(cfg: TransportConfig, rappels: RappelsRadio) {
        this.rappels = rappels
    }

    override fun arreter() = Unit

    override fun chargeUtile(pair: RadioPeer): Int = 20

    override fun ecrire(pair: RadioPeer, morceau: ByteArray): Boolean {
        autre.rappels!!.morceauRecu(autre.pair, morceau)
        return true
    }

    override fun deconnecter(pair: RadioPeer) = Unit

    fun connecter() = rappels!!.connecte(pair, -50)

    fun couper() = rappels!!.deconnecte(pair, DisconnectReason.BRUTALE)
}
