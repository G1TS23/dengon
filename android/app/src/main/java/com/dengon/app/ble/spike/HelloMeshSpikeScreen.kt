package com.dengon.app.ble.spike

import android.os.Build
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp

/**
 * Écran de debug du spike « hello mesh » (US-103). **Code jetable** (DoD
 * §7.2, type Spike) : à supprimer une fois la décision go/no-go actée dans
 * `docs/synthese/01-sujets-a-trancher.md` §A-1 — voir aussi
 * `docs/suivi/modules/android-app.md` pour le protocole de mesure manuelle.
 *
 * Un appareil choisit « Peripheral », l'autre « Central » ; le résultat
 * (MTU négocié, temps scan→connexion→échange) s'affiche après l'échange des
 * 20 octets. À relever manuellement (pas de transmission des résultats vers
 * un serveur — ce spike ne teste que la couche BLE, pas le dashboard).
 */
@Composable
fun HelloMeshSpikeScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    var role by remember { mutableStateOf<String?>(null) }
    val logLines = remember { mutableStateListOf<String>() }
    var result by remember { mutableStateOf<SpikeResult?>(null) }
    var peripheral by remember { mutableStateOf<HelloMeshPeripheral?>(null) }
    var central by remember { mutableStateOf<HelloMeshCentral?>(null) }

    fun stopAll() {
        peripheral?.stop()
        central?.stop()
        peripheral = null
        central = null
    }

    // Coupe proprement l'advertising/scan/GATT si l'utilisateur quitte l'écran
    // sans passer par « Arrêter » (retour système, changement de config…).
    DisposableEffect(Unit) {
        onDispose { stopAll() }
    }

    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Text("Spike C — hello mesh (US-103)", style = MaterialTheme.typography.titleLarge)
        Spacer(Modifier.height(4.dp))
        Text("Appareil : ${Build.MODEL} · Android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT})")
        Spacer(Modifier.height(16.dp))

        if (role == null) {
            Text("Choisir le rôle de CET appareil pour ce test :")
            Spacer(Modifier.height(8.dp))
            Row {
                Button(onClick = {
                    role = "peripheral"
                    logLines.clear()
                    result = null
                    peripheral = HelloMeshPeripheral(
                        context = context,
                        onLog = { logLines.add(it) },
                        onResult = { result = it },
                    ).also { it.start() }
                }) { Text("Peripheral") }

                Spacer(Modifier.width(8.dp))

                Button(onClick = {
                    role = "central"
                    logLines.clear()
                    result = null
                    central = HelloMeshCentral(
                        context = context,
                        onLog = { logLines.add(it) },
                        onResult = { result = it },
                    ).also { it.start() }
                }) { Text("Central") }
            }
        } else {
            Text("Rôle actif : $role")
            Spacer(Modifier.height(8.dp))
            Button(onClick = {
                stopAll()
                role = null
            }) { Text("Arrêter / réinitialiser") }
        }

        result?.let { r ->
            Spacer(Modifier.height(16.dp))
            Card(modifier = Modifier.fillMaxWidth()) {
                Column(Modifier.padding(12.dp)) {
                    Text("Résultat à consigner", style = MaterialTheme.typography.titleMedium)
                    Text("MTU négocié : ${r.mtu?.let { "$it octets" } ?: "n/a côté peripheral (voir note d'onboarding)"}")
                    Text("Scan → connexion : ${r.scanToConnectMs} ms")
                    Text("Connexion → échange : ${r.connectToExchangeMs} ms")
                    Text("Appareil : ${r.deviceModel} (Android ${r.androidVersion})")
                }
            }
        }

        Spacer(Modifier.height(16.dp))
        Text("Journal", style = MaterialTheme.typography.titleMedium)
        LazyColumn(modifier = Modifier.weight(1f)) {
            items(logLines) { line -> Text(line, style = MaterialTheme.typography.bodySmall) }
        }

        Spacer(Modifier.height(8.dp))
        Button(onClick = {
            stopAll()
            onBack()
        }) { Text("Retour") }
    }
}
