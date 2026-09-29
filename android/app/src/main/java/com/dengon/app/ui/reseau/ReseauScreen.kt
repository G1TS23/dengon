package com.dengon.app.ui.reseau

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.selection.toggleable
import com.dengon.app.ble.transport.TransportActif
import com.dengon.app.ui.composants.BandeauAlerte
import com.dengon.app.ui.composants.EnTeteEcran
import com.dengon.app.ui.composants.ouvrirReglagesBluetooth
import com.dengon.app.ui.composants.rememberBluetoothActif
import com.dengon.app.ui.theme.CibleTactileMin
import com.dengon.app.ui.theme.DengonTheme

/** Écran « Appareils à proximité » (US-313, refonte US-321) : téléphones et relais joignables, mode éco. */
@Composable
fun ReseauRoute(onRetour: () -> Unit) {
    val etat by TransportActif.etat.collectAsState()
    val contexte = LocalContext.current
    ReseauScreen(
        vue = vueReseau(etat.demarre, etat.modeEco, etat.pairs, etat.pseudos),
        bluetoothActif = rememberBluetoothActif(),
        onModeEco = TransportActif::definirModeEco,
        onReglagesBluetooth = { ouvrirReglagesBluetooth(contexte) },
        onRetour = onRetour,
    )
}

@Composable
fun ReseauScreen(
    vue: VueReseau,
    bluetoothActif: Boolean,
    onModeEco: (Boolean) -> Unit,
    onRetour: () -> Unit,
    onReglagesBluetooth: () -> Unit = {},
) {
    Column(modifier = Modifier.fillMaxSize()) {
        EnTeteEcran(titre = "Appareils à proximité", onRetour = onRetour)
        Column(
            modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            if (!bluetoothActif) {
                BandeauAlerte(
                    icone = Icons.Filled.Warning,
                    titre = "Bluetooth coupé",
                    explication = "Sans Bluetooth, dengon ne peut ni trouver d'appareils ni transmettre vos messages.",
                    libelleAction = "Ouvrir les réglages Bluetooth",
                    onAction = onReglagesBluetooth,
                )
            }

            CarteEtat(vue.serviceDemarre && bluetoothActif)

            ModeEco(vue.modeEco, onModeEco)

            Section(
                titre = "Relais",
                pairs = vue.relais,
                vide = "Aucun relais à portée. Un relais est un petit boîtier qui fait suivre vos messages plus loin.",
            )
            Section(
                titre = "Téléphones",
                pairs = vue.pairs,
                vide = "Aucun téléphone à proximité. Rapprochez-vous d'un contact qui a ouvert dengon.",
            )
        }
    }
}

@Composable
private fun CarteEtat(actif: Boolean) {
    Card(
        colors = CardDefaults.cardColors(
            containerColor = if (actif) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceVariant,
            contentColor = if (actif) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurfaceVariant,
        ),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(modifier = Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(if (actif) Icons.Filled.CheckCircle else Icons.Filled.Info, contentDescription = null)
            Spacer(Modifier.width(12.dp))
            Column {
                Text(
                    if (actif) "Recherche en cours" else "Recherche à l'arrêt",
                    style = MaterialTheme.typography.titleMedium,
                )
                Text(
                    if (actif) {
                        "Votre téléphone cherche des appareils autour de vous."
                    } else {
                        "Activez le Bluetooth et autorisez dengon pour retrouver vos contacts."
                    },
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
    }
}

@Composable
private fun ModeEco(actif: Boolean, onChange: (Boolean) -> Unit) {
    // Toute la ligne est cliquable (pas seulement l'interrupteur) : cible ≥ 48 dp.
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = CibleTactileMin)
            .toggleable(value = actif, role = Role.Switch, onValueChange = onChange)
            .semantics { stateDescription = if (actif) "Activé" else "Désactivé" },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text("Économie de batterie", style = MaterialTheme.typography.titleMedium)
            Text(
                "Cherche moins souvent : la batterie dure plus longtemps, mais les appareils sont trouvés plus lentement.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(Modifier.width(16.dp))
        // Le clic est géré par la ligne : l'interrupteur est purement visuel.
        Switch(checked = actif, onCheckedChange = null)
    }
}

@Composable
private fun Section(titre: String, pairs: List<PairVu>, vide: String) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("$titre (${pairs.size})", style = MaterialTheme.typography.titleMedium)
        if (pairs.isEmpty()) {
            Text(vide, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        pairs.forEach { LignePair(it) }
    }
}

@Composable
private fun LignePair(pair: PairVu) {
    Surface(
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.surfaceVariant,
        contentColor = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
            Text(pair.pseudo ?: "Appareil inconnu", style = MaterialTheme.typography.titleSmall)
            // Identifiant technique : utile au dépannage, écrit en petit.
            Text("Identifiant : ${pair.peerId}", style = MaterialTheme.typography.labelSmall)
        }
    }
}

private val apercuVue = VueReseau(
    serviceDemarre = true,
    modeEco = false,
    pairs = listOf(PairVu("a1b2c3d4", "alice", false)),
    relais = listOf(PairVu("ffee0011", "relais-4f2a", true)),
)

@Preview(showBackground = true, widthDp = 360, heightDp = 720)
@Composable
private fun ApercuReseau() {
    DengonTheme { Surface { ReseauScreen(apercuVue, bluetoothActif = true, onModeEco = {}, onRetour = {}) } }
}

@Preview(showBackground = true, widthDp = 360, heightDp = 720, name = "Bluetooth coupé, aucun appareil")
@Composable
private fun ApercuReseauVide() {
    DengonTheme {
        Surface {
            ReseauScreen(
                apercuVue.copy(pairs = emptyList(), relais = emptyList()),
                bluetoothActif = false,
                onModeEco = {},
                onRetour = {},
            )
        }
    }
}
