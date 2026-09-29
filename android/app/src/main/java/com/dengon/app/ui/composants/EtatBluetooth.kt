package com.dengon.app.ui.composants

import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.provider.Settings
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat

/**
 * `true` tant que le Bluetooth du téléphone est allumé, mis à jour en direct
 * (US-321 : l'utilisateur doit voir « Bluetooth coupé » sans relancer l'app).
 * Lire `isEnabled` ne demande aucune permission.
 */
@Composable
fun rememberBluetoothActif(): Boolean {
    val contexte = LocalContext.current
    var actif by remember { mutableStateOf(bluetoothAllume(contexte)) }
    DisposableEffect(contexte) {
        val recepteur = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                actif = bluetoothAllume(context)
            }
        }
        ContextCompat.registerReceiver(
            contexte,
            recepteur,
            IntentFilter(BluetoothAdapter.ACTION_STATE_CHANGED),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        actif = bluetoothAllume(contexte)
        onDispose { contexte.unregisterReceiver(recepteur) }
    }
    return actif
}

private fun bluetoothAllume(contexte: Context): Boolean =
    contexte.getSystemService(BluetoothManager::class.java)?.adapter?.isEnabled == true

/** Ouvre les réglages Bluetooth du système (sans permission particulière). */
fun ouvrirReglagesBluetooth(contexte: Context) {
    contexte.startActivity(Intent(Settings.ACTION_BLUETOOTH_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
}
