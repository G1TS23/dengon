package com.dengon.app.ui.conversations

import com.dengon.app.ffi.Conversation
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Badge « non lus » (US-321) : le cœur ne remet jamais `unreadCount` à zéro
 * (pas de `mark_read` dans le contrat) ; le ViewModel retient le compteur vu
 * à l'ouverture et n'affiche que les messages reçus depuis.
 */
class NonLusTest {

    /** Nœud dont le compteur de la conversation canned est réglable. */
    private class NoeudCompteur(var nonLus: UInt) : FauxNoeud() {
        override fun listConversations(): List<Conversation> =
            super.listConversations().map { it.copy(unreadCount = nonLus) }
    }

    private fun conversation(vm: ConversationsViewModel) = vm.etat.value.conversations.single()

    @Test
    fun `avant ouverture tous les messages recus sont non lus`() {
        val vm = ConversationsViewModel(NoeudCompteur(2u))
        assertEquals(2, vm.etat.value.nonLus(conversation(vm)))
    }

    @Test
    fun `ouvrir la conversation remet le badge a zero`() {
        val vm = ConversationsViewModel(NoeudCompteur(2u))
        vm.ouvrir(conversation(vm).convId)
        vm.fermer()
        assertEquals(0, vm.etat.value.nonLus(conversation(vm)))
    }

    @Test
    fun `un message recu apres la fermeture redonne un non lu`() {
        val noeud = NoeudCompteur(2u)
        val vm = ConversationsViewModel(noeud)
        vm.ouvrir(conversation(vm).convId)
        vm.fermer()

        noeud.nonLus = 3u
        vm.rafraichir()
        assertEquals(1, vm.etat.value.nonLus(conversation(vm)))
    }

    @Test
    fun `un message recu pendant que le fil est ouvert est deja lu`() {
        val noeud = NoeudCompteur(2u)
        val vm = ConversationsViewModel(noeud)
        vm.ouvrir(conversation(vm).convId)

        noeud.nonLus = 3u
        vm.rafraichir()
        vm.fermer()
        assertEquals(0, vm.etat.value.nonLus(conversation(vm)))
    }

    @Test
    fun `le compteur ne devient jamais negatif`() {
        val noeud = NoeudCompteur(5u)
        val vm = ConversationsViewModel(noeud)
        vm.ouvrir(conversation(vm).convId)
        vm.fermer()

        noeud.nonLus = 1u // ne devrait pas arriver, mais on ne plante pas
        vm.rafraichir()
        assertEquals(0, vm.etat.value.nonLus(conversation(vm)))
    }
}
