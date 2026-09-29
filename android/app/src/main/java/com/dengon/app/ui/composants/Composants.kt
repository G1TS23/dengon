package com.dengon.app.ui.composants

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.dengon.app.ui.theme.CibleTactileMin

/**
 * Barre de titre commune : flèche de retour (cible 48 dp, lue « Retour » par
 * TalkBack) puis titre. Le titre passe à la ligne plutôt que d'être tronqué
 * quand le texte est agrandi.
 */
@Composable
fun EnTeteEcran(titre: String, onRetour: () -> Unit, modifier: Modifier = Modifier) {
    Row(
        modifier = modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onRetour, modifier = Modifier.size(CibleTactileMin)) {
            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Retour")
        }
        Text(
            titre,
            style = MaterialTheme.typography.titleLarge,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(start = 4.dp).semantics { heading() },
        )
    }
}

/**
 * État vide ou d'erreur guidé : ce qui se passe, pourquoi, et **quoi faire**
 * ([action], facultative). Défile si le texte agrandi dépasse l'écran.
 */
@Composable
fun EtatGuide(
    icone: ImageVector,
    titre: String,
    explication: String,
    modifier: Modifier = Modifier,
    libelleAction: String? = null,
    onAction: (() -> Unit)? = null,
) {
    Column(
        modifier = modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        // Décoratif : le titre dit déjà tout.
        Icon(
            icone,
            contentDescription = null,
            modifier = Modifier.size(56.dp),
            tint = MaterialTheme.colorScheme.primary,
        )
        Spacer(Modifier.size(16.dp))
        Text(titre, style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
        Spacer(Modifier.size(8.dp))
        Text(
            explication,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        if (libelleAction != null && onAction != null) {
            Spacer(Modifier.size(24.dp))
            Button(onClick = onAction, modifier = Modifier.heightIn(min = CibleTactileMin)) { Text(libelleAction) }
        }
    }
}

/** Bandeau d'avertissement (Bluetooth coupé, permission refusée…) avec action. */
@Composable
fun BandeauAlerte(
    icone: ImageVector,
    titre: String,
    explication: String,
    libelleAction: String,
    onAction: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Card(
        modifier = modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.errorContainer,
            contentColor = MaterialTheme.colorScheme.onErrorContainer,
        ),
    ) {
        Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(icone, contentDescription = null)
                Spacer(Modifier.width(12.dp))
                Text(titre, style = MaterialTheme.typography.titleMedium)
            }
            Text(explication, style = MaterialTheme.typography.bodyMedium)
            Button(onClick = onAction, modifier = Modifier.heightIn(min = CibleTactileMin)) { Text(libelleAction) }
        }
    }
}
