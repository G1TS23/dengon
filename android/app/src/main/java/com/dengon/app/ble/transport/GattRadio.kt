package com.dengon.app.ble.transport

import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothGattServerCallback
import android.bluetooth.BluetoothGattService
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.BluetoothStatusCodes
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.ParcelUuid
import android.util.Log

/**
 * Radio BLE réelle d'[AndroidTransport] (US-213) : serveur GATT + annonce
 * (rôle périphérique) **et** scan + client GATT (rôle central), en même temps.
 *
 * Tout ce qui relève du contrat (liens, file d'événements, quota,
 * fragmentation, règles de déconnexion) est dans [AndroidTransport] ; ici, on
 * ne fait que parler à la pile Android.
 *
 * # Pièges Android traités ici
 *
 * - **Un lien n'est « connecté » qu'une fois les notifications activées.**
 *   Le rappel serveur `onConnectionStateChange` se déclenche pour *toute*
 *   connexion LE, y compris celles que nous initions comme central : côté
 *   périphérique, le lien n'existe qu'à l'écriture du CCCD de `CHAR_TX`.
 * - **Une seule opération GATT en vol par connexion** : écritures et
 *   notifications passent par une file, la suivante part au rappel de
 *   fin de la précédente (`onCharacteristicWrite` / `onNotificationSent`).
 * - **512 octets au plus par valeur GATT**, même avec un MTU de 517.
 * - **Jamais de rappel vers [RappelsRadio] sous notre verrou** :
 *   [AndroidTransport] nous appelle en tenant le sien ; l'inverse
 *   provoquerait un interblocage.
 * - **Règle anti-boucle** ([Annonce.doitInitier]) : seul le nœud au plus
 *   petit préfixe de `peerID` se connecte.
 */
@SuppressLint("MissingPermission") // permissions vérifiées avant démarrage (BlePermissions.allGranted)
class GattRadio(context: Context) : BleRadio {

    private val contexte = context.applicationContext
    private val gestionnaire = contexte.getSystemService(BluetoothManager::class.java)
    private val principal = Handler(Looper.getMainLooper())
    private val verrou = Any()

    private var rappels: RappelsRadio? = null
    private var cfg: TransportConfig? = null
    private var serveur: BluetoothGattServer? = null
    private var caracteristiqueTx: BluetoothGattCharacteristic? = null
    private var actif = false

    /** Une connexion GATT, vue d'un côté ou de l'autre. */
    private inner class Connexion(val pair: RadioPeer, val appareil: BluetoothDevice) {
        var gatt: BluetoothGatt? = null
        var rx: BluetoothGattCharacteristic? = null
        var mtu = MTU_PAR_DEFAUT
        var pret = false
        var fermetureDemandee = false
        var rssi: Short? = null
        val file = ArrayDeque<ByteArray>()
        var enVol = false
    }

    private val connexions = HashMap<RadioPeer, Connexion>()

    /** Adresses récemment tentées en central (évite de marteler un pair qui échoue). */
    private val derniersEssais = HashMap<String, Long>()

    /** Génération de la prochaine [RadioPeer] créée (voir rustdoc de [RadioPeer]). */
    private var prochaineGeneration = 0L

    /**
     * Sous verrou. Le [RadioPeer] actuellement en vie pour cette adresse/rôle
     * (la clé exacte de [connexions]), quelle que soit sa génération.
     * Nécessaire côté périphérique : les rappels `BluetoothGattServerCallback`
     * ne donnent qu'une adresse, jamais la génération.
     */
    private fun pairActuel(adresse: String, role: RadioPeer.Role): RadioPeer? =
        connexions.keys.firstOrNull { it.adresse == adresse && it.role == role }

    // --- BleRadio -----------------------------------------------------------

    override fun demarrer(cfg: TransportConfig, rappels: RappelsRadio) {
        val adaptateur: BluetoothAdapter = gestionnaire?.adapter
            ?: throw TransportException.Backend("pas d'adaptateur Bluetooth")
        if (!adaptateur.isEnabled) throw TransportException.Backend("Bluetooth désactivé")
        try {
            synchronized(verrou) {
                this.cfg = cfg
                this.rappels = rappels
                actif = true
            }
            if (cfg.advertise) demarrerServeurEtAnnonce(adaptateur, cfg)
            if (cfg.scan) demarrerScan(adaptateur)
        } catch (e: SecurityException) {
            arreter()
            throw TransportException.Backend("permission Bluetooth manquante : ${e.message}")
        } catch (e: TransportException) {
            // demarrerServeurEtAnnonce()/demarrerScan() peuvent avoir déjà ouvert
            // le serveur GATT (addService()) avant d'échouer (annonce/scan non
            // pris en charge) : sans ce nettoyage, `serveur`/`actif` restent
            // « ouverts » alors que l'appelant considère start() en échec, et un
            // retry fuit le handle du premier serveur GATT (jamais fermé).
            arreter()
            throw e
        }
    }

    override fun arreter() {
        val aFermer = synchronized(verrou) {
            actif = false
            val liste = connexions.values.toList()
            connexions.clear()
            liste
        }
        val adaptateur = gestionnaire?.adapter
        try {
            adaptateur?.bluetoothLeScanner?.stopScan(rappelScan)
            adaptateur?.bluetoothLeAdvertiser?.stopAdvertising(rappelAnnonce)
            for (c in aFermer) {
                c.gatt?.let {
                    it.disconnect()
                    it.close()
                }
                if (c.pair.role == RadioPeer.Role.PERIPHERAL) serveur?.cancelConnection(c.appareil)
            }
            serveur?.close()
        } catch (e: SecurityException) {
            Log.w(TAG, "arrêt : permission retirée", e)
        }
        synchronized(verrou) {
            serveur = null
            caracteristiqueTx = null
            rappels = null
        }
    }

    override fun chargeUtile(pair: RadioPeer): Int = synchronized(verrou) {
        val mtu = connexions[pair]?.mtu ?: MTU_PAR_DEFAUT
        minOf(FragmentationBle.chargeUtile(mtu), GattDengon.VALEUR_MAX)
    }

    override fun ecrire(pair: RadioPeer, morceau: ByteArray): Boolean = synchronized(verrou) {
        val c = connexions[pair]?.takeIf { it.pret && !it.fermetureDemandee } ?: return false
        c.file.addLast(morceau)
        pomper(c)
        true
    }

    override fun deconnecter(pair: RadioPeer) {
        synchronized(verrou) {
            val c = connexions[pair] ?: return
            c.fermetureDemandee = true
            try {
                // Central : on ferme notre client GATT ; périphérique : on coupe côté serveur.
                if (pair.role == RadioPeer.Role.PERIPHERAL) {
                    serveur?.cancelConnection(c.appareil)
                } else {
                    c.gatt?.disconnect()
                }
            } catch (e: SecurityException) {
                Log.w(TAG, "déconnexion : permission retirée", e)
            }
        }
    }

    // --- Envoi : une opération en vol par connexion -------------------------

    /** Sous verrou. Lance le morceau suivant si rien n'est en vol. */
    private fun pomper(c: Connexion) {
        if (c.enVol || !actif || connexions[c.pair] !== c) return
        val morceau = c.file.firstOrNull() ?: return
        val lance = try {
            when (c.pair.role) {
                RadioPeer.Role.CENTRAL -> ecrireCentral(c, morceau)
                RadioPeer.Role.PERIPHERAL -> notifierPeripherique(c, morceau)
            }
        } catch (e: SecurityException) {
            Log.w(TAG, "envoi : permission retirée", e)
            false
        } catch (e: RuntimeException) {
            // Pile Bluetooth redémarrée sous nos pieds (Bluetooth coupé puis
            // rallumé) : le serveur/client GATT est mort (`DeadObjectException`
            // enveloppée). Vu sur Pixel 8 Pro pendant l'US-312 : l'app
            // plantait sur un envoi depuis l'UI. Ce lien ne reviendra pas ;
            // on jette sa file au lieu de réessayer à l'infini. Le cœur rejoue
            // ce qui n'a pas été accusé.
            Log.w(TAG, "envoi : pile Bluetooth indisponible, file du lien jetée", e)
            c.file.clear()
            return
        }
        if (lance) {
            c.file.removeFirst()
            c.enVol = true
        } else {
            // Pile occupée (ex. écriture système en cours) : on réessaie un peu plus tard.
            principal.postDelayed({ synchronized(verrou) { pomper(c) } }, REESSAI_MS)
        }
    }

    @Suppress("DEPRECATION")
    private fun ecrireCentral(c: Connexion, morceau: ByteArray): Boolean {
        val gatt = c.gatt ?: return false
        val rx = c.rx ?: return false
        val type = BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            gatt.writeCharacteristic(rx, morceau, type) == BluetoothStatusCodes.SUCCESS
        } else {
            rx.writeType = type
            rx.value = morceau
            gatt.writeCharacteristic(rx)
        }
    }

    @Suppress("DEPRECATION")
    private fun notifierPeripherique(c: Connexion, morceau: ByteArray): Boolean {
        val serveur = serveur ?: return false
        val tx = caracteristiqueTx ?: return false
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            serveur.notifyCharacteristicChanged(c.appareil, tx, false, morceau) == BluetoothStatusCodes.SUCCESS
        } else {
            tx.value = morceau
            serveur.notifyCharacteristicChanged(c.appareil, tx, false)
        }
    }

    /** Fin d'une opération d'envoi : on passe au morceau suivant. */
    private fun operationTerminee(pair: RadioPeer) {
        synchronized(verrou) {
            val c = connexions[pair] ?: return
            c.enVol = false
            pomper(c)
        }
    }

    // --- Rôle périphérique : serveur GATT + annonce --------------------------

    private fun demarrerServeurEtAnnonce(adaptateur: BluetoothAdapter, cfg: TransportConfig) {
        val rx = BluetoothGattCharacteristic(
            GattDengon.CHAR_RX,
            BluetoothGattCharacteristic.PROPERTY_WRITE_NO_RESPONSE or BluetoothGattCharacteristic.PROPERTY_WRITE,
            BluetoothGattCharacteristic.PERMISSION_WRITE,
        )
        val tx = BluetoothGattCharacteristic(
            GattDengon.CHAR_TX,
            BluetoothGattCharacteristic.PROPERTY_NOTIFY,
            BluetoothGattCharacteristic.PERMISSION_READ,
        ).apply {
            addDescriptor(
                BluetoothGattDescriptor(
                    GattDengon.CCCD,
                    BluetoothGattDescriptor.PERMISSION_READ or BluetoothGattDescriptor.PERMISSION_WRITE,
                ),
            )
        }
        val service = BluetoothGattService(GattDengon.SERVICE, BluetoothGattService.SERVICE_TYPE_PRIMARY).apply {
            addCharacteristic(rx)
            addCharacteristic(tx)
        }
        val s = gestionnaire?.openGattServer(contexte, rappelServeur)
            ?: throw TransportException.Backend("openGattServer a échoué")
        synchronized(verrou) {
            serveur = s
            caracteristiqueTx = tx
        }
        s.addService(service)

        val annonceur = adaptateur.bluetoothLeAdvertiser
            ?: throw TransportException.Backend("annonce BLE non prise en charge")
        val reglages = AdvertiseSettings.Builder()
            .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY)
            .setTxPowerLevel(AdvertiseSettings.ADVERTISE_TX_POWER_MEDIUM)
            .setConnectable(true)
            .setTimeout(0)
            .build()
        // 3 (flags) + 18 (UUID 128 bits) + 8 (manufacturer data 4 o) = 29 ≤ 31 octets.
        val donnees = AdvertiseData.Builder()
            .addServiceUuid(ParcelUuid(GattDengon.SERVICE))
            .addManufacturerData(Annonce.ID_FABRICANT, Annonce.donnees(cfg.localPeerId))
            .setIncludeDeviceName(false)
            .setIncludeTxPowerLevel(false)
            .build()
        annonceur.startAdvertising(reglages, donnees, rappelAnnonce)
    }

    private val rappelAnnonce = object : AdvertiseCallback() {
        override fun onStartFailure(errorCode: Int) {
            Log.e(TAG, "échec de l'annonce BLE, code=$errorCode")
        }
    }

    private val rappelServeur = object : BluetoothGattServerCallback() {
        override fun onConnectionStateChange(device: BluetoothDevice, status: Int, newState: Int) {
            if (newState != BluetoothProfile.STATE_DISCONNECTED) return
            val (r, pair, motif) = synchronized(verrou) {
                val p = pairActuel(device.address, RadioPeer.Role.PERIPHERAL) ?: return
                val c = connexions.remove(p) ?: return
                if (!c.pret) return
                Triple(rappels, p, motifDeconnexion(status, c.fermetureDemandee))
            }
            Log.i(TAG, "périphérique : ${device.address} déconnecté (status=$status → $motif)")
            r?.deconnecte(pair, motif)
        }

        override fun onMtuChanged(device: BluetoothDevice, mtu: Int) {
            synchronized(verrou) {
                connexionPeripherique(device).mtu = mtu
            }
        }

        override fun onDescriptorWriteRequest(
            device: BluetoothDevice,
            requestId: Int,
            descriptor: BluetoothGattDescriptor,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray,
        ) {
            if (responseNeeded) serveur?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, offset, null)
            if (descriptor.uuid != GattDengon.CCCD || descriptor.characteristic.uuid != GattDengon.CHAR_TX) return
            when {
                value.contentEquals(BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE) -> {
                    val (r, pair) = synchronized(verrou) {
                        if (!actif) return
                        val c = connexionPeripherique(device)
                        if (c.pret) return
                        c.pret = true
                        rappels to c.pair
                    }
                    Log.i(TAG, "périphérique : ${device.address} abonné à CHAR_TX, lien prêt")
                    r?.connecte(pair, null)
                }
                value.contentEquals(BluetoothGattDescriptor.DISABLE_NOTIFICATION_VALUE) -> {
                    // Désabonnement sans déconnexion (certaines piles centrales le
                    // font, ex. en tâche de fond) : sans réabonnement on ne peut
                    // plus livrer de notifications sur ce lien, donc on le ferme
                    // proprement plutôt que de laisser `pret=true` indéfiniment
                    // (ce qui bloquerait la file d'envoi sur des écritures jamais
                    // confirmées par `onNotificationSent`).
                    Log.w(TAG, "périphérique : ${device.address} s'est désabonné de CHAR_TX sans se déconnecter, fermeture du lien")
                    synchronized(verrou) {
                        pairActuel(device.address, RadioPeer.Role.PERIPHERAL)?.let { connexions[it]?.fermetureDemandee = true }
                    }
                    try {
                        serveur?.cancelConnection(device)
                    } catch (e: SecurityException) {
                        Log.w(TAG, "désabonnement : permission retirée", e)
                    }
                }
            }
        }

        override fun onCharacteristicWriteRequest(
            device: BluetoothDevice,
            requestId: Int,
            characteristic: BluetoothGattCharacteristic,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray,
        ) {
            if (responseNeeded) serveur?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, offset, null)
            if (characteristic.uuid != GattDengon.CHAR_RX) return
            val (r, pair) = synchronized(verrou) {
                val p = pairActuel(device.address, RadioPeer.Role.PERIPHERAL) ?: return
                if (connexions[p]?.pret != true) return
                rappels to p
            }
            r?.morceauRecu(pair, value)
        }

        override fun onNotificationSent(device: BluetoothDevice, status: Int) {
            synchronized(verrou) {
                val pair = pairActuel(device.address, RadioPeer.Role.PERIPHERAL) ?: return
                val c = connexions[pair] ?: return
                c.enVol = false
                pomper(c)
            }
        }
    }

    /** Sous verrou. Connexion existante pour cette adresse, ou nouvelle génération sinon. */
    private fun connexionPeripherique(device: BluetoothDevice): Connexion {
        pairActuel(device.address, RadioPeer.Role.PERIPHERAL)?.let { return connexions.getValue(it) }
        val pair = RadioPeer(device.address, RadioPeer.Role.PERIPHERAL, prochaineGeneration++)
        return connexions.getOrPut(pair) { Connexion(pair, device) }
    }

    // --- Rôle central : scan + client GATT ------------------------------------

    /**
     * Mode éco (US-313) : scan `LOW_POWER` (fenêtres courtes, longues pauses)
     * au lieu de `LOW_LATENCY`. Un pair est découvert plus lentement, la
     * batterie tient plus longtemps. Effet immédiat si le scan tourne.
     */
    @Volatile
    var modeEco = false
        private set

    /**
     * @return `true` si le mode demandé est en place. Si le scan en cours n'a
     *   pas pu être relancé, [modeEco] revient à sa valeur d'avant, l'ancien
     *   scan est rétabli (au mieux) et le résultat est `false`.
     */
    fun definirModeEco(eco: Boolean): Boolean {
        val ancien = modeEco
        if (ancien == eco) return true
        modeEco = eco
        val (adaptateur, scanne) = synchronized(verrou) { gestionnaire?.adapter to (actif && cfg?.scan == true) }
        if (adaptateur == null || !scanne) return true
        try {
            adaptateur.bluetoothLeScanner?.stopScan(rappelScan)
            demarrerScan(adaptateur)
            return true
        } catch (e: SecurityException) {
            Log.e(TAG, "changement de mode de scan refusé : ${e.message}")
        } catch (e: TransportException) {
            Log.e(TAG, "changement de mode de scan impossible : ${e.message}")
        }
        // Le scan est arrêté : on rétablit l'ancien mode plutôt que de laisser
        // la découverte morte avec un interrupteur qui affiche le nouveau.
        modeEco = ancien
        try {
            demarrerScan(adaptateur)
        } catch (e: SecurityException) {
            Log.e(TAG, "scan non rétabli : ${e.message}")
        } catch (e: TransportException) {
            Log.e(TAG, "scan non rétabli : ${e.message}")
        }
        return false
    }

    private fun demarrerScan(adaptateur: BluetoothAdapter) {
        val scanneur = adaptateur.bluetoothLeScanner
            ?: throw TransportException.Backend("scan BLE non pris en charge")
        // Le filtre est aussi ce qui autorise le scan écran éteint (Android 8.1+).
        val filtre = ScanFilter.Builder().setServiceUuid(ParcelUuid(GattDengon.SERVICE)).build()
        val mode = if (modeEco) ScanSettings.SCAN_MODE_LOW_POWER else ScanSettings.SCAN_MODE_LOW_LATENCY
        val reglages = ScanSettings.Builder().setScanMode(mode).build()
        scanneur.startScan(listOf(filtre), reglages, rappelScan)
    }

    private val rappelScan = object : ScanCallback() {
        override fun onScanResult(callbackType: Int, result: ScanResult) {
            val prefixe = Annonce.prefixeDistant(result.scanRecord?.getManufacturerSpecificData(Annonce.ID_FABRICANT))
                ?: return
            val adresse = result.device.address
            synchronized(verrou) {
                val c = cfg ?: return
                if (!actif) return
                if (!Annonce.doitInitier(c.localPeerId, prefixe)) return
                if (connexions.keys.any { it.adresse == adresse }) return
                val maintenant = System.currentTimeMillis()
                // Les adresses BLE tournent (adresses privées résolubles) : on oublie les vieilles.
                derniersEssais.entries.removeAll { maintenant - it.value > 60_000L }
                if (maintenant - (derniersEssais[adresse] ?: 0L) < DELAI_ENTRE_ESSAIS_MS) return
                derniersEssais[adresse] = maintenant

                val pair = RadioPeer(adresse, RadioPeer.Role.CENTRAL, prochaineGeneration++)
                val connexion = Connexion(pair, result.device).apply { rssi = result.rssi.toShort() }
                connexions[pair] = connexion
                Log.i(TAG, "central : connexion à $adresse (rssi ${result.rssi})")
                connexion.gatt = result.device.connectGatt(
                    contexte,
                    false,
                    rappelClient(pair),
                    BluetoothDevice.TRANSPORT_LE,
                )
            }
        }

        override fun onScanFailed(errorCode: Int) {
            Log.e(TAG, "échec du scan BLE, code=$errorCode")
        }
    }

    @Suppress("DEPRECATION")
    private fun rappelClient(pair: RadioPeer) = object : BluetoothGattCallback() {
        override fun onConnectionStateChange(gatt: BluetoothGatt, status: Int, newState: Int) {
            when (newState) {
                BluetoothProfile.STATE_CONNECTED -> {
                    val mtu = synchronized(verrou) { cfg?.preferredMtu } ?: MTU_PAR_DEFAUT
                    gatt.requestMtu(mtu)
                }
                BluetoothProfile.STATE_DISCONNECTED -> {
                    gatt.close()
                    val (r, motif) = synchronized(verrou) {
                        val c = connexions[pair]?.takeIf { it.gatt === gatt } ?: return
                        connexions.remove(pair)
                        if (!c.pret) return // échec avant l'ouverture du lien : rien à signaler
                        rappels to motifDeconnexion(status, c.fermetureDemandee)
                    }
                    Log.i(TAG, "central : ${pair.adresse} déconnecté (status=$status → $motif)")
                    r?.deconnecte(pair, motif)
                }
            }
        }

        override fun onMtuChanged(gatt: BluetoothGatt, mtu: Int, status: Int) {
            synchronized(verrou) { connexions[pair]?.mtu = mtu }
            gatt.discoverServices()
        }

        override fun onServicesDiscovered(gatt: BluetoothGatt, status: Int) {
            val service = gatt.getService(GattDengon.SERVICE)
            val rx = service?.getCharacteristic(GattDengon.CHAR_RX)
            val tx = service?.getCharacteristic(GattDengon.CHAR_TX)
            val cccd = tx?.getDescriptor(GattDengon.CCCD)
            if (rx == null || tx == null || cccd == null) {
                Log.w(TAG, "central : ${pair.adresse} n'expose pas le service dengon complet")
                gatt.disconnect()
                return
            }
            synchronized(verrou) { connexions[pair]?.rx = rx }
            gatt.setCharacteristicNotification(tx, true)
            val valeur = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                gatt.writeDescriptor(cccd, valeur)
            } else {
                cccd.value = valeur
                gatt.writeDescriptor(cccd)
            }
        }

        override fun onDescriptorWrite(gatt: BluetoothGatt, descriptor: BluetoothGattDescriptor, status: Int) {
            if (descriptor.uuid != GattDengon.CCCD) return
            if (status != BluetoothGatt.GATT_SUCCESS) {
                Log.w(TAG, "central : abonnement refusé par ${pair.adresse} (status=$status)")
                gatt.disconnect()
                return
            }
            val (r, rssi) = synchronized(verrou) {
                val c = connexions[pair] ?: return
                if (!actif || c.pret) return
                c.pret = true
                rappels to c.rssi
            }
            Log.i(TAG, "central : lien prêt avec ${pair.adresse}")
            r?.connecte(pair, rssi)
        }

        override fun onCharacteristicWrite(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic, status: Int) {
            operationTerminee(pair)
        }

        // API 33+ : la valeur arrive en paramètre.
        override fun onCharacteristicChanged(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic, value: ByteArray) {
            recu(characteristic, value)
        }

        // Avant l'API 33 : seule cette variante est appelée.
        @Deprecated("Remplacée par la variante à 3 paramètres en API 33")
        override fun onCharacteristicChanged(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic) {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) recu(characteristic, characteristic.value ?: return)
        }

        private fun recu(characteristic: BluetoothGattCharacteristic, value: ByteArray) {
            if (characteristic.uuid != GattDengon.CHAR_TX) return
            val r = synchronized(verrou) { if (connexions[pair]?.pret == true) rappels else null }
            r?.morceauRecu(pair, value)
        }
    }

    private companion object {
        const val TAG = "dengon-transport"

        /** ATT_MTU minimal garanti par BLE (`protocol::consts::ATT_MTU_MIN`). */
        const val MTU_PAR_DEFAUT = 23

        const val REESSAI_MS = 20L
        const val DELAI_ENTRE_ESSAIS_MS = 5_000L
    }
}
