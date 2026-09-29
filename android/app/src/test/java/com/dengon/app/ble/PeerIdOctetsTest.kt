package com.dengon.app.ble

import org.junit.Assert.assertArrayEquals
import org.junit.Test

/** Mêmes vecteurs que `identity::keys::tests::peer_id_en_base32` côté Rust. */
class PeerIdOctetsTest {

    @Test
    fun `zero et le vecteur RFC 4648`() {
        assertArrayEquals(ByteArray(8), PeerIdOctets.depuisBase32("aaaaaaaaaaaaa"))
        // RFC 4648 §10 : BASE32("foobar") = "MZXW6YTBOI======" ; complété de 2 octets nuls.
        assertArrayEquals("foobar".toByteArray() + ByteArray(2), PeerIdOctets.depuisBase32("mzxw6ytboiaaa"))
    }

    @Test(expected = IllegalArgumentException::class)
    fun `longueur incorrecte refusee`() {
        PeerIdOctets.depuisBase32("aaaa")
    }

    @Test(expected = IllegalArgumentException::class)
    fun `caractere hors alphabet refuse`() {
        PeerIdOctets.depuisBase32("aaaaaaaaaaaa1")
    }

    @Test(expected = IllegalArgumentException::class)
    fun `bit de remplissage non nul refuse`() {
        PeerIdOctets.depuisBase32("aaaaaaaaaaaab")
    }
}
