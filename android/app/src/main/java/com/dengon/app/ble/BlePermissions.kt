package com.dengon.app.ble

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.ContextCompat

/**
 * Permissions BLE + notification requises pour faire tourner le service de
 * fond du maillage. Le jeu de permissions dépend de la version d'Android :
 * API 31+ (Android 12) introduit BLUETOOTH_SCAN/CONNECT/ADVERTISE et rend la
 * localisation optionnelle (`neverForLocation`, voir AndroidManifest.xml) ;
 * en dessous, seule ACCESS_FINE_LOCATION débloque le scan BLE.
 */
object BlePermissions {

    /** Permissions à demander à l'exécution, pour la version d'Android courante. */
    fun required(): Array<String> {
        val permissions = mutableListOf<String>()

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            permissions += Manifest.permission.BLUETOOTH_SCAN
            permissions += Manifest.permission.BLUETOOTH_CONNECT
            permissions += Manifest.permission.BLUETOOTH_ADVERTISE
        } else {
            permissions += Manifest.permission.ACCESS_FINE_LOCATION
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            permissions += Manifest.permission.POST_NOTIFICATIONS
        }

        return permissions.toTypedArray()
    }

    fun allGranted(context: Context): Boolean =
        required().all {
            ContextCompat.checkSelfPermission(context, it) == PackageManager.PERMISSION_GRANTED
        }
}
