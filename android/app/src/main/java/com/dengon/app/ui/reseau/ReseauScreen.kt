package com.dengon.app.ui.reseau

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.dengon.app.ble.transport.TransportActif

/** Écran réseau (US-313) : pairs vus, relais atteints, mode éco. */
@Composable
fun ReseauRoute(onRetour: () -> Unit) {
    val etat by TransportActif.etat.collectAsState()
    ReseauScreen(
        vue = vueReseau(etat.demarre, etat.modeEco, etat.pairs, etat.pseudos),
        onModeEco = TransportActif::definirModeEco,
        onRetour = onRetour,
    )
}

@Composable
fun ReseauScreen(vue: VueReseau, onModeEco: (Boolean) -> Unit, onRetour: () -> Unit) {
    Column(
        modifier = Modifier.fillMaxSize().padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        TextButton(onClick = onRetour) { Text("← Retour") }
        Text("Réseau", style = MaterialTheme.typography.titleLarge)
        Text(if (vue.serviceDemarre) "Maillage actif" else "Maillage arrêté")

        Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Column(modifier = Modifier.weight(1f)) {
                Text("Mode éco")
                Text(
                    "Scan à cycle réduit : moins de batterie, pairs trouvés plus lentement",
                    style = MaterialTheme.typography.labelSmall,
                )
            }
            Switch(checked = vue.modeEco, onCheckedChange = onModeEco)
        }

        Section("Relais atteints (${vue.relais.size})", vue.relais)
        Section("Pairs vus (${vue.pairs.size})", vue.pairs)
    }
}

@Composable
private fun Section(titre: String, pairs: List<PairVu>) {
    Text(titre, style = MaterialTheme.typography.titleMedium)
    if (pairs.isEmpty()) Text("Aucun", style = MaterialTheme.typography.bodyMedium)
    pairs.forEach { Text("${it.pseudo ?: "?"} · ${it.peerId}", style = MaterialTheme.typography.bodyMedium) }
}
