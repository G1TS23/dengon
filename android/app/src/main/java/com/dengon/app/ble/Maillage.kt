package com.dengon.app.ble

import com.dengon.app.ble.transport.LinkId
import com.dengon.app.ble.transport.Transport
import com.dengon.app.ble.transport.TransportEvent
import com.dengon.app.ble.transport.TransportException
import com.dengon.app.ffi.DengonException
import com.dengon.app.ffi.DengonNodeInterface
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
 *    valide → lien ↔ `peerID`, puis `onPeerConnected` (qui lance le
 *    handshake Noise `XX`) ; sinon la trame est jetée ;
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
 * Ne consomme **pas** `pollEvents` : l'UI en reste la seule lectrice.
 *
 * Thread-safe : la boucle du service appelle [traiter], l'UI [vider].
 */
class Maillage(
    private val transport: Transport,
    private val noeud: DengonNodeInterface,
    /** `peerID` de l'émetteur si `trame` est un `ANNOUNCE` valide, `null` sinon. */
    private val lireAnnonce: (ByteArray) -> String? = ::peerIdDeLAnnonce,
    private val journal: (String) -> Unit = {},
) {
    private val pairParLien = HashMap<LinkId, String>()
    private val lienParPair = HashMap<String, LinkId>()

    /** Liens identifiés, pour l'écran de debug. */
    @get:Synchronized
    val pairs: Map<LinkId, String> get() = pairParLien.toMap()

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
        val annonce = lireAnnonce(octets)
        if (annonce == null) {
            journal("$lien : trame ignorée, ANNOUNCE attendu")
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
        journal("$lien ↔ $annonce")
        appelerNoeud("connexion de $annonce") { noeud.onPeerConnected(annonce) }
    }

    private fun fermer(lien: LinkId) {
        val pair = pairParLien.remove(lien) ?: return
        lienParPair.remove(pair)
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

/** Lecture d'`ANNOUNCE` par le vrai FFI (vérifie signature et `peerID`). */
fun peerIdDeLAnnonce(trame: ByteArray): String? =
    try {
        identityFromAnnounce(trame).peerId
    } catch (e: DengonException) {
        null
    }
