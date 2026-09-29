package com.dengon.app.ble

import com.dengon.app.ble.transport.AndroidTransport
import com.dengon.app.ble.transport.DisconnectReason
import com.dengon.app.ble.transport.FauxRadio
import com.dengon.app.ble.transport.TransportConfig
import com.dengon.app.ffi.DengonException
import com.dengon.app.ffi.OutgoingFrame
import com.dengon.app.ui.conversations.FauxNoeud
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * [Maillage] en JVM pur (US-306) : vrai [AndroidTransport] sur [FauxRadio],
 * faux nœud qui enregistre les appels. Un « ANNOUNCE » de test est la
 * chaîne `ANNOUNCE <peerID>` ; la vraie vérification (signature) est
 * couverte par `DengonNodeIntegrationTest`.
 */
class MaillageTest {

    private class NoeudEspion : FauxNoeud() {
        val connectes = mutableListOf<String>()
        val deconnectes = mutableListOf<String>()
        val recus = mutableListOf<Pair<String, String>>()
        val sortantes = mutableListOf<OutgoingFrame>()
        var refuser = false

        override fun onPeerConnected(peerId: String) {
            if (refuser) throw DengonException.UnknownPeer("refusé")
            connectes += peerId
        }

        override fun onPeerDisconnected(peerId: String) {
            deconnectes += peerId
        }

        override fun onBytesReceived(peerId: String, frame: ByteArray) {
            recus += peerId to String(frame)
        }

        override fun takeOutgoing(): List<OutgoingFrame> = sortantes.toList().also { sortantes.clear() }
    }

    private val radio = FauxRadio()
    private val transport = AndroidTransport(radio)
    private val noeud = NoeudEspion()
    private val journal = mutableListOf<String>()
    private val maillage = Maillage(
        transport,
        noeud,
        lireAnnonce = { octets ->
            String(octets).takeIf { it.startsWith("ANNOUNCE ") }?.let { AnnonceLue(it.removePrefix("ANNOUNCE "), null) }
        },
        journal = { journal += it },
    )

    @Before
    fun demarrer() {
        transport.start(TransportConfig(localPeerId = ByteArray(8)))
    }

    private fun tour() = maillage.traiter(transport.poll())

    @Test
    fun `un lien qui s ouvre recoit notre ANNOUNCE`() {
        val pair = radio.nouveauPair()
        radio.connecter(pair)
        tour()
        assertArrayEquals(noeud.announceFrame(), radio.trameEcrite(pair))
        assertTrue("pas encore identifié", maillage.pairs.isEmpty())
    }

    @Test
    fun `l ANNOUNCE du pair relie le lien et connecte le noeud`() {
        val pair = radio.nouveauPair()
        radio.connecter(pair)
        radio.faireRecevoir(pair, "ANNOUNCE bob".toByteArray())
        tour()
        assertEquals(listOf("bob"), noeud.connectes)
        assertEquals("bob", maillage.pairs[transport.lienPour(pair)])

        radio.faireRecevoir(pair, "paquet".toByteArray())
        tour()
        assertEquals(listOf("bob" to "paquet"), noeud.recus)
    }

    @Test
    fun `le pseudo de l ANNOUNCE est retenu tant que le lien vit`() {
        val radio = FauxRadio()
        val transport = AndroidTransport(radio)
        transport.start(TransportConfig(localPeerId = ByteArray(8)))
        val avecPseudo = Maillage(
            transport,
            noeud,
            lireAnnonce = { AnnonceLue("bob", "relay-3f2a9c") },
        )
        val pair = radio.nouveauPair()
        radio.connecter(pair)
        radio.faireRecevoir(pair, "ANNOUNCE bob".toByteArray())
        avecPseudo.traiter(transport.poll())
        assertEquals(mapOf("bob" to "relay-3f2a9c"), avecPseudo.pseudos)

        radio.couper(pair, DisconnectReason.BRUTALE)
        avecPseudo.traiter(transport.poll())
        assertTrue(avecPseudo.pseudos.isEmpty())
    }

    @Test
    fun `une premiere trame qui n est pas un ANNOUNCE est jetee`() {
        val pair = radio.nouveauPair()
        radio.connecter(pair)
        radio.faireRecevoir(pair, "paquet".toByteArray())
        tour()
        assertTrue(noeud.connectes.isEmpty())
        assertTrue(noeud.recus.isEmpty())
        assertTrue(maillage.pairs.isEmpty())
    }

    @Test
    fun `la sortie du noeud part sur le lien du bon pair`() {
        val bob = radio.nouveauPair()
        val carol = radio.nouveauPair()
        radio.connecter(bob)
        radio.connecter(carol)
        radio.faireRecevoir(bob, "ANNOUNCE bob".toByteArray())
        radio.faireRecevoir(carol, "ANNOUNCE carol".toByteArray())
        tour()

        noeud.sortantes += OutgoingFrame("carol", "pour carol".toByteArray())
        noeud.sortantes += OutgoingFrame("dave", "pour dave".toByteArray())
        maillage.vider()

        assertEquals("pour carol", String(radio.trameEcrite(carol)!!))
        assertArrayEquals("bob n'a reçu que l'ANNOUNCE", noeud.announceFrame(), radio.trameEcrite(bob))
        assertTrue(journal.any { "dave" in it })
    }

    @Test
    fun `la deconnexion d un lien identifie deconnecte le pair`() {
        val pair = radio.nouveauPair()
        radio.connecter(pair)
        radio.faireRecevoir(pair, "ANNOUNCE bob".toByteArray())
        tour()
        radio.couper(pair, DisconnectReason.BRUTALE)
        tour()
        assertEquals(listOf("bob"), noeud.deconnectes)
        assertTrue(maillage.pairs.isEmpty())

        noeud.sortantes += OutgoingFrame("bob", "trop tard".toByteArray())
        maillage.vider() // ne lève pas : trame jetée, rejouée par le cœur
    }

    @Test
    fun `un lien non identifie qui tombe ne touche pas le noeud`() {
        val pair = radio.nouveauPair()
        radio.connecter(pair)
        radio.couper(pair, DisconnectReason.PROPRE)
        tour()
        assertTrue(noeud.deconnectes.isEmpty())
    }

    @Test
    fun `un second lien vers le meme pair est ignore`() {
        val premier = radio.nouveauPair()
        val second = radio.nouveauPair(com.dengon.app.ble.transport.RadioPeer.Role.CENTRAL)
        radio.connecter(premier)
        radio.connecter(second)
        radio.faireRecevoir(premier, "ANNOUNCE bob".toByteArray())
        radio.faireRecevoir(second, "ANNOUNCE bob".toByteArray())
        tour()
        assertEquals(listOf("bob"), noeud.connectes)
        assertEquals(transport.lienPour(premier), maillage.pairs.keys.single())
        assertNull(maillage.pairs[transport.lienPour(second)])
    }

    @Test
    fun `un refus du noeud est journalise sans casser la boucle`() {
        noeud.refuser = true
        val pair = radio.nouveauPair()
        radio.connecter(pair)
        radio.faireRecevoir(pair, "ANNOUNCE bob".toByteArray())
        tour()
        assertTrue(journal.any { "refusé par le nœud" in it })
    }
}
