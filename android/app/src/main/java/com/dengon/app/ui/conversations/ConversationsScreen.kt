package com.dengon.app.ui.conversations

import android.content.res.Configuration.UI_MODE_NIGHT_YES
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.MailOutline
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
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
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.dengon.app.ffi.Conversation
import com.dengon.app.ffi.Message
import com.dengon.app.ffi.MessageStatus
import com.dengon.app.ui.composants.EnTeteEcran
import com.dengon.app.ui.composants.EtatGuide
import com.dengon.app.ui.theme.CibleTactileMin
import com.dengon.app.ui.theme.DengonTheme
import kotlinx.coroutines.delay

/** Période d'interrogation du nœud (`pollEvents`), en ms. */
private const val PERIODE_SONDAGE_MS = 1_000L

/**
 * Point d'entrée des écrans de messagerie (US-214) : liste des
 * conversations, ou fil de la conversation ouverte.
 *
 * Le retour système ferme le fil, puis quitte la messagerie ([onQuitter]).
 * [onAjouterContact] mène à l'appairage depuis l'état « aucune conversation ».
 */
@Composable
fun MessagerieRoute(
    viewModel: ConversationsViewModel,
    onQuitter: () -> Unit,
    onAjouterContact: () -> Unit = {},
) {
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
            onAjouterContact = onAjouterContact,
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
    onAjouterContact: () -> Unit = {},
) {
    Column(modifier = Modifier.fillMaxSize()) {
        EnTeteEcran(titre = "Conversations", onRetour = onQuitter)
        if (conversations.isEmpty()) {
            EtatGuide(
                icone = Icons.Filled.MailOutline,
                titre = "Aucune conversation",
                explication = "Pour écrire à quelqu'un, ajoutez-le d'abord comme contact : " +
                    "vous vous montrez chacun un code à scanner, en personne.",
                libelleAction = "Ajouter un contact",
                onAction = onAjouterContact,
            )
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
    val dernier = conversation.lastMessage
    val nonLus = conversation.unreadCount.toInt()
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .clickable(onClickLabel = "Ouvrir la conversation", onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Pastille(conversation.peerPseudo)
        Spacer(Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                conversation.peerPseudo,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = if (nonLus > 0) FontWeight.Bold else FontWeight.Medium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (dernier != null) {
                val apercu = if (dernier.outgoing) "Vous : ${dernier.body}" else dernier.body
                Text(
                    apercu,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                if (dernier.outgoing) StatutMessage(dernier)
            } else {
                Text(
                    "Aucun message pour l'instant",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Column(horizontalAlignment = Alignment.End) {
            if (dernier != null) {
                Text(
                    formaterHorodatage(dernier.sentMs),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (nonLus > 0) {
                Spacer(Modifier.height(4.dp))
                Surface(
                    shape = CircleShape,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.semantics {
                        contentDescription = if (nonLus == 1) "1 message non lu" else "$nonLus messages non lus"
                    },
                ) {
                    Text(
                        nonLus.toString(),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onPrimary,
                        modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp),
                    )
                }
            }
        }
    }
}

/** Pastille ronde avec l'initiale du correspondant (décorative). */
@Composable
private fun Pastille(pseudo: String) {
    Box(
        modifier = Modifier.size(48.dp).background(MaterialTheme.colorScheme.primaryContainer, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            initiale(pseudo),
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onPrimaryContainer,
        )
    }
}

/**
 * Statut d'un message sortant : pictogramme **et** texte (la couleur seule ne
 * suffit pas : inaccessible aux daltoniens).
 */
@Composable
private fun StatutMessage(message: Message, modifier: Modifier = Modifier) {
    val couleur = if (message.enEchec) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant
    Row(modifier = modifier, verticalAlignment = Alignment.CenterVertically) {
        Icon(
            when (iconeStatut(message.status)) {
                IconeStatut.EN_COURS -> Icons.Filled.Refresh
                IconeStatut.ENVOYE -> Icons.Filled.Check
                IconeStatut.DISTRIBUE -> Icons.Filled.CheckCircle
                IconeStatut.ECHEC -> Icons.Filled.Warning
                IconeStatut.ANNULE -> Icons.Filled.Close
            },
            contentDescription = null,
            tint = couleur,
            modifier = Modifier.size(14.dp),
        )
        Spacer(Modifier.width(4.dp))
        Text(libelleStatut(message.status), style = MaterialTheme.typography.labelSmall, color = couleur)
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
        EnTeteEcran(titre = conversation.peerPseudo, onRetour = onRetour)
        if (etat.messages.isEmpty()) {
            Box(modifier = Modifier.weight(1f).fillMaxWidth().padding(32.dp), contentAlignment = Alignment.Center) {
                Text(
                    "Aucun message. Écrivez le premier ci-dessous : il sera transmis dès que " +
                        "${conversation.peerPseudo} ou un relais sera à portée.",
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                )
            }
        } else {
            LazyColumn(
                state = liste,
                modifier = Modifier.weight(1f).fillMaxWidth().padding(horizontal = 12.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(etat.messages, key = { it.msgUuid }) { message ->
                    BulleMessage(message, dejaRenvoye = message.msgUuid in etat.renvoyes, onRenvoyer = onRenvoyer)
                }
            }
        }
        etat.erreur?.let { erreur ->
            Text(
                erreur,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier
                    .padding(horizontal = 16.dp, vertical = 4.dp)
                    .semantics { liveRegion = LiveRegionMode.Polite },
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
                placeholder = { Text("Écrire un message") },
                maxLines = 4,
                shape = MaterialTheme.shapes.large,
            )
            Spacer(Modifier.width(8.dp))
            FilledIconButton(
                onClick = onEnvoyer,
                enabled = etat.peutEnvoyer,
                modifier = Modifier.size(CibleTactileMin),
            ) {
                Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Envoyer")
            }
        }
    }
}

@Composable
private fun BulleMessage(message: Message, dejaRenvoye: Boolean, onRenvoyer: (String) -> Unit) {
    val sortant = message.outgoing
    Box(
        modifier = Modifier.fillMaxWidth(),
        contentAlignment = if (sortant) Alignment.CenterEnd else Alignment.CenterStart,
    ) {
        Surface(
            // Coin « queue » côté expéditeur : on distingue d'un coup d'œil le sens.
            shape = RoundedCornerShape(
                topStart = 16.dp,
                topEnd = 16.dp,
                bottomStart = if (sortant) 16.dp else 4.dp,
                bottomEnd = if (sortant) 4.dp else 16.dp,
            ),
            color = if (sortant) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceVariant,
            contentColor = if (sortant) {
                MaterialTheme.colorScheme.onPrimaryContainer
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
            modifier = Modifier.widthIn(max = 300.dp),
        ) {
            Column(modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp)) {
                Text(message.body, style = MaterialTheme.typography.bodyLarge)
                Row(
                    modifier = Modifier.align(Alignment.End),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    val heure = formaterHorodatage(message.sentMs)
                    if (heure.isNotEmpty()) Text(heure, style = MaterialTheme.typography.labelSmall)
                    if (sortant) StatutMessage(message)
                }
                if (message.enEchec && dejaRenvoye) {
                    Text(
                        "Renvoyé",
                        style = MaterialTheme.typography.labelSmall,
                        modifier = Modifier.align(Alignment.End).padding(top = 4.dp),
                    )
                } else if (message.enEchec) {
                    TextButton(
                        onClick = { onRenvoyer(message.msgUuid) },
                        modifier = Modifier.align(Alignment.End).heightIn(min = CibleTactileMin),
                    ) {
                        Text("Renvoyer")
                    }
                }
            }
        }
    }
}

// --- Aperçus (Android Studio) : données fixes. Pas de nœud ici : le vrai
// demande la bibliothèque native, absente du rendu d'aperçu. ---

private val apercuMessages = listOf(
    Message("m0", "c0", "alice", "Bienvenue sur dengon", outgoing = false, sentMs = 1_700_000_000_000L, status = MessageStatus.DELIVERED),
    Message("m1", "c0", "moi", "Salut !", outgoing = true, sentMs = 1_700_000_060_000L, status = MessageStatus.IN_FLIGHT),
)

private val apercuConversation =
    Conversation("c0", "alice", "Alice", lastMessage = apercuMessages.last(), unreadCount = 1u)

@Preview(showBackground = true, widthDp = 360)
@Composable
private fun ApercuListe() {
    DengonTheme { Surface { ListeConversations(listOf(apercuConversation), onOuvrir = {}, onQuitter = {}) } }
}

@Preview(showBackground = true, widthDp = 360, name = "Liste sombre", uiMode = UI_MODE_NIGHT_YES)
@Composable
private fun ApercuListeSombre() {
    DengonTheme(sombre = true) { Surface { ListeConversations(listOf(apercuConversation), onOuvrir = {}, onQuitter = {}) } }
}

@Preview(showBackground = true, widthDp = 360, heightDp = 480, name = "Liste vide")
@Composable
private fun ApercuListeVide() {
    DengonTheme { Surface { ListeConversations(emptyList(), onOuvrir = {}, onQuitter = {}) } }
}

@Preview(showBackground = true, widthDp = 360, heightDp = 640)
@Composable
private fun ApercuFil() {
    val etat = ConversationsUiState(
        conversations = listOf(apercuConversation),
        conversationOuverte = apercuConversation,
        messages = apercuMessages,
    )
    DengonTheme { Surface { FilConversation(etat, onRetour = {}, onBrouillon = {}, onEnvoyer = {}) } }
}

@Preview(showBackground = true, widthDp = 360, heightDp = 640, fontScale = 2f, name = "Fil, texte 200 %")
@Composable
private fun ApercuFilGrandePolice() {
    val echec = Message("m2", "c0", "moi", "Es-tu là ?", outgoing = true, sentMs = 1_700_000_120_000L, status = MessageStatus.EXPIRED)
    val etat = ConversationsUiState(
        conversations = listOf(apercuConversation),
        conversationOuverte = apercuConversation,
        messages = apercuMessages + echec,
    )
    DengonTheme { Surface { FilConversation(etat, onRetour = {}, onBrouillon = {}, onEnvoyer = {}) } }
}
