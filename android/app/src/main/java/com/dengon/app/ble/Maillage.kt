package com.dengon.app.ble

import com.dengon.app.ble.transport.LinkId
import com.dengon.app.ble.transport.Transport
import com.dengon.app.ble.transport.TransportEvent
import com.dengon.app.ble.transport.TransportException
import com.dengon.app.ffi.DengonException
import com.dengon.app.ffi.DengonNodeInterface
import com.dengon.app.ffi.announceIsRelay
import com.dengon.app.ffi.identityFromAnnounce

/**
 * Le pont entre la radio et le nœud (US-306) : ce qui fait enfin passer les
 * octets de [Transport] (US-213) à `dengon-core` (US-301, via le FFI US-302).
 *
 * Le transport ne connaît que des [LinkId] (une connexion), le nœud que des
 * `peerID` (un pair). La liaison se fait par l'**`ANNOUNCE`** signé que
 * chaque côté écrit en première trame d'un lien qui s'ouvre
 * (`synthese/07` §6, étape 1) :
 *
 * 1. `PeerConnected(lien)` → on écrit notre `ANNOUNCE` sur ce lien ;
 * 2. première trame reçue sur un lien **non identifié** → [lireAnnonce] ;
 *    valide → lien ↔ `peerID`, puis `onNeighborAnnounced` : le nœud lance le
 *    handshake Noise `XX` avec un pair, ou lie un **relais ESP32** sans
 *    session et lui confie ses enveloppes (US-312) ; sinon la trame est
 *    jetée ;
 * 3. trame sur un lien identifié → `onBytesReceived` ;
 * 4. `PeerDisconnected(lien)` identifié → `onPeerDisconnected`.
 *
 * Le transport garantit l'ordre **par lien** : l'`ANNOUNCE` d'un pair arrive
 * toujours avant ses paquets, puisqu'il l'écrit avant de connaître le nôtre.
 *
 * Après chaque lot d'événements, et après chaque envoi de l'UI ([vider]),
 * `takeOutgoing` est écrit sur le lien du pair destinataire. Un pair sans
 * lien : la trame est jetée, l'outbox du cœur la rejouera à la reconnexion.
 *
 * Option de démonstration (US-312) : si [ignorerLiensDirects] rend `true`,
 * un lien dont l'`ANNOUNCE` n'est pas celui d'un relais est ignoré. Deux
 * téléphones posés côte à côte se comportent alors comme s'ils étaient hors
 * de portée l'un de l'autre, et ne communiquent que par le relais.
 *
 * Ne consomme **pas** `pollEvents` : l'UI en reste la seule lectrice.
 *
 * Thread-safe : la boucle du service appelle [traiter], l'UI [vider].
 */
class Maillage(
    private val transport: Transport,
    private val noeud: DengonNodeInterface,
    /** Lecture de l'`ANNOUNCE` : `null` si `trame` n'en est pas un valide. */
    private val lireAnnonce: (ByteArray) -> AnnonceLue? = ::annonceLue,
    private val journal: (String) -> Unit = {},
    /** `true` si `trame` est l'`ANNOUNCE` d'un relais (US-312). */
    private val estRelais: (ByteArray) -> Boolean = ::announceIsRelay,
    /** Option de démonstration, lue à chaque nouveau lien (voir la doc de classe). */
    private val ignorerLiensDirects: () -> Boolean = { false },
) {
    private val pairParLien = HashMap<LinkId, String>()
    private val lienParPair = HashMap<String, LinkId>()

    /** Liens identifiés, pour l'écran de debug. */
    @get:Synchronized
    val pairs: Map<LinkId, String> get() = pairParLien.toMap()

    private val pseudoParPair = HashMap<String, String>()

    /** `peerID` → pseudo annoncé, pour les pairs actuellement reliés. */
    @get:Synchronized
    val pseudos: Map<String, String> get() = pseudoParPair.toMap()

    /** Traite un lot d'événements du transport, puis vide la sortie du nœud. */
    @Synchronized
    fun traiter(evenements: List<TransportEvent>) {
        for (evenement in evenements) {
            when (evenement) {
                is TransportEvent.PeerConnected -> annoncer(evenement.peerLinkId)
                is TransportEvent.FrameReceived -> recevoir(evenement.peerLinkId, evenement.bytes)
                is TransportEvent.PeerDisconnected -> fermer(evenement.peerLinkId)
            }
        }
        vider()
    }

    /** Écrit sur la radio tout ce que le nœud a produit (`takeOutgoing`). */
    @Synchronized
    fun vider() {
        for (sortante in noeud.takeOutgoing()) {
            val lien = lienParPair[sortante.peerId]
            if (lien == null) {
                journal("trame pour ${sortante.peerId} sans lien : rejouée à la reconnexion")
                continue
            }
            envoyer(lien, sortante.frame)
        }
    }

    private fun annoncer(lien: LinkId) {
        val annonce = try {
            noeud.announceFrame()
        } catch (e: DengonException) {
            return journal("ANNOUNCE impossible : ${e.message}")
        }
        envoyer(lien, annonce)
    }

    private fun recevoir(lien: LinkId, octets: ByteArray) {
        val pair = pairParLien[lien]
        if (pair != null) {
            appelerNoeud("octets de $pair") { noeud.onBytesReceived(pair, octets) }
            return
        }
        val lue = lireAnnonce(octets)
        if (lue == null) {
            journal("$lien : trame ignorée, ANNOUNCE attendu")
            return
        }
        val annonce = lue.peerId
        val relais = estRelais(octets)
        if (!relais && ignorerLiensDirects()) {
            journal("$lien : $annonce n'est pas un relais, lien direct ignoré (option debug)")
            return
        }
        if (annonce in lienParPair) {
            // Deux liens vers le même pair (course des deux rôles GATT) : le
            // premier reste le seul, le cœur n'en gère qu'un par pair.
            journal("$lien : $annonce déjà relié par ${lienParPair[annonce]}, lien ignoré")
            return
        }
        pairParLien[lien] = annonce
        lienParPair[annonce] = lien
        lue.pseudo?.let { pseudoParPair[annonce] = it }
        journal("$lien ↔ $annonce${if (relais) " (relais)" else ""}")
        appelerNoeud("connexion de $annonce") { noeud.onNeighborAnnounced(octets) }
    }

    private fun fermer(lien: LinkId) {
        val pair = pairParLien.remove(lien) ?: return
        lienParPair.remove(pair)
        pseudoParPair.remove(pair)
        appelerNoeud("déconnexion de $pair") { noeud.onPeerDisconnected(pair) }
    }

    private fun envoyer(lien: LinkId, trame: ByteArray) {
        try {
            transport.send(lien, trame)
        } catch (e: TransportException) {
            // Lien fermé entre-temps : son PeerDisconnected arrive au prochain lot.
            journal("$lien : envoi refusé (${e.message})")
        }
    }

    private inline fun appelerNoeud(quoi: String, appel: () -> Unit) {
        try {
            appel()
        } catch (e: DengonException) {
            journal("$quoi refusé par le nœud : ${e.message}")
        }
    }
}

/** Ce qu'un `ANNOUNCE` valide apprend sur son émetteur. */
data class AnnonceLue(val peerId: String, val pseudo: String?)

/**
 * Lecture d'`ANNOUNCE` par le vrai FFI (vérifie signature et `peerID`), en un
 * seul appel : `peerID` et pseudo viennent de la même vérification.
 */
fun annonceLue(trame: ByteArray): AnnonceLue? =
    try {
        identityFromAnnounce(trame).let { AnnonceLue(it.peerId, it.pseudo) }
    } catch (e: DengonException) {
        null
    }

/** `peerID` de l'émetteur si `trame` est un `ANNOUNCE` valide, `null` sinon. */
fun peerIdDeLAnnonce(trame: ByteArray): String? = annonceLue(trame)?.peerId
