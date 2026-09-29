package com.dengon.app.ble.transport

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Écran de debug du transport (US-213) : état des liens, pairs identifiés
 * par `ANNOUNCE` (US-306), journal. Outil d'essai sur deux téléphones, pas
 * un écran produit.
 *
 * Les envois d'essai de l'US-213 (message court, grande trame, battement)
 * ont été retirés à l'US-306 : chaque trame reçue va désormais au nœud, qui
 * jetterait ces octets bruts. Le lien se teste maintenant par la messagerie.
 */
@Composable
fun TransportDebugScreen(onRetour: () -> Unit) {
    val etat by TransportActif.etat.collectAsState()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = onRetour) { Text("Retour") }
            Text("Transport BLE (debug)", style = MaterialTheme.typography.titleLarge)
        }
        Text(
            if (etat.demarre) "Démarré — peerID ${etat.peerIdLocal}" else "Arrêté${etat.erreur?.let { " : $it" } ?: ""}",
        )
        Text("Liens ouverts : ${etat.liens.size} ${etat.liens.joinToString(prefix = "(", postfix = ")")}")
        Text("Pairs identifiés : ${etat.pairs.size}")
        for ((lien, pair) in etat.pairs) {
            Text("  $lien ↔ $pair", fontFamily = FontFamily.Monospace, fontSize = 12.sp)
        }
        Text("Trames reçues : ${etat.trames}")
        Row(verticalAlignment = Alignment.CenterVertically) {
            Switch(
                checked = etat.ignorerLiensDirects,
                onCheckedChange = TransportActif::ignorerLiensDirects,
            )
            Text("  Relais seulement (ignorer les liens directs)")
        }

        Text("Journal", style = MaterialTheme.typography.titleSmall)
        LazyColumn {
            items(etat.journal) { ligne ->
                Text(ligne, fontFamily = FontFamily.Monospace, fontSize = 12.sp)
            }
        }
    }
}
