package com.dengon.app.ui.conversations

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.dengon.app.ffi.Conversation
import com.dengon.app.ffi.Message
import com.dengon.app.ffi.MessageStatus
import kotlinx.coroutines.delay

/** Période d'interrogation du nœud (`pollEvents`), en ms. */
private const val PERIODE_SONDAGE_MS = 1_000L

/**
 * Point d'entrée des écrans de messagerie (US-214) : liste des
 * conversations, ou fil de la conversation ouverte.
 *
 * Le retour système ferme le fil, puis quitte la messagerie ([onQuitter]).
 */
@Composable
fun MessagerieRoute(viewModel: ConversationsViewModel, onQuitter: () -> Unit) {
    val etat by viewModel.etat.collectAsState()

    // Interrogation périodique du nœud, tant que l'écran est affiché.
    LaunchedEffect(viewModel) {
        while (true) {
            delay(PERIODE_SONDAGE_MS)
            viewModel.sonder()
        }
    }

    BackHandler {
        if (etat.conversationOuverte != null) viewModel.fermer() else onQuitter()
    }

    if (etat.conversationOuverte == null) {
        ListeConversations(
            conversations = etat.conversations,
            onOuvrir = viewModel::ouvrir,
            onQuitter = onQuitter,
        )
    } else {
        FilConversation(
            etat = etat,
            onRetour = viewModel::fermer,
            onBrouillon = viewModel::modifierBrouillon,
            onEnvoyer = viewModel::envoyer,
            onRenvoyer = viewModel::renvoyer,
        )
    }
}

@Composable
fun ListeConversations(
    conversations: List<Conversation>,
    onOuvrir: (String) -> Unit,
    onQuitter: () -> Unit,
) {
    Column(modifier = Modifier.fillMaxSize()) {
        EnTete(titre = "Conversations", onRetour = onQuitter)
        if (conversations.isEmpty()) {
            Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("Aucune conversation")
            }
            return
        }
        LazyColumn(modifier = Modifier.fillMaxSize()) {
            items(conversations, key = { it.convId }) { conversation ->
                LigneConversation(conversation = conversation, onClick = { onOuvrir(conversation.convId) })
            }
        }
    }
}

@Composable
private fun LigneConversation(conversation: Conversation, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(conversation.peerPseudo, style = MaterialTheme.typography.titleMedium)
            val dernier = conversation.lastMessage
            if (dernier != null) {
                val apercu = if (dernier.outgoing) "Vous : ${dernier.body}" else dernier.body
                Text(
                    apercu,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (dernier.outgoing) {
                    Text(libelleStatut(dernier.status), style = MaterialTheme.typography.labelSmall)
                }
            }
        }
        if (conversation.unreadCount > 0u) {
            Spacer(Modifier.padding(start = 8.dp))
            Surface(
                shape = RoundedCornerShape(50),
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.semantics { contentDescription = "${conversation.unreadCount} non lus" },
            ) {
                Text(
                    conversation.unreadCount.toString(),
                    color = MaterialTheme.colorScheme.onPrimary,
                    modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp),
                )
            }
        }
    }
}

@Composable
fun FilConversation(
    etat: ConversationsUiState,
    onRetour: () -> Unit,
    onBrouillon: (String) -> Unit,
    onEnvoyer: () -> Unit,
    onRenvoyer: (String) -> Unit = {},
) {
    val conversation = etat.conversationOuverte ?: return
    val liste = rememberLazyListState()

    // Toujours afficher le dernier message (après un envoi ou une réception).
    LaunchedEffect(etat.messages.size) {
        if (etat.messages.isNotEmpty()) liste.animateScrollToItem(etat.messages.size - 1)
    }

    Column(modifier = Modifier.fillMaxSize().imePadding()) {
        EnTete(titre = conversation.peerPseudo, onRetour = onRetour)
        LazyColumn(
            state = liste,
            modifier = Modifier.weight(1f).fillMaxWidth().padding(horizontal = 12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(etat.messages, key = { it.msgUuid }) { message ->
                BulleMessage(message, dejaRenvoye = message.msgUuid in etat.renvoyes, onRenvoyer = onRenvoyer)
            }
        }
        etat.erreur?.let { erreur ->
            Text(
                erreur,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
        }
        Row(
            modifier = Modifier.fillMaxWidth().padding(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(
                value = etat.brouillon,
                onValueChange = onBrouillon,
                modifier = Modifier.weight(1f),
                placeholder = { Text("Message") },
            )
            Spacer(Modifier.padding(start = 8.dp))
            Button(onClick = onEnvoyer, enabled = etat.peutEnvoyer) { Text("Envoyer") }
        }
    }
}

@Composable
private fun BulleMessage(message: Message, dejaRenvoye: Boolean, onRenvoyer: (String) -> Unit) {
    Box(
        modifier = Modifier.fillMaxWidth(),
        contentAlignment = if (message.outgoing) Alignment.CenterEnd else Alignment.CenterStart,
    ) {
        Surface(
            shape = RoundedCornerShape(12.dp),
            color = if (message.outgoing) {
                MaterialTheme.colorScheme.primaryContainer
            } else {
                MaterialTheme.colorScheme.surfaceVariant
            },
            modifier = Modifier.widthIn(max = 280.dp),
        ) {
            Column(modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp)) {
                Text(message.body)
                if (message.outgoing) {
                    Text(
                        libelleStatut(message.status),
                        style = MaterialTheme.typography.labelSmall,
                        color = if (message.enEchec) MaterialTheme.colorScheme.error else Color.Unspecified,
                        modifier = Modifier.align(Alignment.End),
                    )
                }
                if (message.enEchec && dejaRenvoye) {
                    Text("Renvoyé", style = MaterialTheme.typography.labelSmall, modifier = Modifier.align(Alignment.End))
                } else if (message.enEchec) {
                    TextButton(onClick = { onRenvoyer(message.msgUuid) }, modifier = Modifier.align(Alignment.End)) {
                        Text("Renvoyer")
                    }
                }
            }
        }
    }
}

@Composable
private fun EnTete(titre: String, onRetour: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TextButton(onClick = onRetour) { Text("← Retour") }
        Text(titre, style = MaterialTheme.typography.titleLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

// --- Aperçus (Android Studio) : données fixes. Pas de nœud ici : le vrai
// demande la bibliothèque native, absente du rendu d'aperçu. ---

private val apercuMessages = listOf(
    Message("m0", "c0", "alice", "Bienvenue sur dengon", outgoing = false, sentMs = 0L, status = MessageStatus.DELIVERED),
    Message("m1", "c0", "moi", "Salut !", outgoing = true, sentMs = 1L, status = MessageStatus.IN_FLIGHT),
)

private val apercuConversation =
    Conversation("c0", "alice", "Alice", lastMessage = apercuMessages.last(), unreadCount = 1u)

@Preview(showBackground = true, widthDp = 360)
@Composable
private fun ApercuListe() {
    MaterialTheme { ListeConversations(listOf(apercuConversation), onOuvrir = {}, onQuitter = {}) }
}

@Preview(showBackground = true, widthDp = 360, heightDp = 640)
@Composable
private fun ApercuFil() {
    val etat = ConversationsUiState(
        conversations = listOf(apercuConversation),
        conversationOuverte = apercuConversation,
        messages = apercuMessages,
    )
    MaterialTheme { FilConversation(etat, onRetour = {}, onBrouillon = {}, onEnvoyer = {}) }
}
