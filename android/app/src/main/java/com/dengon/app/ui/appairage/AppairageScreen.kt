package com.dengon.app.ui.appairage

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.dengon.app.R
import com.dengon.app.ui.composants.EnTeteEcran
import com.dengon.app.ui.theme.CibleTactileMin
import com.google.zxing.common.BitMatrix
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

private const val NOMBRE_ETAPES = 3

/** Numéro (1 à 3) de l'étape affichée : montrer mon code, comparer, conclure. */
private fun numeroEtape(etape: EtapeAppairage): Int = when (etape) {
    EtapeAppairage.AfficherMonQr -> 1
    is EtapeAppairage.Comparaison -> 2
    is EtapeAppairage.Verifie, is EtapeAppairage.Refuse -> 3
}

/**
 * Écran d'appairage (US-215) : mon QR + scan du QR de l'autre, puis
 * comparaison du code de 60 chiffres. Tout l'état vit dans
 * [AppairageViewModel] ; cet écran ne fait qu'afficher et relayer les clics.
 *
 * US-321 : parcours expliqué en 3 étapes numérotées (« Étape 1 sur 3 »),
 * chacune avec un titre, une consigne et une seule action principale.
 */
@Composable
fun AppairageScreen(viewModel: AppairageViewModel, onRetour: () -> Unit) {
    val etat by viewModel.etat.collectAsState()
    val scanner = rememberLauncherForActivityResult(ScanContract()) { resultat ->
        viewModel.onQrScanne(resultat.contents)
    }
    val invite = stringResource(R.string.appairage_scan_invite)
    val lancerScan = {
        viewModel.effacerErreur()
        scanner.launch(
            ScanOptions()
                .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
                .setPrompt(invite)
                .setBeepEnabled(false)
                .setOrientationLocked(false),
        )
    }

    Column(modifier = Modifier.fillMaxSize()) {
        EnTeteEcran(titre = stringResource(R.string.appairage_titre), onRetour = onRetour)
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp, vertical = 8.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            val numero = numeroEtape(etat.etape)
            Text(
                text = stringResource(R.string.appairage_etape, numero, NOMBRE_ETAPES),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
            )
            // Barre décorative : le texte « Étape N sur 3 » porte l'information.
            LinearProgressIndicator(
                progress = { numero.toFloat() / NOMBRE_ETAPES },
                modifier = Modifier.fillMaxWidth().clearAndSetSemantics { },
            )

            when (val etape = etat.etape) {
                EtapeAppairage.AfficherMonQr -> MonQr(etat, onScanner = lancerScan)
                is EtapeAppairage.Comparaison -> Comparaison(
                    etape,
                    monQr = etat.monQr,
                    onIdentiques = viewModel::confirmer,
                    onDifferents = viewModel::refuser,
                )
                is EtapeAppairage.Verifie -> Resultat(
                    message = stringResource(R.string.appairage_verifie, etape.contact.pseudo),
                    onRecommencer = viewModel::recommencer,
                    onTerminer = onRetour,
                )
                is EtapeAppairage.Refuse -> Resultat(
                    message = stringResource(R.string.appairage_refuse, etape.pseudo),
                    onRecommencer = viewModel::recommencer,
                    onTerminer = onRetour,
                    alerte = true,
                )
            }

            etat.erreur?.let { erreur ->
                Text(
                    text = stringResource(
                        when (erreur) {
                            ErreurAppairage.QrInvalide -> R.string.appairage_erreur_qr_invalide
                            ErreurAppairage.PropreQr -> R.string.appairage_erreur_propre_qr
                        },
                    ),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.error,
                    textAlign = TextAlign.Center,
                    // Annoncée par TalkBack dès qu'elle apparaît.
                    modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
                )
            }
        }
    }
}

@Composable
private fun MonQr(etat: AppairageEtat, onScanner: () -> Unit) {
    Text(
        text = stringResource(R.string.appairage_etape1_titre),
        style = MaterialTheme.typography.headlineSmall,
        textAlign = TextAlign.Center,
    )
    Text(
        text = stringResource(R.string.appairage_consigne),
        style = MaterialTheme.typography.bodyLarge,
        textAlign = TextAlign.Center,
    )
    Text(
        text = stringResource(R.string.appairage_mon_qr, etat.monPseudo),
        style = MaterialTheme.typography.titleMedium,
    )
    ImageQr(contenu = etat.monQr, description = stringResource(R.string.appairage_mon_qr_description))
    Button(onClick = onScanner, modifier = Modifier.fillMaxWidth().heightIn(min = CibleTactileMin)) {
        Text(stringResource(R.string.appairage_scanner))
    }
    Text(
        text = stringResource(R.string.appairage_aide_camera),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
    )

    if (etat.contactsVerifies.isNotEmpty()) {
        Spacer(Modifier.height(8.dp))
        Text(
            text = stringResource(R.string.appairage_contacts_verifies),
            style = MaterialTheme.typography.titleSmall,
        )
        etat.contactsVerifies.forEach { contact ->
            Text(stringResource(R.string.appairage_contact_verifie, contact.pseudo))
        }
    }
}

@Composable
private fun Comparaison(
    etape: EtapeAppairage.Comparaison,
    monQr: String,
    onIdentiques: () -> Unit,
    onDifferents: () -> Unit,
) {
    Text(
        text = stringResource(R.string.appairage_etape2_titre),
        style = MaterialTheme.typography.headlineSmall,
        textAlign = TextAlign.Center,
    )
    Text(
        text = stringResource(R.string.appairage_comparer, etape.distant.pseudo),
        style = MaterialTheme.typography.bodyLarge,
        textAlign = TextAlign.Center,
    )
    Column(
        modifier = Modifier.semantics {
            contentDescription = "Code de vérification ${etape.code}"
        },
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        lignesCode(etape.code).forEach { ligne ->
            Text(
                text = ligne,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.Bold,
                // `sp` : suit la taille de police du système ; le texte passe à la ligne au besoin.
                fontSize = 22.sp,
                letterSpacing = 1.sp,
                textAlign = TextAlign.Center,
            )
        }
    }
    Text(
        text = stringResource(R.string.appairage_lecture_croisee),
        textAlign = TextAlign.Center,
        style = MaterialTheme.typography.bodyMedium,
    )
    Button(onClick = onIdentiques, modifier = Modifier.fillMaxWidth().heightIn(min = CibleTactileMin)) {
        Text(stringResource(R.string.appairage_codes_identiques))
    }
    OutlinedButton(onClick = onDifferents, modifier = Modifier.fillMaxWidth().heightIn(min = CibleTactileMin)) {
        Text(stringResource(R.string.appairage_codes_differents))
    }

    // Lecture croisée : l'autre téléphone doit encore scanner MON QR pour
    // afficher le code. Sans ce rappel, mon QR disparaissait dès mon scan
    // (constaté sur deux vrais téléphones) et l'autre n'avait rien à viser.
    Spacer(Modifier.height(8.dp))
    Text(
        text = stringResource(R.string.appairage_rappel_mon_qr, etape.distant.pseudo),
        textAlign = TextAlign.Center,
        style = MaterialTheme.typography.bodyMedium,
    )
    ImageQr(contenu = monQr, description = stringResource(R.string.appairage_mon_qr_description))
}

@Composable
private fun Resultat(
    message: String,
    onRecommencer: () -> Unit,
    onTerminer: () -> Unit,
    alerte: Boolean = false,
) {
    Text(
        text = message,
        textAlign = TextAlign.Center,
        style = MaterialTheme.typography.headlineSmall,
        color = if (alerte) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
    )
    Button(onClick = onTerminer, modifier = Modifier.fillMaxWidth().heightIn(min = CibleTactileMin)) {
        Text(stringResource(R.string.appairage_terminer))
    }
    OutlinedButton(onClick = onRecommencer, modifier = Modifier.fillMaxWidth().heightIn(min = CibleTactileMin)) {
        Text(stringResource(R.string.appairage_autre_contact))
    }
}

/**
 * Dessine le QR de `contenu` : modules noirs sur fond blanc, quel que soit le
 * thème (un QR clair sur fond sombre n'est pas lisible par tous les lecteurs).
 * `matriceQr` encode+rastérise+décode jusqu'à 9 fois (revue PR #94) :
 * calculée sur `Dispatchers.Default`, pas sur le thread UI qui compose cet
 * écran.
 */
@Composable
private fun ImageQr(contenu: String, description: String) {
    val matrice by produceState<BitMatrix?>(initialValue = null, contenu) {
        value = withContext(Dispatchers.Default) { matriceQr(contenu) }
    }
    Canvas(
        modifier = Modifier
            .size(260.dp)
            .background(Color.White)
            .semantics { contentDescription = description },
    ) {
        val matriceActuelle = matrice ?: return@Canvas
        val module = size.minDimension / matriceActuelle.width
        for (y in 0 until matriceActuelle.height) {
            for (x in 0 until matriceActuelle.width) {
                if (matriceActuelle[x, y]) {
                    drawRect(
                        color = Color.Black,
                        topLeft = Offset(x * module, y * module),
                        // +0.5 px : évite les liserés blancs dus à l'arrondi entre modules.
                        size = Size(module + 0.5f, module + 0.5f),
                    )
                }
            }
        }
    }
}
