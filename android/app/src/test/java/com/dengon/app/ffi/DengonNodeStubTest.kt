package com.dengon.app.ffi

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.Base64
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/**
 * Preuve que le bouchon (US-106) suffit à faire avancer l'UI : une
 * conversation est visible sans appel préalable, et le contrat
 * `sendMessage`/`pollEvents`/`onPeerConnected` se comporte comme attendu.
 *
 * Écrit **uniquement** contre la surface que génèrera UniFFI
 * ([DengonNodeInterface], fonctions de premier niveau, `DengonException`
 * scellée) : à l'US-302, ces tests doivent compiler tels quels contre les
 * bindings générés.
 */
class DengonNodeStubTest {

    private fun assertMemeIdentite(attendue: Identity, obtenue: Identity) {
        // Pas d'`assertEquals(identity, decoded)` : comme dans les bindings
        // générés, `Identity` est une data class à champs `ByteArray`, sans
        // égalité par contenu (revue PR #69).
        assertEquals(attendue.peerId, obtenue.peerId)
        assertEquals(attendue.pseudo, obtenue.pseudo)
        assertArrayEquals(attendue.pubStatic, obtenue.pubStatic)
        assertArrayEquals(attendue.pubSign, obtenue.pubSign)
    }

    @Test
    fun `une conversation canned est visible sans appel prealable`() {
        val identity = generateIdentity("alice")
        val node: DengonNodeInterface = DengonNodeStub(identity)

        val conversations = node.listConversations()

        assertEquals(1, conversations.size)
        val conversation = conversations.first()
        assertEquals("conv-canned", conversation.convId)
        assertEquals(1u, conversation.unreadCount)
        assertEquals(MessageStatus.DELIVERED, conversation.lastMessage?.status)
    }

    @Test
    fun `envoyer un message cree une conversation et signale le pair connecte`() {
        val identity = generateIdentity("alice")
        val node: DengonNodeInterface = DengonNodeStub(identity)

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
    fun `repondre dans la conversation canned reste dans le meme fil`() {
        // Régression (US-214) : la conversation canned s'appelle `conv-canned`,
        // pas `conv-peer-canned`. Répondre à son pair créait une deuxième
        // conversation.
        val node: DengonNodeInterface = DengonNodeStub(generateIdentity("alice"))

        node.sendMessage("peer-canned", "réponse")

        assertEquals(1, node.listConversations().size)
        assertEquals(2, node.listMessages("conv-canned").size)
    }

    @Test
    fun `pollEvents ne renvoie chaque evenement qu une seule fois`() {
        val node: DengonNodeInterface = DengonNodeStub(generateIdentity("alice"))
        node.onPeerConnected("bob")

        assertEquals(1, node.pollEvents().size)
        assertTrue(node.pollEvents().isEmpty())
    }

    @Test
    fun `le bouchon supporte des appels concurrents`() {
        // L'UI et le service de premier plan appellent le nœud depuis des
        // threads différents (revue PR #69, Paul).
        val node: DengonNodeInterface = DengonNodeStub(generateIdentity("alice"))
        val pool = Executors.newFixedThreadPool(8)
        repeat(400) { i ->
            pool.execute {
                node.sendMessage("bob", "m$i")
                node.onPeerConnected("pair-${i % 10}")
                node.listConversations()
            }
        }
        pool.shutdown()
        assertTrue(pool.awaitTermination(30, TimeUnit.SECONDS))

        assertEquals(400, node.listMessages("conv-bob").size)
        assertEquals(400, node.listMessages("conv-bob").map { it.msgUuid }.toSet().size)
        assertEquals(10, node.pollEvents().size)
    }

    @Test
    fun `un qr code encode puis decode redonne la meme identite`() {
        val identity = generateIdentity("alice")

        val qr = identityQrCode(identity)
        val decoded = identityFromQrCode(qr)

        assertMemeIdentite(identity, decoded)
    }

    @Test
    fun `un pseudo trop long est tronque sans couper un caractere`() {
        // 130 « é » = 260 octets UTF-8 : même règle que le Rust (recul à la
        // frontière de caractère, 254 octets), au lieu d'une exception.
        val long = "é".repeat(130)
        val decoded = identityFromQrCode(identityQrCode(generateIdentity(long)))

        assertEquals(254, decoded.pseudo.toByteArray(Charsets.UTF_8).size)
        assertTrue(long.startsWith(decoded.pseudo))
    }

    @Test
    fun `fromQrCode leve DengonException sur un QR qui n est pas un QR dengon`() {
        // Le cas le plus courant : scanner une URL ou un QR de menu. Levait
        // `IllegalArgumentException` avant la revue PR #69.
        assertThrows(DengonException.Internal::class.java) {
            identityFromQrCode("https://example.com")
        }
    }

    @Test
    fun `fromQrCode leve DengonException sur un payload tronque au lieu de planter`() {
        assertThrows(DengonException::class.java) {
            identityFromQrCode("dengon:v1:")
        }
    }

    @Test
    fun `fromQrCode leve DengonException sur un payload non vide mais tronque`() {
        val base64Url = Base64.getUrlEncoder().withoutPadding()

        // Un seul octet `pseudo_len` (5) sans rien derrière : passe la garde
        // « payload vide », doit être arrêté par la garde de taille.
        assertThrows(DengonException::class.java) {
            identityFromQrCode("dengon:v1:" + base64Url.encodeToString(byteArrayOf(5)))
        }

        // QR valide dont on retire le dernier octet (clé `pubSign` incomplète).
        val qr = identityQrCode(generateIdentity("alice"))
        val payload = Base64.getUrlDecoder().decode(qr.removePrefix("dengon:v1:"))
        val tronque = payload.copyOf(payload.size - 1)
        assertThrows(DengonException::class.java) {
            identityFromQrCode("dengon:v1:" + base64Url.encodeToString(tronque))
        }
    }

    @Test
    fun `le code de verification est le meme des deux cotes`() {
        val alice = generateIdentity("alice")
        val bob = generateIdentity("bob")

        val codeCoteAlice = verificationCode(alice, bob)
        val codeCoteBob = verificationCode(bob, alice)

        assertEquals(codeCoteAlice, codeCoteBob)
        assertEquals(12, codeCoteAlice.split(" ").size)
    }
}
