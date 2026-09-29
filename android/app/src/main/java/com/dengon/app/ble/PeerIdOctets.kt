package com.dengon.app.ble

/**
 * `peerID` du FFI (base32 RFC 4648, minuscules, sans padding, 13 caractères)
 * → les 8 octets que le transport annonce et attend dans `TransportConfig`
 * (US-306). Inverse exact de `identity::peer_id_base32` côté Rust.
 */
object PeerIdOctets {

    private const val ALPHABET = "abcdefghijklmnopqrstuvwxyz234567"
    private const val LONGUEUR = 13
    private const val OCTETS = 8

    /** @throws IllegalArgumentException si `peerId` n'est pas un `peerID` bien formé. */
    fun depuisBase32(peerId: String): ByteArray {
        require(peerId.length == LONGUEUR) { "peerID : $LONGUEUR caractères attendus" }
        val sortie = ByteArray(OCTETS)
        var tampon = 0
        var bits = 0
        var i = 0
        for (c in peerId) {
            val valeur = ALPHABET.indexOf(c)
            require(valeur >= 0) { "peerID : caractère hors base32 « $c »" }
            tampon = ((tampon shl 5) or valeur) and 0xFFFF
            bits += 5
            if (bits >= 8) {
                bits -= 8
                sortie[i++] = (tampon shr bits).toByte()
            }
        }
        // 13 × 5 = 65 bits : le dernier bit est du remplissage, forcément nul.
        require(tampon and ((1 shl bits) - 1) == 0) { "peerID : bits de remplissage non nuls" }
        return sortie
    }
}
