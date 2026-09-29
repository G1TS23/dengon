package com.dengon.app.ffi

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
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
 * `takeOutgoing` de l'un vers `onBytesReceived` de l'autre : exactement le
 * rôle qu'aura `AndroidTransport` (US-213) dans l'app (US-306).
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
        assertTrue("statuts vus : $statuts", MessageStatus.IN_FLIGHT in statuts)

        // Même conversation des deux côtés.
        val convBob = bob.listConversations().single()
        assertEquals(alice.listConversations().single().convId, convBob.convId)
        assertEquals(listOf("bonjour Bob"), bob.listMessages(convBob.convId).map { it.body })
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
