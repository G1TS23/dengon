package com.dengon.app.ble.transport

import android.content.Context
import android.util.Log
import com.dengon.app.DengonApplication
import com.dengon.app.ble.Maillage
import com.dengon.app.ble.PeerIdOctets
import com.dengon.app.ble.pseudoDeLAnnonce
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit

/**
 * Le transport du processus (US-213), possédé par `MeshForegroundService`.
 *
 * Depuis l'US-306, il est **branché sur le nœud** : la boucle `poll()` du
 * contrat passe chaque lot d'événements au [Maillage], qui les donne à
 * `DengonApplication.noeud` et écrit sur la radio ce que le nœud produit.
 * Le transport annonce le **vrai** `peerID` du nœud (plus de tirage
 * aléatoire). Les événements restent journalisés (écran de debug +
 * `logcat`, étiquette `dengon-transport`) pour les essais sur deux
 * téléphones.
 */
object TransportActif {

    /** Ce que l'écran de debug affiche. */
    data class Etat(
        val demarre: Boolean = false,
        val peerIdLocal: String = "",
        val liens: List<LinkId> = emptyList(),
        /** Liens identifiés par `ANNOUNCE` → `peerID` du pair. */
        val pairs: Map<LinkId, String> = emptyMap(),
        /** `peerID` → pseudo annoncé des pairs reliés (écran réseau). */
        val pseudos: Map<String, String> = emptyMap(),
        /** Mode éco : scan à cycle réduit (US-313). */
        val modeEco: Boolean = false,
        val trames: Int = 0,
        val erreur: String? = null,
        val journal: List<String> = emptyList(),
    )

    private const val TAG = "dengon-transport"
    private const val PERIODE_POLL_MS = 50L
    private const val LIGNES_JOURNAL = 60

    private val etatMutable = MutableStateFlow(Etat())
    val etat: StateFlow<Etat> = etatMutable.asStateFlow()

    // Écrits uniquement sous @Synchronized (demarrer/arreter), mais lus sans
    // verrou depuis le thread de sonder() et depuis vider() (thread de
    // l'UI) : @Volatile garantit la visibilité inter-thread.
    @Volatile
    private var transport: AndroidTransport? = null

    @Volatile
    private var maillage: Maillage? = null
    @Volatile
    private var radio: GattRadio? = null
    private var boucle: ScheduledExecutorService? = null

    /** Démarre radio + transport + boucle. Sans effet s'il tourne déjà. */
    @Synchronized
    fun demarrer(context: Context) {
        if (transport != null) return
        val noeud = (context.applicationContext as DengonApplication).noeud
        val peerIdTexte = noeud.localIdentity().peerId
        val peerId = PeerIdOctets.depuisBase32(peerIdTexte)
        val r = GattRadio(context).also { it.definirModeEco(etatMutable.value.modeEco) }
        val t = AndroidTransport(r)
        try {
            t.start(TransportConfig(localPeerId = peerId))
        } catch (e: TransportException) {
            etatMutable.update { it.copy(erreur = e.message) }
            journaliser("échec du démarrage : ${e.message}")
            return
        }
        transport = t
        radio = r
        maillage = Maillage(t, noeud, lirePseudo = ::pseudoDeLAnnonce, journal = ::journaliser)
        etatMutable.update { it.copy(demarre = true, peerIdLocal = peerIdTexte, erreur = null) }
        journaliser("démarré, peerID $peerIdTexte (préfixe annoncé ${hex(peerId.copyOfRange(0, 4))})")
        boucle = Executors.newSingleThreadScheduledExecutor().also { exec ->
            exec.scheduleWithFixedDelay(::sonder, 0, PERIODE_POLL_MS, TimeUnit.MILLISECONDS)
        }
    }

    @Synchronized
    fun arreter() {
        boucle?.shutdownNow()
        boucle = null
        transport?.stop()
        transport?.let { sonderAvec(it) } // dernières fermetures LOCALE, vues aussi par le nœud
        transport = null
        radio = null
        maillage = null
        etatMutable.update { it.copy(demarre = false, liens = emptyList(), pairs = emptyMap(), pseudos = emptyMap()) }
        journaliser("arrêté")
    }

    /**
     * Écrit tout de suite ce que le nœud vient de produire (appelé par l'UI
     * après `sendMessage`), sans attendre le prochain tour de boucle. Sans
     * effet si le service ne tourne pas : le message reste en file.
     */
    fun vider() {
        maillage?.vider()
    }

    /** Active / désactive le mode éco (US-313). Retenu même service arrêté. */
    fun definirModeEco(eco: Boolean) {
        etatMutable.update { it.copy(modeEco = eco) }
        radio?.definirModeEco(eco)
        journaliser(if (eco) "mode éco : scan à cycle réduit" else "mode normal : scan à faible latence")
    }

    private fun sonder() {
        transport?.let { sonderAvec(it) }
    }

    private fun sonderAvec(t: AndroidTransport) {
        val evenements = t.poll()
        // Un nœud qui lève ne doit pas tuer la boucle de l'exécuteur
        // (une exception non rattrapée annule les exécutions suivantes).
        try {
            maillage?.traiter(evenements)
        } catch (e: RuntimeException) {
            journaliser("erreur du maillage : $e")
        }
        for (evenement in evenements) {
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
                }
            }
        }
        if (evenements.isNotEmpty()) {
            val pairs = maillage?.pairs ?: emptyMap()
            val pseudos = maillage?.pseudos ?: emptyMap()
            etatMutable.update { it.copy(pairs = pairs, pseudos = pseudos) }
        }
    }

    private fun journaliser(ligne: String) {
        Log.i(TAG, ligne)
        val horodatee = "${HEURE.get()?.format(Date())} $ligne"
        etatMutable.update { it.copy(journal = (listOf(horodatee) + it.journal).take(LIGNES_JOURNAL)) }
    }

    private fun hex(octets: ByteArray): String = octets.joinToString("") { "%02x".format(it) }

    private val HEURE = ThreadLocal.withInitial { SimpleDateFormat("HH:mm:ss", Locale.FRANCE) }
}
