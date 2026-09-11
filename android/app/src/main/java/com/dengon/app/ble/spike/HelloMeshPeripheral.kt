package com.dengon.app.ble.spike

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothGattServerCallback
import android.bluetooth.BluetoothGattService
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.BluetoothLeAdvertiser
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.ParcelUuid

/**
 * Rôle peripheral du spike « hello mesh » (US-103) : publie le service
 * `SERVICE_UUID`, expose `CHAR_RX` (écriture, pair → nœud) et `CHAR_TX`
 * (notification, nœud → pair), conformément à
 * `docs/powl/03-network-protocol.md` §6.1.
 *
 * Simplification volontaire propre au spike : un seul rôle actif à la fois,
 * choisi manuellement dans l'UI. Pas de bascule dynamique par comparaison de
 * `peerID` (règle anti-boucle du §6.1) — `dengon-core` n'a pas encore
 * d'identité de nœud, ce sera fait par `AndroidTransport` (US-213).
 *
 * Code jetable (DoD §7.2, type Spike) : à supprimer une fois la décision
 * go/no-go actée dans `docs/synthese/01-sujets-a-trancher.md` §A-1.
 */
@SuppressLint("MissingPermission") // permissions runtime déjà vérifiées par l'appelant (BlePermissions.allGranted)
class HelloMeshPeripheral(
    private val context: Context,
    private val onLog: (String) -> Unit,
    private val onResult: (SpikeResult) -> Unit,
) {
    private val mainHandler = Handler(Looper.getMainLooper())
    private var gattServer: BluetoothGattServer? = null
    private var advertiser: BluetoothLeAdvertiser? = null
    private var txCharacteristicRef: BluetoothGattCharacteristic? = null
    private var startedAtMs = 0L
    private var connectedAtMs = 0L

    private fun log(message: String) = mainHandler.post { onLog(message) }

    fun start() {
        startedAtMs = System.currentTimeMillis()
        val bluetoothManager = context.getSystemService(BluetoothManager::class.java)

        val rxCharacteristic = BluetoothGattCharacteristic(
            HelloMeshConstants.CHAR_RX_UUID,
            BluetoothGattCharacteristic.PROPERTY_WRITE or BluetoothGattCharacteristic.PROPERTY_WRITE_NO_RESPONSE,
            BluetoothGattCharacteristic.PERMISSION_WRITE,
        )
        val txCharacteristic = BluetoothGattCharacteristic(
            HelloMeshConstants.CHAR_TX_UUID,
            BluetoothGattCharacteristic.PROPERTY_NOTIFY,
            BluetoothGattCharacteristic.PERMISSION_READ,
        ).apply {
            addDescriptor(
                BluetoothGattDescriptor(
                    HelloMeshConstants.CCCD_UUID,
                    BluetoothGattDescriptor.PERMISSION_READ or BluetoothGattDescriptor.PERMISSION_WRITE,
                ),
            )
        }

        val service = BluetoothGattService(
            HelloMeshConstants.SERVICE_UUID,
            BluetoothGattService.SERVICE_TYPE_PRIMARY,
        ).apply {
            addCharacteristic(rxCharacteristic)
            addCharacteristic(txCharacteristic)
        }

        gattServer = bluetoothManager.openGattServer(context, serverCallback)
        if (gattServer == null) {
            log("openGattServer a échoué (null) — arrêt du spike")
            return
        }
        gattServer?.addService(service)

        advertiser = bluetoothManager.adapter.bluetoothLeAdvertiser
        if (advertiser == null) {
            log("Pas d'advertiser BLE disponible sur cet appareil — arrêt du spike")
            return
        }

        val settings = AdvertiseSettings.Builder()
            .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY)
            .setConnectable(true)
            .setTimeout(0)
            .build()
        val data = AdvertiseData.Builder()
            .addServiceUuid(ParcelUuid(HelloMeshConstants.SERVICE_UUID))
            .setIncludeDeviceName(false)
            .build()

        advertiser?.startAdvertising(settings, data, advertiseCallback)
        log("Peripheral démarré : service publié, advertising en cours…")
    }

    fun stop() {
        advertiser?.stopAdvertising(advertiseCallback)
        gattServer?.close()
        gattServer = null
        advertiser = null
        txCharacteristicRef = null
    }

    private val advertiseCallback = object : AdvertiseCallback() {
        override fun onStartFailure(errorCode: Int) {
            log("Échec advertising, code=$errorCode")
        }
    }

    private val serverCallback = object : BluetoothGattServerCallback() {
        override fun onServiceAdded(status: Int, service: BluetoothGattService) {
            txCharacteristicRef = service.getCharacteristic(HelloMeshConstants.CHAR_TX_UUID)
            log("Service GATT enregistré (status=$status)")
        }

        override fun onConnectionStateChange(device: BluetoothDevice, status: Int, newState: Int) {
            if (newState == BluetoothProfile.STATE_CONNECTED) {
                connectedAtMs = System.currentTimeMillis()
                log("Central connecté : ${device.address} (t+${connectedAtMs - startedAtMs} ms)")
            }
        }

        override fun onMtuChanged(device: BluetoothDevice, mtu: Int) {
            log("MTU négocié côté peripheral : $mtu octets")
        }

        @Suppress("DEPRECATION") // characteristic.value : API dépréciée en 33+, gardée pour compat minSdk 26
        override fun onCharacteristicWriteRequest(
            device: BluetoothDevice,
            requestId: Int,
            characteristic: BluetoothGattCharacteristic,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray,
        ) {
            if (responseNeeded) {
                gattServer?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, offset, null)
            }
            val receivedAtMs = System.currentTimeMillis()
            log("Reçu ${value.size} octets sur CHAR_RX : ${String(value, Charsets.US_ASCII)}")

            // Écho immédiat sur CHAR_TX : sert au central à mesurer le round-trip.
            txCharacteristicRef?.let { tx ->
                tx.value = value
                gattServer?.notifyCharacteristicChanged(device, tx, false)
            }

            onResult(
                SpikeResult(
                    role = "peripheral",
                    // Pas d'API directe pour relire le MTU négocié hors du callback
                    // onMtuChanged côté serveur — voir note d'onboarding.
                    mtu = null,
                    scanToConnectMs = connectedAtMs - startedAtMs,
                    connectToExchangeMs = receivedAtMs - connectedAtMs,
                    deviceModel = Build.MODEL,
                    androidVersion = Build.VERSION.RELEASE,
                ),
            )
        }
    }
}
