package com.dengon.app.ui.conversations

import com.dengon.app.ffi.Conversation
import com.dengon.app.ffi.DengonException
import com.dengon.app.ffi.Message
import com.dengon.app.ffi.MessageStatus
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Tests du ViewModel de messagerie (US-214), sur [FauxNoeud] : c'est l'UI
 * qu'on teste ici, pas le nœud (couvert par `DengonNodeIntegrationTest`).
 * Aucune dépendance Android ni coroutine de test : le ViewModel est
 * synchrone, on lit `etat.value`.
 */
class ConversationsViewModelTest {

    private fun vmSurBouchon(): Pair<ConversationsViewModel, FauxNoeud> {
        val noeud = FauxNoeud()
        return ConversationsViewModel(noeud) to noeud
    }

    @Test
    fun `la liste affiche la conversation canned des l ouverture`() {
        val (vm, _) = vmSurBouchon()
        val etat = vm.etat.value

        assertEquals(listOf("conv-canned"), etat.conversations.map { it.convId })
        assertNull("la liste s'affiche, pas un fil", etat.conversationOuverte)
        assertTrue(etat.messages.isEmpty())
    }

    @Test
    fun `ouvrir charge le fil et fermer revient a la liste`() {
        val (vm, _) = vmSurBouchon()

        vm.ouvrir("conv-canned")
        val ouvert = vm.etat.value
        assertEquals("conv-canned", ouvert.conversationOuverte?.convId)
        assertEquals(1, ouvert.messages.size)
        assertEquals(MessageStatus.DELIVERED, ouvert.messages.first().status)

        vm.fermer()
        assertNull(vm.etat.value.conversationOuverte)
        assertTrue(vm.etat.value.messages.isEmpty())
    }

    @Test
    fun `ouvrir une conversation inconnue ne change rien`() {
        val (vm, _) = vmSurBouchon()
        vm.ouvrir("conv-inexistante")
        assertNull(vm.etat.value.conversationOuverte)
    }

    @Test
    fun `envoyer ajoute le message au fil ouvert avec son statut et vide la saisie`() {
        val (vm, _) = vmSurBouchon()
        vm.ouvrir("conv-canned")
        vm.modifierBrouillon("  Salut Alice  ")
        assertTrue(vm.etat.value.peutEnvoyer)

        vm.envoyer()

        val etat = vm.etat.value
        assertEquals("", etat.brouillon)
        assertEquals(2, etat.messages.size)
        val envoye = etat.messages.last()
        assertEquals("Salut Alice", envoye.body)
        assertTrue(envoye.outgoing)
        // Pair non connecté : le message attend dans l'outbox.
        assertEquals(MessageStatus.QUEUED, envoye.status)
        // Le dernier message de la conversation (liste) suit.
        assertEquals("Salut Alice", etat.conversations.single().lastMessage?.body)
        assertEquals(1, etat.conversations.size)
    }

    @Test
    fun `un pair connecte fait partir le message et le sondage rafraichit`() {
        val (vm, noeud) = vmSurBouchon()
        vm.ouvrir("conv-canned")

        assertFalse("rien à signaler au départ", vm.sonder())
        noeud.onPeerConnected("peer-canned")
        assertTrue("l'événement PeerConnected est traité", vm.sonder())

        vm.modifierBrouillon("tu es là ?")
        vm.envoyer()
        assertEquals(MessageStatus.IN_FLIGHT, vm.etat.value.messages.last().status)
    }

    @Test
    fun `un brouillon blanc ou aucune conversation ouverte n envoie rien`() {
        val (vm, noeud) = vmSurBouchon()

        vm.modifierBrouillon("hors fil")
        vm.envoyer()
        assertFalse(vm.etat.value.peutEnvoyer)

        vm.ouvrir("conv-canned")
        vm.modifierBrouillon("   ")
        assertFalse(vm.etat.value.peutEnvoyer)
        vm.envoyer()

        assertEquals(1, noeud.listMessages("conv-canned").size)
    }

    @Test
    fun `une erreur du noeud est affichee et le brouillon conserve`() {
        val conversation = Conversation("c1", "p1", "Bob", lastMessage = null, unreadCount = 0u)
        val noeudEnPanne = object : FauxNoeud() {
            override fun sendMessage(destPeerId: String, body: String): String =
                throw DengonException.NotConnected("pair hors de portée")
            override fun listConversations(): List<Conversation> = listOf(conversation)
        }
        val vm = ConversationsViewModel(noeudEnPanne)
        vm.ouvrir("c1")
        vm.modifierBrouillon("important")

        vm.envoyer()

        val etat = vm.etat.value
        assertNotNull(etat.erreur)
        assertTrue(etat.erreur!!.contains("pair hors de portée"))
        assertEquals("le texte n'est pas perdu", "important", etat.brouillon)

        vm.effacerErreur()
        assertNull(vm.etat.value.erreur)
    }

    @Test
    fun `chaque statut a son libelle`() {
        assertEquals(
            listOf("En attente", "Parti", "Distribué", "Lu", "Échec", "Annulé"),
            MessageStatus.values().map(::libelleStatut),
        )
    }

    open class NoeudAvecEchec : FauxNoeud() {
        val envois = mutableListOf<Pair<String, String>>()
        private val echec = Message("m-echec", "c1", "moi", "urgent", outgoing = true, sentMs = 0L, status = MessageStatus.EXPIRED)
        private val enCours = Message("m-cours", "c1", "moi", "patient", outgoing = true, sentMs = 1L, status = MessageStatus.QUEUED)

        override fun listConversations() = listOf(Conversation("c1", "peer-1", "Bob", enCours, 0u))
        override fun listMessages(convId: String) = listOf(echec, enCours)
        override fun sendMessage(destPeerId: String, body: String): String {
            envois += destPeerId to body
            return "m-neuf"
        }
    }

    @Test
    fun `seul un message sortant expire est en echec`() {
        val expire = Message("a", "c", "moi", "x", outgoing = true, sentMs = 0L, status = MessageStatus.EXPIRED)
        assertTrue(expire.enEchec)
        assertFalse(expire.copy(status = MessageStatus.QUEUED).enEchec)
        assertFalse("un message reçu n'est jamais « Échec »", expire.copy(outgoing = false).enEchec)
    }

    @Test
    fun `renvoyer reemet le texte d un message en echec au meme pair`() {
        val noeud = NoeudAvecEchec()
        var vides = 0
        val vm = ConversationsViewModel(noeud, apresEnvoi = { vides++ })
        vm.ouvrir("c1")

        vm.renvoyer("m-echec")

        assertEquals(listOf("peer-1" to "urgent"), noeud.envois)
        assertEquals("la radio est vidée aussitôt", 1, vides)
        assertNull(vm.etat.value.erreur)
    }

    @Test
    fun `renvoyer ignore un message qui n est pas en echec ou inconnu`() {
        val noeud = NoeudAvecEchec()
        val vm = ConversationsViewModel(noeud)
        vm.ouvrir("c1")

        vm.renvoyer("m-cours")
        vm.renvoyer("inconnu")

        assertTrue(noeud.envois.isEmpty())
    }

    @Test
    fun `un renvoi refuse par le noeud affiche l erreur`() {
        val noeud = object : NoeudAvecEchec() {
            override fun sendMessage(destPeerId: String, body: String): String =
                throw DengonException.UnknownPeer("pair inconnu")
        }
        val vm = ConversationsViewModel(noeud)
        vm.ouvrir("c1")

        vm.renvoyer("m-echec")

        assertTrue(vm.etat.value.erreur!!.startsWith("Renvoi impossible"))
    }

    @Test
    fun `la fabrique cree le ViewModel sur le noeud fourni`() {
        val noeud = FauxNoeud()
        val vm = ConversationsViewModel.fabrique(noeud).create(ConversationsViewModel::class.java)
        assertEquals(1, vm.etat.value.conversations.size)
    }
}
