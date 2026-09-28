package com.dengon.app.ble.transport

/**
 * Contrat `Transport` côté Kotlin : miroir exact du trait Rust gelé
 * `dengon_ble::Transport` (US-105, `crates/dengon-ble/src/transport.rs`).
 *
 * À l'US-302, ce contrat deviendra une **callback interface** UniFFI :
 * l'objet Kotlin sera passé dans Rust et y sera vu comme un `Transport`, ce
 * qui permettra de lui appliquer la suite de conformité Rust telle quelle.
 * En attendant, `AndroidTransportConformiteTest` transcrit ces cas un pour
 * un (voir `docs/suivi/03-ecarts-conception.md`).
 */
interface Transport {
    /** Démarre l'annonce et/ou le scan. Un second appel lève [TransportException.AlreadyStarted]. */
    @Throws(TransportException::class)
    fun start(cfg: TransportConfig)

    /**
     * Retire et rend les événements accumulés. Ne bloque jamais, ne lève
     * jamais ; vide avant [start]. Ordre garanti **par lien**.
     */
    fun poll(): List<TransportEvent>

    /** Remet une trame à la pile BLE pour ce lien (pas « reçue par le pair »). */
    @Throws(TransportException::class)
    fun send(peerLinkId: LinkId, bytes: ByteArray)

    /** Envoi au mieux à tous les liens ouverts ; sans pair, c'est un succès. */
    @Throws(TransportException::class)
    fun broadcast(bytes: ByteArray)
}

/**
 * Identifiant d'un **lien** (une connexion), jamais d'un nœud. Attribué par
 * un compteur monotone : jamais réutilisé pendant une exécution.
 */
data class LinkId(val raw: Long) {
    override fun toString(): String = "link#$raw"
}

/** Réglages de [Transport.start] (miroir de `TransportConfig`). */
data class TransportConfig(
    /** `peerID` local, 8 octets (A-8). Les 4 premiers sont annoncés. */
    val localPeerId: ByteArray,
    val advertise: Boolean = true,
    val scan: Boolean = true,
    val maxConnections: Int = 8,
    val preferredMtu: Int = 517,
) {
    init {
        require(localPeerId.size == 8) { "localPeerId : 8 octets attendus" }
    }

    override fun equals(other: Any?): Boolean =
        other is TransportConfig &&
            localPeerId.contentEquals(other.localPeerId) &&
            advertise == other.advertise &&
            scan == other.scan &&
            maxConnections == other.maxConnections &&
            preferredMtu == other.preferredMtu

    override fun hashCode(): Int =
        listOf(localPeerId.contentHashCode(), advertise, scan, maxConnections, preferredMtu).hashCode()
}

/** Pourquoi un lien s'est fermé (miroir de `DisconnectReason`). */
enum class DisconnectReason {
    /** Le pair s'est déconnecté proprement. */
    PROPRE,

    /** Coupure sans prévenir (hors de portée, radio coupée, supervision timeout). */
    BRUTALE,

    /** Notre nœud a fermé le lien (quota, arrêt, politique locale). */
    LOCALE,
}

/** Ce qu'un transport remonte au cœur (miroir de `TransportEvent`). */
sealed class TransportEvent {
    abstract val peerLinkId: LinkId

    data class PeerConnected(override val peerLinkId: LinkId, val rssi: Short?) : TransportEvent()

    data class PeerDisconnected(override val peerLinkId: LinkId, val reason: DisconnectReason) : TransportEvent()

    /** Trame **complète** : la fragmentation BLE est déjà réassemblée. */
    class FrameReceived(override val peerLinkId: LinkId, val bytes: ByteArray) : TransportEvent() {
        override fun equals(other: Any?): Boolean =
            other is FrameReceived && peerLinkId == other.peerLinkId && bytes.contentEquals(other.bytes)

        override fun hashCode(): Int = 31 * peerLinkId.hashCode() + bytes.contentHashCode()

        override fun toString(): String = "FrameReceived($peerLinkId, ${bytes.size} o)"
    }
}

/** Erreurs du transport (miroir de `TransportError`). */
sealed class TransportException(message: String) : Exception(message) {
    class NotStarted : TransportException("le transport n'est pas démarré")

    class AlreadyStarted : TransportException("le transport est déjà démarré")

    class UnknownPeer(val link: LinkId) : TransportException("pair inconnu ou déconnecté : $link")

    class FrameTooLarge(val size: Int, val max: Int) : TransportException("trame de $size octets, maximum $max")

    class TooManyConnections(val max: Int) : TransportException("quota de connexions atteint ($max)")

    class Backend(details: String) : TransportException("erreur de la pile BLE : $details")
}
