package com.dengon.app.ui.appairage

import com.dengon.app.ffi.generateIdentity
import com.dengon.app.ffi.identityQrCode
import com.dengon.app.ffi.verificationCode
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Tests JVM de [AppairageViewModel] (US-215) : parcours complet de
 * l'appairage, erreurs de scan, lecture croisée (même code des deux côtés).
 * Alimentés uniquement par le bouchon FFI (US-106).
 */
class AppairageViewModelTest {

    private val alice = generateIdentity("alice")
    private val bob = generateIdentity("bob")

    private fun vmAlice() = AppairageViewModel(alice)

    @Test
    fun `au depart mon QR est affiche`() {
        val etat = vmAlice().etat.value
        assertEquals("alice", etat.monPseudo)
        assertEquals(identityQrCode(alice), etat.monQr)
        assertTrue(etat.monQr.startsWith("dengon:v1:"))
        assertEquals(EtapeAppairage.AfficherMonQr, etat.etape)
        assertNull(etat.erreur)
        assertTrue(etat.contactsVerifies.isEmpty())
    }

    @Test
    fun `scanner le QR de l autre affiche le code de 60 chiffres`() {
        val vm = vmAlice()
        vm.onQrScanne(identityQrCode(bob))

        val etape = vm.etat.value.etape as EtapeAppairage.Comparaison
        assertEquals(bob.peerId, etape.distant.peerId)
        assertEquals("bob", etape.distant.pseudo)
        assertTrue(etape.code, estCodeVerification(etape.code))
        assertEquals(60, etape.code.count { it.isDigit() })
    }

    @Test
    fun `lecture croisee - les deux telephones affichent le meme code`() {
        val chezAlice = AppairageViewModel(alice)
        val chezBob = AppairageViewModel(bob)
        chezAlice.onQrScanne(chezBob.etat.value.monQr)
        chezBob.onQrScanne(chezAlice.etat.value.monQr)

        val codeAlice = (chezAlice.etat.value.etape as EtapeAppairage.Comparaison).code
        val codeBob = (chezBob.etat.value.etape as EtapeAppairage.Comparaison).code
        assertEquals(codeAlice, codeBob)
        assertEquals(verificationCode(alice, bob), codeAlice)
    }

    @Test
    fun `codes identiques confirmes - contact verifie`() {
        val vm = vmAlice()
        vm.onQrScanne(identityQrCode(bob))
        vm.confirmer()

        val etat = vm.etat.value
        assertEquals(EtapeAppairage.Verifie(ContactVerifie(bob.peerId, "bob")), etat.etape)
        assertEquals(listOf(ContactVerifie(bob.peerId, "bob")), etat.contactsVerifies)
    }

    @Test
    fun `reverifier un contact ne le duplique pas`() {
        val vm = vmAlice()
        repeat(2) {
            vm.onQrScanne(identityQrCode(bob))
            vm.confirmer()
            vm.recommencer()
        }
        assertEquals(1, vm.etat.value.contactsVerifies.size)
        assertEquals(EtapeAppairage.AfficherMonQr, vm.etat.value.etape)
    }

    @Test
    fun `codes differents - contact rejete et non enregistre`() {
        val vm = vmAlice()
        vm.onQrScanne(identityQrCode(bob))
        vm.refuser()

        assertEquals(EtapeAppairage.Refuse("bob"), vm.etat.value.etape)
        assertTrue(vm.etat.value.contactsVerifies.isEmpty())
    }

    @Test
    fun `confirmer ou refuser sans comparaison en cours ne fait rien`() {
        val vm = vmAlice()
        vm.confirmer()
        vm.refuser()
        assertEquals(EtapeAppairage.AfficherMonQr, vm.etat.value.etape)
        assertTrue(vm.etat.value.contactsVerifies.isEmpty())
    }

    @Test
    fun `un QR qui n est pas un QR dengon affiche une erreur`() {
        val vm = vmAlice()
        for (contenu in listOf("https://exemple.org", "dengon:v1:", "dengon:v1:AAAA")) {
            vm.onQrScanne(contenu)
            assertEquals(contenu, ErreurAppairage.QrInvalide, vm.etat.value.erreur)
            assertEquals(EtapeAppairage.AfficherMonQr, vm.etat.value.etape)
        }
        vm.effacerErreur()
        assertNull(vm.etat.value.erreur)
    }

    @Test
    fun `scanner son propre QR est refuse`() {
        val vm = vmAlice()
        vm.onQrScanne(vm.etat.value.monQr)
        assertEquals(ErreurAppairage.PropreQr, vm.etat.value.erreur)
        assertEquals(EtapeAppairage.AfficherMonQr, vm.etat.value.etape)
    }

    @Test
    fun `scan annule - rien ne change`() {
        val vm = vmAlice()
        val avant = vm.etat.value
        vm.onQrScanne(null)
        assertEquals(avant, vm.etat.value)
    }

    @Test
    fun `un scan valide efface l erreur precedente`() {
        val vm = vmAlice()
        vm.onQrScanne("pas un qr dengon")
        vm.onQrScanne(identityQrCode(bob))
        assertNull(vm.etat.value.erreur)
        assertTrue(vm.etat.value.etape is EtapeAppairage.Comparaison)
    }

    @Test
    fun `la fabrique cree un ViewModel pour l identite donnee`() {
        val vm = AppairageViewModel.fabrique(bob).create(AppairageViewModel::class.java)
        assertEquals("bob", vm.etat.value.monPseudo)
    }
}
