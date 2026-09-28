package com.dengon.app.ble.transport

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

/**
 * Suite de conformité `Transport` (US-105) appliquée à [AndroidTransport].
 *
 * **Transcription un pour un** de `crates/dengon-ble/src/conformance.rs` :
 * mêmes 12 cas, mêmes noms (`cas_…`), mêmes messages d'échec. La suite Rust
 * ne peut piloter un objet Kotlin qu'à travers une *callback interface*
 * UniFFI (US-302) ; en attendant, cette transcription est l'écart assumé
 * (`docs/suivi/03-ecarts-conception.md`, US-213). À l'US-302, ce fichier
 * disparaît au profit de la suite Rust elle-même.
 *
 * [Banc] est l'équivalent de `BancDEssai` : il provoque connexions, coupures
 * et arrivées de trames par la [FauxRadio], qui fragmente vraiment les trames.
 */
class AndroidTransportConformiteTest {

    /** Équivalent Kotlin de `BancDEssai`. */
    private class Banc {
        lateinit var radio: FauxRadio

        fun nouveau(): AndroidTransport {
            radio = FauxRadio()
            return AndroidTransport(radio)
        }

        fun connecterUnPair(t: AndroidTransport): LinkId {
            val pair = radio.nouveauPair()
            radio.connecter(pair)
            return t.lienPour(pair) ?: error("banc : la connexion n'a pas ouvert de lien")
        }

        fun couper(t: AndroidTransport, lien: LinkId, motif: DisconnectReason) {
            radio.couper(pairDe(t, lien), motif)
        }

        fun faireRecevoir(t: AndroidTransport, lien: LinkId, bytes: ByteArray) {
            // Un lien fermé n'a plus de pair côté transport : on vise alors la
            // dernière connexion connue, comme un rappel radio tardif.
            val pair = radio.connectees.firstOrNull { t.lienPour(it) == lien } ?: dernierPair
            radio.faireRecevoir(pair ?: return, bytes)
        }

        fun config() = TransportConfig(localPeerId = ByteArray(8))

        private var dernierPair: RadioPeer? = null

        private fun pairDe(t: AndroidTransport, lien: LinkId): RadioPeer =
            radio.connectees.first { t.lienPour(it) == lien }.also { dernierPair = it }
    }

    private val banc = Banc()

    private fun demarre(): AndroidTransport {
        val t = banc.nouveau()
        try {
            t.start(banc.config())
        } catch (e: TransportException) {
            fail("conformité : start() sur un transport neuf doit réussir")
        }
        return t
    }

    @Test
    fun cas_poll_avant_start_est_vide() {
        val t = banc.nouveau()
        assertTrue(
            "conformité : poll() avant start() doit rendre un Vec vide, pas paniquer",
            t.poll().isEmpty(),
        )
    }

    @Test
    fun cas_envoi_avant_start_est_refuse() {
        val t = banc.nouveau()
        assertThrows("conformité : send() avant start() doit rendre NotStarted", TransportException.NotStarted::class.java) {
            t.send(LinkId(0), "x".toByteArray())
        }
        assertThrows("conformité : broadcast() avant start() doit rendre NotStarted", TransportException.NotStarted::class.java) {
            t.broadcast("x".toByteArray())
        }
    }

    @Test
    fun cas_double_start_est_refuse() {
        val t = demarre()
        assertThrows("conformité : un second start() doit rendre AlreadyStarted", TransportException.AlreadyStarted::class.java) {
            t.start(banc.config())
        }
    }

    @Test
    fun cas_connexion_remonte_un_evenement() {
        val t = demarre()
        val lien = banc.connecterUnPair(t)
        val vu = t.poll().any { it is TransportEvent.PeerConnected && it.peerLinkId == lien }
        assertTrue("conformité : une connexion doit produire un PeerConnected portant son LinkId", vu)
    }

    @Test
    fun cas_poll_consomme_les_evenements() {
        val t = demarre()
        banc.connecterUnPair(t)
        assertFalse("conformité : le premier poll() doit livrer l'événement de connexion", t.poll().isEmpty())
        assertTrue("conformité : poll() consomme — le même événement ne doit pas ressortir", t.poll().isEmpty())
    }

    @Test
    fun cas_trame_recue_remonte_intacte() {
        val t = demarre()
        val lien = banc.connecterUnPair(t)
        t.poll()

        val charge = "dengon-conformite".toByteArray()
        banc.faireRecevoir(t, lien, charge)

        val vu = t.poll().any { it is TransportEvent.FrameReceived && it.peerLinkId == lien && it.bytes.contentEquals(charge) }
        assertTrue("conformité : une trame reçue doit remonter intacte, sur le lien qui l'a livrée", vu)
    }

    @Test
    fun cas_envoi_vers_un_pair_connecte_reussit() {
        val t = demarre()
        val lien = banc.connecterUnPair(t)
        try {
            t.send(lien, "charge".toByteArray())
        } catch (e: TransportException) {
            fail("conformité : send() vers un lien ouvert doit réussir")
        }
    }

    @Test
    fun cas_broadcast_sans_pair_reussit() {
        val t = demarre()
        try {
            t.broadcast("personne".toByteArray())
        } catch (e: TransportException) {
            fail("conformité : broadcast() sans pair connecté est un succès, pas une erreur")
        }
    }

    @Test
    fun cas_deconnexion_brutale() {
        val t = demarre()
        val lien = banc.connecterUnPair(t)
        t.poll()

        banc.couper(t, lien, DisconnectReason.BRUTALE)

        val ferme = t.poll().any {
            it is TransportEvent.PeerDisconnected && it.peerLinkId == lien && it.reason == DisconnectReason.BRUTALE
        }
        assertTrue("conformité : une coupure brutale doit produire un PeerDisconnected portant DisconnectReason::Brutale", ferme)

        val erreur = assertThrows(
            "conformité : send() sur un lien mort doit rendre UnknownPeer, sans paniquer",
            TransportException.UnknownPeer::class.java,
        ) { t.send(lien, "trop tard".toByteArray()) }
        assertEquals(lien, erreur.link)

        banc.faireRecevoir(t, lien, "fantome".toByteArray())
        val fantome = t.poll().any { it is TransportEvent.FrameReceived && it.peerLinkId == lien }
        assertFalse("conformité : plus aucun événement ne doit porter un LinkId fermé", fantome)
    }

    @Test
    fun cas_trame_recue_avant_coupure_est_livree() {
        val t = demarre()
        val lien = banc.connecterUnPair(t)
        t.poll()

        val charge = "avant-la-coupure".toByteArray()
        banc.faireRecevoir(t, lien, charge)
        banc.couper(t, lien, DisconnectReason.BRUTALE)

        val evenements = t.poll()
        val rangTrame = evenements.indexOfFirst {
            it is TransportEvent.FrameReceived && it.peerLinkId == lien && it.bytes.contentEquals(charge)
        }
        val rangFermeture = evenements.indexOfFirst { it is TransportEvent.PeerDisconnected && it.peerLinkId == lien }

        if (rangTrame < 0) fail("conformité : une trame reçue avant la coupure ne doit pas être perdue — elle était complète et valide")
        if (rangFermeture < 0) fail("conformité : une coupure brutale doit produire un PeerDisconnected")
        assertTrue(
            "conformité : la trame reçue avant la coupure doit être livrée *avant* l'événement de fermeture du lien",
            rangTrame < rangFermeture,
        )
    }

    @Test
    fun cas_deconnexion_propre_est_distinguee() {
        val t = demarre()
        val lien = banc.connecterUnPair(t)
        t.poll()

        banc.couper(t, lien, DisconnectReason.PROPRE)

        val propre = t.poll().any {
            it is TransportEvent.PeerDisconnected && it.peerLinkId == lien && it.reason == DisconnectReason.PROPRE
        }
        assertTrue("conformité : une déconnexion propre doit être signalée comme telle", propre)
    }

    @Test
    fun cas_link_id_jamais_reutilise() {
        val t = demarre()
        val premier = banc.connecterUnPair(t)
        banc.couper(t, premier, DisconnectReason.BRUTALE)
        t.poll()

        val second = banc.connecterUnPair(t)
        assertNotEquals("conformité : réutiliser un LinkId attribuerait des trames au mauvais pair", premier, second)
        assertNotNull(second)
    }
}
