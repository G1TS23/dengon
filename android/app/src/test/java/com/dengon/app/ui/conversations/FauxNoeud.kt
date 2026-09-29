package com.dengon.app.ui.conversations

import com.dengon.app.ffi.Conversation
import com.dengon.app.ffi.DengonNodeInterface
import com.dengon.app.ffi.Identity
import com.dengon.app.ffi.Message
import com.dengon.app.ffi.MessageStatus
import com.dengon.app.ffi.NodeEvent
import com.dengon.app.ffi.OutgoingFrame

/**
 * Faux nœud en mémoire pour tester l'**UI** de messagerie (US-214) sans la
 * bibliothèque native. Reprend l'ancien bouchon `DengonNodeStub` de l'US-106,
 * retiré de l'app à l'US-302 : ici, il ne sert qu'aux tests du ViewModel.
 * Le vrai nœud est couvert par `DengonNodeIntegrationTest`.
 */
open class FauxNoeud : DengonNodeInterface {

    private val messages = mutableListOf<Message>()
    private val conversations = mutableListOf<Conversation>()
    private val evenements = mutableListOf<NodeEvent>()
    private val connectes = mutableSetOf<String>()

    init {
        val bienvenue = Message(
            msgUuid = "msg-canned-0",
            convId = "conv-canned",
            authorPeerId = "peer-canned",
            body = "Bienvenue sur dengon",
            outgoing = false,
            sentMs = 0L,
            status = MessageStatus.DELIVERED,
        )
        messages += bienvenue
        conversations += Conversation("conv-canned", "peer-canned", "Alice", bienvenue, 1u)
    }

    override fun sendMessage(destPeerId: String, body: String): String {
        val msgUuid = "msg-${messages.size}"
        val convId = conversations.firstOrNull { it.peerId == destPeerId }?.convId ?: "conv-$destPeerId"
        val status = if (destPeerId in connectes) MessageStatus.IN_FLIGHT else MessageStatus.QUEUED
        val message = Message(msgUuid, convId, "moi", body, outgoing = true, sentMs = 0L, status = status)
        messages += message
        val i = conversations.indexOfFirst { it.convId == convId }
        if (i >= 0) {
            conversations[i] = conversations[i].copy(lastMessage = message)
        } else {
            conversations += Conversation(convId, destPeerId, destPeerId, message, 0u)
        }
        return msgUuid
    }

    override fun pollEvents(): List<NodeEvent> = evenements.toList().also { evenements.clear() }

    override fun onPeerConnected(peerId: String) {
        if (connectes.add(peerId)) evenements += NodeEvent.PeerConnected(peerId)
    }

    override fun onPeerDisconnected(peerId: String) {
        if (connectes.remove(peerId)) evenements += NodeEvent.PeerDisconnected(peerId)
    }

    override fun listConversations(): List<Conversation> = conversations.toList()

    override fun listMessages(convId: String): List<Message> = messages.filter { it.convId == convId }

    override fun localIdentity(): Identity = Identity("moi", "moi", ByteArray(32), ByteArray(32))

    override fun announceFrame(): ByteArray = "ANNOUNCE moi".toByteArray()

    override fun addContact(contact: Identity) = Unit

    override fun onBytesReceived(peerId: String, frame: ByteArray) = Unit

    override fun takeOutgoing(): List<OutgoingFrame> = emptyList()
}
