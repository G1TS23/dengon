package com.dengon.app.ui.conversations

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import com.dengon.app.ffi.Conversation
import com.dengon.app.ffi.DengonException
import com.dengon.app.ffi.DengonNodeInterface
import com.dengon.app.ffi.Message
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/** Tout ce que les écrans « conversations » et « fil » affichent. */
data class ConversationsUiState(
    val conversations: List<Conversation> = emptyList(),
    /** Conversation ouverte (fil affiché), `null` = liste. */
    val conversationOuverte: Conversation? = null,
    /** Messages de la conversation ouverte, dans l'ordre d'envoi. */
    val messages: List<Message> = emptyList(),
    /** Texte en cours de saisie dans le fil. */
    val brouillon: String = "",
    /** Dernière erreur à afficher (envoi refusé…), `null` si aucune. */
    val erreur: String? = null,
) {
    /** Le bouton « Envoyer » n'est actif qu'avec un texte non blanc. */
    val peutEnvoyer: Boolean get() = conversationOuverte != null && brouillon.isNotBlank()
}

/**
 * ViewModel des écrans de messagerie (US-214).
 *
 * Alimenté **exclusivement** par [DengonNodeInterface] : dans l'app, le
 * `DengonNode` généré par UniFFI sur le vrai `dengon-core` (US-302) ; dans
 * les tests, un faux nœud en mémoire.
 *
 * Toutes les méthodes sont **synchrones** : le nœud Rust tient ses
 * conversations en mémoire et répond sans E/S, et les tests unitaires lisent
 * [etat] directement, sans dispatcher de test.
 * L'interrogation périodique du nœud ([sonder]) est cadencée par l'écran.
 *
 * TODO(US-306) : chaque appel prend le `Mutex` du nœud Rust, qui sera
 * partagé avec le service de premier plan (radio : `on_bytes_received`,
 * `on_peer_connected`…). Un appel de l'UI pourra alors attendre que le
 * service relâche ce verrou : passer ces appels dans `viewModelScope` sur un
 * dispatcher d'E/S, pour ne jamais bloquer le thread principal.
 */
class ConversationsViewModel(private val noeud: DengonNodeInterface) : ViewModel() {

    private val _etat = MutableStateFlow(ConversationsUiState())

    /** État observé par l'UI. */
    val etat: StateFlow<ConversationsUiState> = _etat.asStateFlow()

    init {
        rafraichir()
    }

    /** Relit conversations et messages depuis le nœud. */
    fun rafraichir() {
        val conversations = noeud.listConversations()
        _etat.update { e ->
            // La conversation ouverte est relue, pour afficher son dernier état
            // (dernier message, compteur) et non une copie figée.
            val ouverte = e.conversationOuverte?.let { o -> conversations.firstOrNull { it.convId == o.convId } ?: o }
            e.copy(
                conversations = conversations,
                conversationOuverte = ouverte,
                messages = ouverte?.let { noeud.listMessages(it.convId) } ?: emptyList(),
            )
        }
    }

    /** Ouvre le fil d'une conversation. Sans effet si elle n'existe pas. */
    fun ouvrir(convId: String) {
        val conversation = _etat.value.conversations.firstOrNull { it.convId == convId } ?: return
        _etat.update {
            it.copy(
                conversationOuverte = conversation,
                messages = noeud.listMessages(convId),
                brouillon = "",
                erreur = null,
            )
        }
    }

    /** Revient à la liste des conversations. */
    fun fermer() {
        _etat.update { it.copy(conversationOuverte = null, messages = emptyList(), brouillon = "", erreur = null) }
    }

    /** Met à jour le texte en cours de saisie. */
    fun modifierBrouillon(texte: String) {
        _etat.update { it.copy(brouillon = texte) }
    }

    /**
     * Envoie le brouillon au pair de la conversation ouverte.
     *
     * Sans effet si rien n'est ouvert ou si le brouillon est blanc. En cas
     * d'erreur du nœud, le brouillon est **conservé** (l'utilisateur ne perd
     * pas son texte) et [ConversationsUiState.erreur] est renseigné.
     */
    fun envoyer() {
        val e = _etat.value
        val conversation = e.conversationOuverte ?: return
        val texte = e.brouillon.trim()
        if (texte.isEmpty()) return

        try {
            noeud.sendMessage(conversation.peerId, texte)
        } catch (err: DengonException) {
            _etat.update { it.copy(erreur = "Envoi impossible : ${err.message ?: err::class.simpleName}") }
            return
        }
        _etat.update { it.copy(brouillon = "", erreur = null) }
        rafraichir()
    }

    /** Efface l'erreur affichée. */
    fun effacerErreur() {
        _etat.update { it.copy(erreur = null) }
    }

    /**
     * Interroge le nœud (`pollEvents`) et rafraîchit s'il s'est passé
     * quelque chose (message reçu, statut changé, pair connecté…).
     *
     * @return `true` si des événements ont été traités.
     */
    fun sonder(): Boolean {
        val evenements = noeud.pollEvents()
        if (evenements.isEmpty()) return false
        rafraichir()
        return true
    }

    companion object {
        /** Fabrique pour `by viewModels { … }` : injecte le nœud. */
        fun fabrique(noeud: DengonNodeInterface): ViewModelProvider.Factory =
            object : ViewModelProvider.Factory {
                @Suppress("UNCHECKED_CAST")
                override fun <T : ViewModel> create(modelClass: Class<T>): T =
                    ConversationsViewModel(noeud) as T
            }
    }
}
