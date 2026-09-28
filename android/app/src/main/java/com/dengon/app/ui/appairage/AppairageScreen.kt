package com.dengon.app.ui.appairage

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.dengon.app.R
import com.google.zxing.common.BitMatrix
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Écran d'appairage (US-215) : mon QR + scan du QR de l'autre, puis
 * comparaison du code de 60 chiffres. Tout l'état vit dans
 * [AppairageViewModel] ; cet écran ne fait qu'afficher et relayer les clics.
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

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = onRetour) { Text(stringResource(R.string.appairage_retour)) }
            Text(
                text = stringResource(R.string.appairage_titre),
                style = MaterialTheme.typography.titleLarge,
            )
        }

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
                color = MaterialTheme.colorScheme.error,
                textAlign = TextAlign.Center,
            )
        }
    }
}

@Composable
private fun MonQr(etat: AppairageEtat, onScanner: () -> Unit) {
    Text(
        text = stringResource(R.string.appairage_mon_qr, etat.monPseudo),
        style = MaterialTheme.typography.titleMedium,
    )
    ImageQr(contenu = etat.monQr, description = stringResource(R.string.appairage_mon_qr_description))
    Text(
        text = stringResource(R.string.appairage_consigne),
        textAlign = TextAlign.Center,
    )
    Button(onClick = onScanner) { Text(stringResource(R.string.appairage_scanner)) }

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
        text = stringResource(R.string.appairage_comparer, etape.distant.pseudo),
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
                fontSize = 22.sp,
                letterSpacing = 1.sp,
            )
        }
    }
    Text(
        text = stringResource(R.string.appairage_lecture_croisee),
        textAlign = TextAlign.Center,
        style = MaterialTheme.typography.bodySmall,
    )
    Button(onClick = onIdentiques, modifier = Modifier.fillMaxWidth()) {
        Text(stringResource(R.string.appairage_codes_identiques))
    }
    OutlinedButton(onClick = onDifferents, modifier = Modifier.fillMaxWidth()) {
        Text(stringResource(R.string.appairage_codes_differents))
    }

    // Lecture croisée : l'autre téléphone doit encore scanner MON QR pour
    // afficher le code. Sans ce rappel, mon QR disparaissait dès mon scan
    // (constaté sur deux vrais téléphones) et l'autre n'avait rien à viser.
    Spacer(Modifier.height(8.dp))
    Text(
        text = stringResource(R.string.appairage_rappel_mon_qr, etape.distant.pseudo),
        textAlign = TextAlign.Center,
        style = MaterialTheme.typography.bodySmall,
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
        style = MaterialTheme.typography.titleMedium,
        color = if (alerte) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
    )
    Button(onClick = onTerminer) { Text(stringResource(R.string.appairage_terminer)) }
    OutlinedButton(onClick = onRecommencer) { Text(stringResource(R.string.appairage_autre_contact)) }
}

/**
 * Dessine le QR de `contenu` : modules noirs sur fond blanc, quel que soit le
 * thème. `matriceQr` encode+rastérise+décode jusqu'à 9 fois (revue PR #94) :
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
