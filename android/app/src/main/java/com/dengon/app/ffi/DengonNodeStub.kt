package com.dengon.app.ffi

/**
 * Surface Kotlin de la façade `dengon-ffi` v0 (US-106), miroir de
 * `interface DengonNode` dans `crates/dengon-ffi/src/dengon.udl`.
 *
 * Porte le nom que lui donnent les bindings générés : UniFFI produit une
 * `class DengonNode` (l'objet Rust) qui implémente `DengonNodeInterface`.
 * L'UI doit donc dépendre de **[DengonNodeInterface]** : aujourd'hui elle
 * reçoit un [DengonNodeStub], à l'US-302 un `DengonNode(identity)` généré,
 * sans autre changement (revue PR #69, Paul).
 */
interface DengonNodeInterface {
    @Throws(DengonException::class)
    fun sendMessage(destPeerId: String, body: String): String
    fun pollEvents(): List<NodeEvent>
    fun onPeerConnected(peerId: String)
    fun listConversations(): List<Conversation>
    fun listMessages(convId: String): List<Message>
}

/**
 * Bouchon en mémoire de [DengonNodeInterface] (US-106) : aucun appel au cœur
 * Rust, données canned. Sert à faire avancer l'UI (US-214, US-215) un sprint
 * entier avant que le vrai FFI (US-302) existe.
 *
 * **Thread-safe**, comme le bouchon Rust (`Mutex`) et comme le sera l'objet
 * généré : l'UI et le service de premier plan l'appellent depuis des threads
 * différents. Toutes les méthodes prennent le même verrou (revue PR #69, Paul).
 */
class DengonNodeStub(private val identity: Identity) : DengonNodeInterface {

    private val verrou = Any()
    private val messages = mutableListOf<Message>()
    private val conversations = mutableListOf<Conversation>()
    private val pendingEvents = mutableListOf<NodeEvent>()
    private val connectedPeers = mutableSetOf<String>()

    init {
        // Conversation canned pré-remplie : le critère « un test Kotlin
        // affiche une conversation » (DoR US-106) doit marcher sans appel
        // préalable à sendMessage.
        val bienvenue = Message(
            msgUuid = "msg-canned-0",
            convId = "conv-canned",
            authorPeerId = "peer-canned",
            body = "Bienvenue sur dengon (donnée canned, bouchon US-106)",
            outgoing = false,
            sentMs = 0L,
            status = MessageStatus.DELIVERED,
        )
        messages += bienvenue
        conversations += Conversation(
            convId = "conv-canned",
            peerId = "peer-canned",
            peerPseudo = "Alice (canned)",
            lastMessage = bienvenue,
            unreadCount = 1u,
        )
    }

    override fun sendMessage(destPeerId: String, body: String): String = synchronized(verrou) {
        val msgUuid = "msg-${messages.size}"
        val convId = "conv-$destPeerId"
        val status = if (destPeerId in connectedPeers) MessageStatus.IN_FLIGHT else MessageStatus.QUEUED
        val message = Message(
            msgUuid = msgUuid,
            convId = convId,
            authorPeerId = identity.peerId,
            body = body,
            outgoing = true,
            sentMs = System.currentTimeMillis(),
            status = status,
        )
        messages += message

        val existing = conversations.indexOfFirst { it.convId == convId }
        if (existing >= 0) {
            conversations[existing] = conversations[existing].copy(lastMessage = message)
        } else {
            conversations += Conversation(
                convId = convId,
                peerId = destPeerId,
                peerPseudo = destPeerId,
                lastMessage = message,
                unreadCount = 0u,
            )
        }
        msgUuid
    }

    override fun pollEvents(): List<NodeEvent> = synchronized(verrou) {
        val events = pendingEvents.toList()
        pendingEvents.clear()
        events
    }

    override fun onPeerConnected(peerId: String): Unit = synchronized(verrou) {
        if (connectedPeers.add(peerId)) {
            pendingEvents += NodeEvent.PeerConnected(peerId)
        }
    }

    override fun listConversations(): List<Conversation> = synchronized(verrou) { conversations.toList() }

    override fun listMessages(convId: String): List<Message> =
        synchronized(verrou) { messages.filter { it.convId == convId } }
}

// ---------------------------------------------------------------------------
// Fonctions libres du `namespace dengon` (identité + QR).
//
// Fonctions de premier niveau, pas un `object` : c'est ainsi qu'UniFFI génère
// les fonctions d'un `namespace` (`generateIdentity(...)`, pas
// `DengonIdentity.generate(...)`) — revue PR #69.
//
// Même algorithme que `crates/dengon-ffi/src/lib.rs`, mais les deux sont des
// bouchons indépendants : placeholders NON cryptographiques (voir les notes
// côté Rust). Base64url codé à la main plutôt que via `android.util.Base64` :
// `app/build.gradle.kts` a `unitTests.isReturnDefaultValues = true` (pas de
// Robolectric), un appel `android.*` en test JVM pur renverrait `null` au lieu
// de s'exécuter.
// ---------------------------------------------------------------------------

private const val QR_PREFIX = "dengon:v1:"
private const val KEY_LEN = 32
private const val PSEUDO_MAX_OCTETS = 0xFF

/** Placeholder cryptographique : pas de vraie génération de clés ici. */
fun generateIdentity(pseudo: String): Identity {
    val pubStatic = ByteArray(KEY_LEN)
    val pubSign = ByteArray(KEY_LEN)
    pseudo.toByteArray(Charsets.UTF_8).forEachIndexed { i, byte ->
        pubStatic[i % KEY_LEN] = (pubStatic[i % KEY_LEN].toInt() xor byte.toInt()).toByte()
        pubSign[i % KEY_LEN] = (pubSign[i % KEY_LEN].toInt() xor (byte + 1)).toByte()
    }
    return Identity(peerId = toHex(pubStatic.copyOfRange(0, 8)), pseudo = pseudo, pubStatic = pubStatic, pubSign = pubSign)
}

/**
 * `docs/synthese/06-securite.md` : `dengon:v1:<base64url(len‖pseudo‖pub_static‖pub_sign)>`.
 *
 * Un pseudo de plus de 255 octets UTF-8 est **tronqué** à la dernière
 * frontière de caractère ≤ 255 octets — même règle que le Rust. Avant, le
 * Kotlin levait une exception et le Rust coupait un caractère en deux (QR
 * indécodable) : les deux bouchons se comportaient différemment.
 */
fun identityQrCode(identity: Identity): String {
    val pseudoBytes = tronquerUtf8(identity.pseudo, PSEUDO_MAX_OCTETS)

    val payload = ByteArray(1 + pseudoBytes.size + identity.pubStatic.size + identity.pubSign.size)
    payload[0] = pseudoBytes.size.toByte()
    pseudoBytes.copyInto(payload, destinationOffset = 1)
    identity.pubStatic.copyInto(payload, destinationOffset = 1 + pseudoBytes.size)
    identity.pubSign.copyInto(payload, destinationOffset = 1 + pseudoBytes.size + identity.pubStatic.size)

    return QR_PREFIX + encodeBase64Url(payload)
}

/**
 * Décode un QR d'identité. **Toute** entrée invalide lève
 * [DengonException.Internal] (comme `Err(DengonError::Internal)` côté Rust),
 * y compris le cas le plus courant : un QR qui n'est pas un QR dengon (URL,
 * menu…). Avant, ce cas levait `IllegalArgumentException` et échappait à un
 * appelant qui attrape `DengonException` (revue PR #69, Paul).
 */
@Throws(DengonException::class)
fun identityFromQrCode(qrCode: String): Identity {
    if (!qrCode.startsWith(QR_PREFIX)) {
        throw DengonException.Internal("QR code dengon invalide (préfixe manquant)")
    }
    val payload = decodeBase64Url(qrCode.removePrefix(QR_PREFIX))

    // Bornage explicite avant indexation, comme côté Rust
    // (`payload.get(...).ok_or(DengonError::Internal)`).
    if (payload.isEmpty()) {
        throw DengonException.Internal("QR code dengon invalide (payload vide)")
    }
    val pseudoLen = payload[0].toInt() and 0xFF
    var offset = 1
    if (payload.size < offset + pseudoLen + 2 * KEY_LEN) {
        throw DengonException.Internal("QR code dengon invalide (payload tronqué)")
    }
    val pseudo = String(payload, offset, pseudoLen, Charsets.UTF_8)
    offset += pseudoLen
    val pubStatic = payload.copyOfRange(offset, offset + KEY_LEN)
    offset += KEY_LEN
    val pubSign = payload.copyOfRange(offset, offset + KEY_LEN)

    return Identity(peerId = toHex(pubStatic.copyOfRange(0, 8)), pseudo = pseudo, pubStatic = pubStatic, pubSign = pubSign)
}

/**
 * Code de vérification 60 chiffres, ordre-indépendant.
 *
 * Placeholder (FNV-1a) : la vraie version SHA-512 arrive avec `crypto` côté
 * Rust (US-203) — voir la note équivalente dans `crates/dengon-ffi/src/lib.rs`.
 * Seule la FORME (12 groupes de 5 chiffres, même code des deux côtés) fait
 * partie du contrat v0.
 */
fun verificationCode(local: Identity, remote: Identity): String {
    val fpLocal = local.pubStatic + local.pubSign
    val fpRemote = remote.pubStatic + remote.pubSign
    val (fpA, fpB) = if (compareBytes(fpLocal, fpRemote) <= 0) fpLocal to fpRemote else fpRemote to fpLocal
    val material = fpA + fpB

    return (0 until 12).joinToString(" ") { groupIndex ->
        var hash = FNV_OFFSET_BASIS xor groupIndex.toULong()
        for (byte in material) {
            hash = hash xor (byte.toInt() and 0xFF).toULong()
            hash *= FNV_PRIME
        }
        "%05d".format((hash % 100_000UL).toInt())
    }
}

/** Les octets UTF-8 de `texte`, tronqués à `max` sans couper un caractère. */
private fun tronquerUtf8(texte: String, max: Int): ByteArray {
    val octets = texte.toByteArray(Charsets.UTF_8)
    if (octets.size <= max) return octets
    var fin = max
    // Un octet de continuation UTF-8 s'écrit 10xxxxxx : on recule jusqu'au
    // début d'un caractère.
    while (fin > 0 && (octets[fin].toInt() and 0xC0) == 0x80) {
        fin--
    }
    return octets.copyOf(fin)
}

private fun compareBytes(a: ByteArray, b: ByteArray): Int {
    val len = minOf(a.size, b.size)
    for (i in 0 until len) {
        val cmp = (a[i].toInt() and 0xFF) - (b[i].toInt() and 0xFF)
        if (cmp != 0) return cmp
    }
    return a.size - b.size
}

private fun toHex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it) }

// base64url (RFC 4648 §5) sans padding, écrit à la main. Même algorithme que
// `encode_base64url`/`decode_base64url` côté Rust.
private const val BASE64URL_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"

private fun encodeBase64Url(data: ByteArray): String {
    val out = StringBuilder((data.size + 2) / 3 * 4)
    var i = 0
    while (i < data.size) {
        val b0 = data[i].toInt() and 0xFF
        val b1 = if (i + 1 < data.size) data[i + 1].toInt() and 0xFF else null
        val b2 = if (i + 2 < data.size) data[i + 2].toInt() and 0xFF else null

        out.append(BASE64URL_ALPHABET[b0 shr 2])
        out.append(BASE64URL_ALPHABET[((b0 and 0b0000_0011) shl 4) or ((b1 ?: 0) shr 4)])
        if (b1 != null) {
            out.append(BASE64URL_ALPHABET[((b1 and 0b0000_1111) shl 2) or ((b2 ?: 0) shr 6)])
        }
        if (b2 != null) {
            out.append(BASE64URL_ALPHABET[b2 and 0b0011_1111])
        }
        i += 3
    }
    return out.toString()
}

private fun sixBits(char: Char): Int = when (char) {
    in 'A'..'Z' -> char - 'A'
    in 'a'..'z' -> char - 'a' + 26
    in '0'..'9' -> char - '0' + 52
    '-' -> 62
    '_' -> 63
    else -> throw DengonException.Internal("caractère base64url invalide : '$char'")
}

private fun decodeBase64Url(input: String): ByteArray {
    val out = mutableListOf<Byte>()
    var i = 0
    while (i < input.length) {
        val v0 = sixBits(input[i])
        val v1 = sixBits(input.getOrElse(i + 1) { throw DengonException.Internal("base64url tronqué") })
        out += ((v0 shl 2) or (v1 shr 4)).toByte()

        val c2 = input.getOrNull(i + 2) ?: break
        val v2 = sixBits(c2)
        out += ((v1 shl 4) or (v2 shr 2)).toByte()

        val c3 = input.getOrNull(i + 3) ?: break
        val v3 = sixBits(c3)
        out += ((v2 shl 6) or v3).toByte()

        i += 4
    }
    return out.toByteArray()
}

private const val FNV_OFFSET_BASIS = 0xcbf29ce484222325UL
private const val FNV_PRIME = 0x100000001b3UL
