package com.dengon.app.ble.transport

import android.os.Build
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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

/** Taille de la grande trame d'essai : ~10 écritures GATT de 512 o (fragmentation BLE). */
private const val GRANDE_TRAME = 5_000

/**
 * Écran de debug du transport (US-213) : état du lien, envois d'essai,
 * journal. Outil d'essai sur deux téléphones, pas un écran produit.
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
        Text("Trames reçues : ${etat.trames}")

        Button(
            onClick = { TransportActif.diffuser("bonjour de ${Build.MODEL}".toByteArray(), "message court") },
            modifier = Modifier.fillMaxWidth(),
        ) { Text("Diffuser un message court") }
        Button(
            onClick = {
                val trame = ByteArray(GRANDE_TRAME) { (it % 251).toByte() }
                TransportActif.diffuser(trame, "grande trame")
            },
            modifier = Modifier.fillMaxWidth(),
        ) { Text("Diffuser $GRANDE_TRAME octets") }
        OutlinedButton(onClick = TransportActif::basculerBattement, modifier = Modifier.fillMaxWidth()) {
            Text(if (etat.battement) "Couper le battement (30 s)" else "Battement toutes les 30 s")
        }

        Text("Journal", style = MaterialTheme.typography.titleSmall)
        LazyColumn {
            items(etat.journal) { ligne ->
                Text(ligne, fontFamily = FontFamily.Monospace, fontSize = 12.sp)
            }
        }
    }
}
