package com.dengon.app.ffi

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Preuve que le bouchon `DengonNode` (US-106) suffit à faire avancer l'UI :
 * une conversation est visible sans appel préalable, et le contrat
 * `sendMessage`/`pollEvents`/`onPeerConnected` se comporte comme attendu.
 */
class DengonNodeStubTest {

    @Test
    fun `une conversation canned est visible sans appel prealable`() {
        val identity = DengonIdentity.generate("alice")
        val node: DengonNode = DengonNodeStub(identity)

        val conversations = node.listConversations()

        assertEquals(1, conversations.size)
        val conversation = conversations.first()
        assertEquals("conv-canned", conversation.convId)
        assertEquals(MessageStatus.DELIVERED, conversation.lastMessage?.status)
    }

    @Test
    fun `envoyer un message cree une conversation et signale le pair connecte`() {
        val identity = DengonIdentity.generate("alice")
        val node: DengonNode = DengonNodeStub(identity)

        node.onPeerConnected("bob")
        val msgUuid = node.sendMessage("bob", "salut")

        val events = node.pollEvents()
        assertTrue(events.any { it is NodeEvent.PeerConnected && it.peerId == "bob" })

        val messages = node.listMessages("conv-bob")
        assertEquals(1, messages.size)
        assertEquals(msgUuid, messages.first().msgUuid)
        assertEquals(MessageStatus.IN_FLIGHT, messages.first().status)
    }

    @Test
    fun `pollEvents ne renvoie chaque evenement qu une seule fois`() {
        val node: DengonNode = DengonNodeStub(DengonIdentity.generate("alice"))
        node.onPeerConnected("bob")

        assertEquals(1, node.pollEvents().size)
        assertTrue(node.pollEvents().isEmpty())
    }

    @Test
    fun `un qr code encode puis decode redonne la meme identite`() {
        val identity = DengonIdentity.generate("alice")

        val qr = DengonIdentity.qrCode(identity)
        val decoded = DengonIdentity.fromQrCode(qr)

        assertEquals(identity, decoded)
    }

    @Test
    fun `fromQrCode leve DengonException sur un payload tronque au lieu de planter`() {
        assertThrows(DengonException::class.java) {
            DengonIdentity.fromQrCode("dengon:v1:")
        }
    }

    @Test
    fun `le code de verification est le meme des deux cotes`() {
        val alice = DengonIdentity.generate("alice")
        val bob = DengonIdentity.generate("bob")

        val codeCoteAlice = DengonIdentity.verificationCode(alice, bob)
        val codeCoteBob = DengonIdentity.verificationCode(bob, alice)

        assertEquals(codeCoteAlice, codeCoteBob)
        assertEquals(12, codeCoteAlice.split(" ").size)
    }
}
