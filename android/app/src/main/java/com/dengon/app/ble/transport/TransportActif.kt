package com.dengon.app.ble.transport

import android.content.Context
import android.os.Build
import android.util.Log
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import java.security.SecureRandom
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit
import java.util.zip.CRC32

/**
 * Le transport du processus (US-213), possédé par `MeshForegroundService`.
 *
 * Tant que le cœur n'est pas branché (US-301/US-306), c'est ici que tourne la
 * boucle `poll()` du contrat : les événements sont journalisés (écran de
 * debug + `logcat`, étiquette `dengon-transport`) pour les essais sur deux
 * téléphones. Un **battement** optionnel diffuse une trame toutes les 30 s :
 * c'est ce qui montre que le lien vit encore écran éteint.
 */
object TransportActif {

    /** Ce que l'écran de debug affiche. */
    data class Etat(
        val demarre: Boolean = false,
        val peerIdLocal: String = "",
        val liens: List<LinkId> = emptyList(),
        val trames: Int = 0,
        val battement: Boolean = false,
        val erreur: String? = null,
        val journal: List<String> = emptyList(),
    )

    private const val TAG = "dengon-transport"
    private const val PERIODE_POLL_MS = 50L
    private const val PERIODE_BATTEMENT_S = 30L
    private const val LIGNES_JOURNAL = 60

    private val etatMutable = MutableStateFlow(Etat())
    val etat: StateFlow<Etat> = etatMutable.asStateFlow()

    private var transport: AndroidTransport? = null
    private var boucle: ScheduledExecutorService? = null
    private var numeroBattement = 0

    /** Démarre radio + transport + boucle. Sans effet s'il tourne déjà. */
    @Synchronized
    fun demarrer(context: Context) {
        if (transport != null) return
        val peerId = peerIdLocal(context)
        val t = AndroidTransport(GattRadio(context))
        try {
            t.start(TransportConfig(localPeerId = peerId))
        } catch (e: TransportException) {
            etatMutable.update { it.copy(erreur = e.message) }
            journaliser("échec du démarrage : ${e.message}")
            return
        }
        transport = t
        etatMutable.update { it.copy(demarre = true, peerIdLocal = hex(peerId), erreur = null) }
        journaliser("démarré, peerID ${hex(peerId)} (préfixe annoncé ${hex(peerId.copyOfRange(0, 4))})")
        boucle = Executors.newSingleThreadScheduledExecutor().also { exec ->
            exec.scheduleWithFixedDelay(::sonder, 0, PERIODE_POLL_MS, TimeUnit.MILLISECONDS)
            exec.scheduleWithFixedDelay(::battre, PERIODE_BATTEMENT_S, PERIODE_BATTEMENT_S, TimeUnit.SECONDS)
        }
    }

    @Synchronized
    fun arreter() {
        boucle?.shutdownNow()
        boucle = null
        transport?.stop()
        transport?.let { sonderAvec(it) } // dernières fermetures LOCALE
        transport = null
        etatMutable.update { it.copy(demarre = false, liens = emptyList()) }
        journaliser("arrêté")
    }

    /** Diffuse `octets` à tous les liens ouverts et le consigne. */
    fun diffuser(octets: ByteArray, libelle: String) {
        val t = transport ?: return journaliser("diffusion impossible : transport arrêté")
        try {
            t.broadcast(octets)
            journaliser("diffusé $libelle : ${octets.size} o, crc ${crc(octets)}, vers ${t.nbLiens} lien(s)")
        } catch (e: TransportException) {
            journaliser("diffusion refusée : ${e.message}")
        }
    }

    /** Active / coupe le battement périodique. */
    fun basculerBattement() {
        etatMutable.update { it.copy(battement = !it.battement) }
        journaliser(if (etatMutable.value.battement) "battement activé (30 s)" else "battement coupé")
    }

    private fun battre() {
        if (!etatMutable.value.battement) return
        numeroBattement++
        diffuser("battement ${Build.MODEL} #$numeroBattement".toByteArray(), "battement #$numeroBattement")
    }

    private fun sonder() {
        transport?.let { sonderAvec(it) }
    }

    private fun sonderAvec(t: AndroidTransport) {
        for (evenement in t.poll()) {
            when (evenement) {
                is TransportEvent.PeerConnected -> {
                    etatMutable.update { it.copy(liens = it.liens + evenement.peerLinkId) }
                    journaliser("${evenement.peerLinkId} ouvert (rssi ${evenement.rssi ?: "?"})")
                }
                is TransportEvent.PeerDisconnected -> {
                    etatMutable.update { it.copy(liens = it.liens - evenement.peerLinkId) }
                    journaliser("${evenement.peerLinkId} fermé : ${evenement.reason}")
                }
                is TransportEvent.FrameReceived -> {
                    etatMutable.update { it.copy(trames = it.trames + 1) }
                    journaliser("reçu sur ${evenement.peerLinkId} : ${apercu(evenement.bytes)}")
                }
            }
        }
    }

    private fun journaliser(ligne: String) {
        Log.i(TAG, ligne)
        val horodatee = "${HEURE.get()?.format(Date())} $ligne"
        etatMutable.update { it.copy(journal = (listOf(horodatee) + it.journal).take(LIGNES_JOURNAL)) }
    }

    /** `peerID` provisoire, tiré une fois par installation (vrai `peerID` : US-306). */
    private fun peerIdLocal(context: Context): ByteArray {
        val prefs = context.getSharedPreferences("dengon_transport", Context.MODE_PRIVATE)
        prefs.getString("peer_id", null)?.let { return unhex(it) }
        val id = ByteArray(8).also { SecureRandom().nextBytes(it) }
        prefs.edit().putString("peer_id", hex(id)).apply()
        return id
    }

    private fun apercu(octets: ByteArray): String {
        val texte = octets.take(40).toByteArray().toString(Charsets.UTF_8)
        val lisible = texte.all { it.isLetterOrDigit() || it in " #-_.:'" }
        return "${octets.size} o, crc ${crc(octets)}" + if (lisible) " « $texte »" else ""
    }

    private fun crc(octets: ByteArray): String = "%08x".format(CRC32().apply { update(octets) }.value)

    private fun hex(octets: ByteArray): String = octets.joinToString("") { "%02x".format(it) }

    private fun unhex(s: String): ByteArray = ByteArray(s.length / 2) { i -> s.substring(2 * i, 2 * i + 2).toInt(16).toByte() }

    private val HEURE = ThreadLocal.withInitial { SimpleDateFormat("HH:mm:ss", Locale.FRANCE) }
}
