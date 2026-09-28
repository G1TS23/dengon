package com.dengon.app.ffi

/**
 * Types échangés entre l'UI et le cœur `dengon-core`, miroir Kotlin du
 * contrat UniFFI v0 défini dans `crates/dengon-ffi/src/dengon.udl` (US-106).
 *
 * **Même forme que les bindings générés** par `uniffi-bindgen` 0.28.3 à
 * partir de ce `.udl` et de `crates/dengon-ffi/uniffi.toml` : même paquet,
 * mêmes noms, mêmes types (`ByteArray`, `Long`, `UInt`), `data class` à
 * champs `var`, `DengonException` scellée. À l'US-302, ce fichier et
 * `DengonNodeStub.kt` seront remplacés par le fichier généré sans que le code
 * de l'UI ait à changer (revue PR #69, Paul — l'ancienne version divergeait
 * sur cinq points et aurait cassé l'UI à la compilation).
 *
 * FIGÉS une fois le contrat annoncé gelé en point d'équipe : ajouter un champ
 * après coup demande une réunion, pas un commit (DoR de l'US-106).
 */

/**
 * ⚠ `data class` avec des champs `ByteArray` : **pas** d'égalité par contenu
 * (`equals` compare les références des tableaux). C'est ce que génère UniFFI,
 * donc le bouchon fait pareil : comparer deux identités se fait sur `peerId`,
 * ou champ par champ avec `contentEquals`. Le bouchon précédent réécrivait
 * `equals` : l'UI aurait pu en dépendre, et casser silencieusement à l'US-302.
 */
data class Identity(
    var peerId: String,
    var pseudo: String,
    var pubStatic: ByteArray,
    var pubSign: ByteArray,
)

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
    var msgUuid: String,
    var convId: String,
    var authorPeerId: String,
    var body: String,
    var outgoing: Boolean,
    /** ms depuis l'epoch (`i64` dans le `.udl`). */
    var sentMs: Long,
    var status: MessageStatus,
)

data class Conversation(
    var convId: String,
    var peerId: String,
    var peerPseudo: String,
    var lastMessage: Message?,
    var unreadCount: UInt,
)

sealed class NodeEvent {
    data class MessageReceived(val message: Message) : NodeEvent()
    data class StatusChanged(val msgUuid: String, val status: MessageStatus) : NodeEvent()
    data class PeerConnected(val peerId: String) : NodeEvent()
    data class PeerDisconnected(val peerId: String) : NodeEvent()
}

/**
 * Miroir de `[Error] enum DengonError` du `.udl` : une sous-classe par
 * variante, comme dans les bindings générés. L'UI peut attraper
 * `DengonException` (toutes) ou une variante précise.
 */
sealed class DengonException(message: String) : Exception(message) {
    class UnknownPeer(message: String) : DengonException(message)
    class NotConnected(message: String) : DengonException(message)
    class Internal(message: String) : DengonException(message)
}
