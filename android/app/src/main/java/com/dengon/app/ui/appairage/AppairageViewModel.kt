package com.dengon.app.ui.appairage

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import com.dengon.app.ffi.DengonException
import com.dengon.app.ffi.Identity
import com.dengon.app.ffi.identityFromQrCode
import com.dengon.app.ffi.identityQrCode
import com.dengon.app.ffi.verificationCode
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/**
 * Appairage vérifié d'un contact (US-215, scénario 1 du DoD) :
 * `docs/powl/04-security.md` §2.2-2.3.
 *
 * 1. J'affiche **mon** QR ; mon correspondant le scanne.
 * 2. Je scanne **le sien** ([onQrScanne]).
 * 3. Les deux téléphones affichent le même code de 60 chiffres ; on les
 *    compare à voix haute (lecture croisée) et **je confirme explicitement**
 *    ([confirmer]) ou je signale une différence ([refuser]).
 *
 * Alimenté exclusivement par le bouchon FFI de US-106 (`identityQrCode`,
 * `identityFromQrCode`, `verificationCode`) : même surface que les bindings
 * UniFFI, branchement réel à l'US-306.
 *
 * Synchrone, comme `ConversationsViewModel` (US-214) : le bouchon calcule en
 * mémoire, donc pas de coroutine à tester.
 */
class AppairageViewModel(private val local: Identity) : ViewModel() {

    private val etatMutable = MutableStateFlow(
        AppairageEtat(monPseudo = local.pseudo, monQr = identityQrCode(local)),
    )

    /** État observé par l'écran. */
    val etat: StateFlow<AppairageEtat> = etatMutable.asStateFlow()

    /**
     * Résultat du scan. `null` = scan annulé par l'utilisateur : rien ne
     * change. Un QR qui n'est pas un QR dengon, ou mon propre QR, affiche une
     * erreur sans quitter l'étape en cours.
     */
    fun onQrScanne(contenu: String?) {
        if (contenu == null) return
        val distant = try {
            identityFromQrCode(contenu)
        } catch (e: DengonException) {
            etatMutable.update { it.copy(erreur = ErreurAppairage.QrInvalide) }
            return
        }
        if (distant.peerId == local.peerId) {
            etatMutable.update { it.copy(erreur = ErreurAppairage.PropreQr) }
            return
        }
        etatMutable.update {
            it.copy(
                etape = EtapeAppairage.Comparaison(distant, verificationCode(local, distant)),
                erreur = null,
            )
        }
    }

    /** Les deux codes sont identiques : le contact est marqué vérifié. */
    fun confirmer() {
        val comparaison = etatMutable.value.etape as? EtapeAppairage.Comparaison ?: return
        val contact = ContactVerifie(comparaison.distant.peerId, comparaison.distant.pseudo)
        etatMutable.update { etat ->
            etat.copy(
                etape = EtapeAppairage.Verifie(contact),
                contactsVerifies = etat.contactsVerifies.filterNot { it.peerId == contact.peerId } + contact,
            )
        }
    }

    /**
     * Les codes diffèrent : possible interception au premier contact (MITM,
     * `powl/04` §1). Le contact n'est **pas** enregistré.
     */
    fun refuser() {
        val comparaison = etatMutable.value.etape as? EtapeAppairage.Comparaison ?: return
        etatMutable.update { it.copy(etape = EtapeAppairage.Refuse(comparaison.distant.pseudo)) }
    }

    /** Retour à l'affichage de mon QR, pour appairer un autre contact. */
    fun recommencer() {
        etatMutable.update { it.copy(etape = EtapeAppairage.AfficherMonQr, erreur = null) }
    }

    /** L'utilisateur a lu le message d'erreur. */
    fun effacerErreur() {
        etatMutable.update { it.copy(erreur = null) }
    }

    companion object {
        /** Fabrique pour `by viewModels { … }` : injecte l'identité locale. */
        fun fabrique(local: Identity): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T = AppairageViewModel(local) as T
        }
    }
}

/** État complet de l'écran d'appairage. */
data class AppairageEtat(
    val monPseudo: String,
    /** Contenu de mon QR : `dengon:v1:<base64url>` (`powl/04` §2.2). */
    val monQr: String,
    val etape: EtapeAppairage = EtapeAppairage.AfficherMonQr,
    val erreur: ErreurAppairage? = null,
    /**
     * Contacts vérifiés pendant la session. Gardés en mémoire : le contrat
     * FFI v0 n'a pas encore d'appel « marquer vérifié »
     * (`contacts.verified_at`, `powl/04` §2.3) — voir les écarts.
     */
    val contactsVerifies: List<ContactVerifie> = emptyList(),
)

/** Étapes de l'appairage. */
sealed interface EtapeAppairage {
    /** Mon QR est affiché ; j'attends de scanner celui du correspondant. */
    data object AfficherMonQr : EtapeAppairage

    /** QR du correspondant lu : comparer le code de 60 chiffres. */
    data class Comparaison(val distant: Identity, val code: String) : EtapeAppairage

    /** Codes identiques, confirmés. */
    data class Verifie(val contact: ContactVerifie) : EtapeAppairage

    /** Codes différents : contact rejeté. */
    data class Refuse(val pseudo: String) : EtapeAppairage
}

/** Erreurs affichées sans changer d'étape. */
enum class ErreurAppairage {
    /** Le QR scanné n'est pas un QR d'identité dengon. */
    QrInvalide,

    /** J'ai scanné mon propre QR. */
    PropreQr,
}

/** Contact dont le code a été comparé et confirmé. */
data class ContactVerifie(val peerId: String, val pseudo: String)
