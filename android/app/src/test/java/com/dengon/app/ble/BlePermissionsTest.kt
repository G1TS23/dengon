package com.dengon.app.ble

import android.Manifest
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Test unitaire minimal (US-109) : vérifie que les permissions demandées
 * couvrent bien le triplet BLE Android 12+ (scan/connect/advertise).
 *
 * Ne dépend pas d'Android runtime (pas de `Build.VERSION.SDK_INT` réel en JVM
 * pur) : on vérifie uniquement l'ensemble déclaré, pas le branchement par
 * version, qui demande un test instrumenté (hors périmètre de ce squelette).
 */
class BlePermissionsTest {

    @Test
    fun `required always includes the core BLE or location permission`() {
        val permissions = BlePermissions.required().toSet()

        val hasModernBle = permissions.containsAll(
            setOf(
                Manifest.permission.BLUETOOTH_SCAN,
                Manifest.permission.BLUETOOTH_CONNECT,
                Manifest.permission.BLUETOOTH_ADVERTISE,
            ),
        )
        val hasLegacyLocation = permissions.contains(Manifest.permission.ACCESS_FINE_LOCATION)

        assertTrue(
            "attendu : soit le triplet BLE moderne, soit la localisation legacy",
            hasModernBle || hasLegacyLocation,
        )
    }
}
