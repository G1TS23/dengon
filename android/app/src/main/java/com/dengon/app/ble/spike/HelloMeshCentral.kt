package com.dengon.app.ble.spike

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.le.BluetoothLeScanner
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.ParcelUuid

/**
 * Rôle central du spike « hello mesh » (US-103) : scanne `SERVICE_UUID`, se
 * connecte, négocie le MTU, écrit 20 octets sur `CHAR_RX` et mesure l'écho
 * reçu sur `CHAR_TX` — `docs/powl/03-network-protocol.md` §6.1 et §6.3.
 *
 * Mêmes simplifications que [HelloMeshPeripheral] : rôle choisi manuellement,
 * code jetable (DoD §7.2, type Spike).
 */
@SuppressLint("MissingPermission") // permissions runtime déjà vérifiées par l'appelant (BlePermissions.allGranted)
class HelloMeshCentral(
    private val context: Context,
    private val onLog: (String) -> Unit,
    private val onResult: (SpikeResult) -> Unit,
) {
    private val mainHandler = Handler(Looper.getMainLooper())
    private var scanner: BluetoothLeScanner? = null
    private var gatt: BluetoothGatt? = null
    private var scanStartedAtMs = 0L
    private var connectedAtMs = 0L
    private var writeStartedAtMs = 0L
    private var negotiatedMtu: Int? = null

    private fun log(message: String) = mainHandler.post { onLog(message) }

    fun start() {
        scanStartedAtMs = System.currentTimeMillis()
        val bluetoothManager = context.getSystemService(BluetoothManager::class.java)
        scanner = bluetoothManager.adapter.bluetoothLeScanner
        if (scanner == null) {
            log("Pas de scanner BLE disponible sur cet appareil — arrêt du spike")
            return
        }

        val filter = ScanFilter.Builder()
            .setServiceUuid(ParcelUuid(HelloMeshConstants.SERVICE_UUID))
            .build()
        val settings = ScanSettings.Builder()
            .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
            .build()

        log("Scan démarré (filtre SERVICE_UUID)…")
        scanner?.startScan(listOf(filter), settings, scanCallback)
    }

    fun stop() {
        scanner?.stopScan(scanCallback)
        gatt?.close()
        gatt = null
    }

    private val scanCallback = object : ScanCallback() {
        override fun onScanResult(callbackType: Int, result: ScanResult) {
            // On ne garde que le premier pair trouvé : un seul échange pour ce spike.
            scanner?.stopScan(this)
            val foundAtMs = System.currentTimeMillis()
            log("Peer trouvé : ${result.device.address} (t+${foundAtMs - scanStartedAtMs} ms)")
            gatt = result.device.connectGatt(context, false, gattCallback, BluetoothDevice.TRANSPORT_LE)
        }

        override fun onScanFailed(errorCode: Int) {
            log("Échec scan, code=$errorCode")
        }
    }

    @Suppress("DEPRECATION") // characteristic.value / onCharacteristicChanged(2-arg) : API 33+, gardé pour minSdk 26
    private val gattCallback = object : BluetoothGattCallback() {
        override fun onConnectionStateChange(g: BluetoothGatt, status: Int, newState: Int) {
            if (newState == BluetoothProfile.STATE_CONNECTED) {
                connectedAtMs = System.currentTimeMillis()
                log(
                    "Connecté (t+${connectedAtMs - scanStartedAtMs} ms) — " +
                        "demande MTU=${HelloMeshConstants.REQUESTED_MTU}",
                )
                g.requestMtu(HelloMeshConstants.REQUESTED_MTU)
            }
        }

        override fun onMtuChanged(g: BluetoothGatt, mtu: Int, status: Int) {
            negotiatedMtu = mtu
            log("MTU négocié : $mtu octets (demandé ${HelloMeshConstants.REQUESTED_MTU})")
            g.discoverServices()
        }

        override fun onServicesDiscovered(g: BluetoothGatt, status: Int) {
            val service = g.getService(HelloMeshConstants.SERVICE_UUID)
            val rx = service?.getCharacteristic(HelloMeshConstants.CHAR_RX_UUID)
            val tx = service?.getCharacteristic(HelloMeshConstants.CHAR_TX_UUID)
            if (rx == null || tx == null) {
                log("Service/characteristics introuvables après découverte — spike interrompu")
                return
            }

            g.setCharacteristicNotification(tx, true)
            val cccd = tx.getDescriptor(HelloMeshConstants.CCCD_UUID)
            if (cccd != null) {
                cccd.value = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
                g.writeDescriptor(cccd)
            }

            rx.writeType = BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE
            rx.value = HelloMeshConstants.PAYLOAD_20_BYTES
            writeStartedAtMs = System.currentTimeMillis()
            g.writeCharacteristic(rx)
            log("Écriture de ${HelloMeshConstants.PAYLOAD_20_BYTES.size} octets sur CHAR_RX…")
        }

        @Suppress("OVERRIDE_DEPRECATION") // 2-arg onCharacteristicChanged : API 33+, gardé pour minSdk 26
        override fun onCharacteristicChanged(g: BluetoothGatt, characteristic: BluetoothGattCharacteristic) {
            if (characteristic.uuid != HelloMeshConstants.CHAR_TX_UUID) return
            val echoedAtMs = System.currentTimeMillis()
            log("Écho reçu sur CHAR_TX (round-trip ${echoedAtMs - writeStartedAtMs} ms)")

            onResult(
                SpikeResult(
                    role = "central",
                    mtu = negotiatedMtu,
                    scanToConnectMs = connectedAtMs - scanStartedAtMs,
                    connectToExchangeMs = echoedAtMs - connectedAtMs,
                    deviceModel = Build.MODEL,
                    androidVersion = Build.VERSION.RELEASE,
                ),
            )
        }
    }
}
