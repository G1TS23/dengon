package com.dengon.app.ffi

/**
 * Types échangés entre l'UI et le cœur `dengon-core`, miroir Kotlin du
 * contrat UniFFI v0 défini dans `crates/dengon-ffi/src/dengon.udl` (US-106).
 *
 * FIGÉS une fois le contrat annoncé gelé en point d'équipe : ajouter un champ
 * après coup demande une réunion, pas un commit (DoR de l'US-106).
 */

/**
 * `ByteArray` n'a pas d'égalité structurelle : `equals`/`hashCode` sont
 * réécrits à la main plutôt que de faire d'`Identity` une `data class`, pour
 * éviter le piège classique (deux identités avec les mêmes octets mais deux
 * instances de tableau différentes seraient jugées différentes).
 */
class Identity(
    val peerId: String,
    val pseudo: String,
    val pubStatic: ByteArray,
    val pubSign: ByteArray,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is Identity) return false
        return peerId == other.peerId &&
            pseudo == other.pseudo &&
            pubStatic.contentEquals(other.pubStatic) &&
            pubSign.contentEquals(other.pubSign)
    }

    override fun hashCode(): Int {
        var result = peerId.hashCode()
        result = 31 * result + pseudo.hashCode()
        result = 31 * result + pubStatic.contentHashCode()
        result = 31 * result + pubSign.contentHashCode()
        return result
    }
}

/** Statuts MVP (`docs/synthese/07-cycle-de-vie-et-statuts.md` §1). */
enum class MessageStatus {
    QUEUED,
    IN_FLIGHT,
    DELIVERED,
    READ,
    EXPIRED,
    CANCELLED,
}

data class Message(
    val msgUuid: String,
    val convId: String,
    val authorPeerId: String,
    val body: String,
    val outgoing: Boolean,
    val sentMs: Long,
    val status: MessageStatus,
)

data class Conversation(
    val convId: String,
    val peerId: String,
    val peerPseudo: String,
    val lastMessage: Message?,
    val unreadCount: Int,
)

sealed class NodeEvent {
    data class MessageReceived(val message: Message) : NodeEvent()
    data class StatusChanged(val msgUuid: String, val status: MessageStatus) : NodeEvent()
    data class PeerConnected(val peerId: String) : NodeEvent()
    data class PeerDisconnected(val peerId: String) : NodeEvent()
}

/** Miroir de l'enum `[Error] DengonError` du `.udl`. */
class DengonException(message: String) : Exception(message)
