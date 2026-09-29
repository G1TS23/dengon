package com.dengon.app

import android.content.Context
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AccountBox
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.LocationOn
import androidx.compose.material.icons.filled.MailOutline
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import com.dengon.app.ble.BlePermissions
import com.dengon.app.ble.MeshForegroundService
import com.dengon.app.ble.spike.HelloMeshSpikeScreen
import com.dengon.app.ble.transport.TransportActif
import com.dengon.app.ble.transport.TransportDebugScreen
import com.dengon.app.ui.appairage.AppairageScreen
import com.dengon.app.ui.appairage.AppairageViewModel
import com.dengon.app.ui.composants.BandeauAlerte
import com.dengon.app.ui.composants.ouvrirReglagesBluetooth
import com.dengon.app.ui.composants.rememberBluetoothActif
import com.dengon.app.ui.conversations.ConversationsViewModel
import com.dengon.app.ui.conversations.MessagerieRoute
import com.dengon.app.ui.reseau.ReseauRoute
import com.dengon.app.ui.theme.CibleTactileMin
import com.dengon.app.ui.theme.DengonTheme

class MainActivity : ComponentActivity() {

    private var permissionsGranted = mutableStateOf(false)

    // Écran d'appairage ouvert : aussi piloté par la carte de debug (US-312).
    private val afficherAppairage = mutableStateOf(false)

    // `true` une fois que l'utilisateur a refusé la demande : Android ne
    // réaffiche alors plus la boîte de dialogue, il faut passer par les réglages.
    private var permissionsRefusees = mutableStateOf(false)

    // Le vrai nœud `dengon-core` (US-302), unique pour le processus.
    private val noeud get() = (application as DengonApplication).noeud

    // Messagerie (US-214) sur le vrai nœud. Le ViewModel survit aux rotations
    // d'écran. Chaque envoi part aussitôt sur la radio (US-306).
    private val conversationsViewModel: ConversationsViewModel by viewModels {
        ConversationsViewModel.fabrique(noeud, apresEnvoi = TransportActif::vider)
    }

    // Appairage (US-215) : un contact confirmé devient un correspondant du
    // nœud, sans quoi `sendMessage` le refuserait (`UnknownPeer`).
    private val appairage: AppairageViewModel by viewModels {
        AppairageViewModel.fabrique(noeud.localIdentity(), onContactVerifie = noeud::addContact)
    }

    private val requestPermissions =
        registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { results ->
            val toutes = results.values.all { it }
            permissionsGranted.value = toutes
            permissionsRefusees.value = !toutes
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        permissionsGranted.value = BlePermissions.allGranted(this)
        if (debuggable) {
            Log.i(TAG_DEBUG, "carte locale : ${appairage.etat.value.monQr}")
        }
        traiterCarteDebug(intent)

        setContent {
            DengonApp(
                conversationsViewModel = conversationsViewModel,
                appairage = appairage,
                afficherAppairage = afficherAppairage,
                permissionsGranted = permissionsGranted,
                permissionsRefusees = permissionsRefusees,
                onRequestPermissions = { requestPermissions.launch(BlePermissions.required()) },
                onStartService = ::startMeshService,
                onStopService = ::stopMeshService,
            )
        }
    }

    override fun onResume() {
        super.onResume()
        // Retour des réglages Android : la permission a pu être accordée entre-temps.
        permissionsGranted.value = BlePermissions.allGranted(this)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        traiterCarteDebug(intent)
    }

    private val debuggable: Boolean
        get() = (applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) != 0

    /**
     * Essai sur matériel sans caméra (US-312), **build debug seulement** :
     * `adb shell am start -n com.dengon.app/.MainActivity --es dengon.carte_debug
     * 'dengon:v1:…'` fait comme si ce QR venait d'être scanné. Le code à
     * 60 chiffres s'affiche et la confirmation reste manuelle. Ignoré sur un
     * build de release (écart consigné).
     */
    private fun traiterCarteDebug(intent: Intent?) {
        if (!debuggable) return
        val carte = intent?.getStringExtra(EXTRA_CARTE_DEBUG) ?: return
        Log.i(TAG_DEBUG, "carte reçue par intent (debug)")
        appairage.onQrScanne(carte)
        afficherAppairage.value = true
    }

    private companion object {
        const val EXTRA_CARTE_DEBUG = "dengon.carte_debug"
        const val TAG_DEBUG = "dengon-appairage"
    }

    private fun startMeshService() {
        val intent = Intent(this, MeshForegroundService::class.java)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            ContextCompat.startForegroundService(this, intent)
        } else {
            startService(intent)
        }
    }

    private fun stopMeshService() {
        stopService(Intent(this, MeshForegroundService::class.java))
    }
}

@Composable
private fun DengonApp(
    conversationsViewModel: ConversationsViewModel,
    appairage: AppairageViewModel,
    afficherAppairage: MutableState<Boolean>,
    permissionsGranted: MutableState<Boolean>,
    permissionsRefusees: MutableState<Boolean>,
    onRequestPermissions: () -> Unit,
    onStartService: () -> Unit,
    onStopService: () -> Unit,
) {
    val granted by permissionsGranted
    val refusees by permissionsRefusees
    // Reflète l'état réel de `MeshForegroundService` (US-324) : un booléen
    // local ne voit jamais un arrêt déclenché hors de l'app (notification
    // système), ce qui désynchronisait l'interrupteur et empêchait de
    // relancer le transport (voir issue #132).
    val etatTransport by TransportActif.etat.collectAsState()
    val serviceRunning = etatTransport.demarre
    var showSpike by remember { mutableStateOf(false) }
    var showMessagerie by remember { mutableStateOf(false) }
    var showAppairage by afficherAppairage
    var showTransport by remember { mutableStateOf(false) }
    var showReseau by remember { mutableStateOf(false) }

    // Démarrage auto dès que les permissions sont accordées (une seule fois
    // par passage à `true`, pas à chaque recomposition). `demarrer()` est
    // idempotent côté service, donc sans risque si déjà en cours.
    LaunchedEffect(granted) {
        if (granted && !serviceRunning) {
            onStartService()
        }
    }

    DengonTheme {
        Surface(modifier = Modifier.fillMaxSize()) {
            if (showMessagerie) {
                MessagerieRoute(
                    viewModel = conversationsViewModel,
                    onQuitter = { showMessagerie = false },
                    onAjouterContact = {
                        showMessagerie = false
                        showAppairage = true
                    },
                )
            } else if (showSpike) {
                HelloMeshSpikeScreen(onBack = { showSpike = false })
            } else if (showAppairage) {
                AppairageScreen(viewModel = appairage, onRetour = { showAppairage = false })
            } else if (showReseau) {
                ReseauRoute(onRetour = { showReseau = false })
            } else if (showTransport) {
                TransportDebugScreen(onRetour = { showTransport = false })
            } else {
                DengonScreen(
                    permissionsGranted = granted,
                    permissionsRefusees = refusees,
                    serviceRunning = serviceRunning,
                    onRequestPermissions = onRequestPermissions,
                    onToggleService = {
                        if (serviceRunning) {
                            onStopService()
                        } else {
                            onStartService()
                        }
                    },
                    onOpenSpike = { showSpike = true },
                    onOpenMessagerie = { showMessagerie = true },
                    onOpenAppairage = { showAppairage = true },
                    onOpenTransport = { showTransport = true },
                    onOpenReseau = { showReseau = true },
                )
            }
        }
    }
}

@Composable
private fun DengonScreen(
    permissionsGranted: Boolean,
    permissionsRefusees: Boolean,
    serviceRunning: Boolean,
    onRequestPermissions: () -> Unit,
    onToggleService: () -> Unit,
    onOpenSpike: () -> Unit,
    onOpenMessagerie: () -> Unit,
    onOpenAppairage: () -> Unit,
    onOpenTransport: () -> Unit,
    onOpenReseau: () -> Unit,
) {
    val contexte = LocalContext.current
    val bluetoothActif = rememberBluetoothActif()
    var outilsOuverts by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column {
            Text(stringResource(R.string.app_name), style = MaterialTheme.typography.headlineSmall)
            Text(
                "Des messages qui passent sans Internet, d'un téléphone à l'autre.",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        // États guidés : chacun dit ce qui bloque et propose l'action pour le lever.
        if (!permissionsGranted) {
            if (permissionsRefusees) {
                BandeauAlerte(
                    icone = Icons.Filled.Warning,
                    titre = "Autorisation refusée",
                    explication = stringResource(R.string.permissions_denied) +
                        " Ouvrez les réglages de dengon, puis « Autorisations » et accordez « Appareils à proximité ».",
                    libelleAction = "Ouvrir les réglages de l'app",
                    onAction = { ouvrirReglagesApp(contexte) },
                )
            } else {
                BandeauAlerte(
                    icone = Icons.Filled.LocationOn,
                    titre = stringResource(R.string.permissions_rationale_title),
                    explication = stringResource(R.string.permissions_rationale_body),
                    libelleAction = stringResource(R.string.permissions_grant_button),
                    onAction = onRequestPermissions,
                )
            }
        } else if (!bluetoothActif) {
            BandeauAlerte(
                icone = Icons.Filled.Warning,
                titre = "Bluetooth coupé",
                explication = "Allumez le Bluetooth pour envoyer et recevoir des messages.",
                libelleAction = "Ouvrir les réglages Bluetooth",
                onAction = { ouvrirReglagesBluetooth(contexte) },
            )
        }

        // Messagerie : accessible sans permissions BLE, puisqu'aucune radio
        // n'alimente encore le nœud (AndroidTransport, US-213 / US-306).
        CarteAction(
            icone = Icons.Filled.MailOutline,
            titre = "Conversations",
            description = "Lire et écrire à vos contacts.",
            onClick = onOpenMessagerie,
        )
        // L'appairage par QR ne dépend pas du Bluetooth : accessible même
        // sans les permissions BLE.
        CarteAction(
            icone = Icons.Filled.Add,
            titre = stringResource(R.string.appairage_titre),
            description = "Scannez le code d'un proche, en personne, pour pouvoir lui écrire.",
            onClick = onOpenAppairage,
        )
        if (permissionsGranted) {
            CarteAction(
                icone = Icons.Filled.AccountBox,
                titre = "Appareils à proximité",
                description = "Voir les téléphones et relais joignables, régler l'économie de batterie.",
                onClick = onOpenReseau,
            )
            LigneService(serviceRunning, onToggleService)
        }

        // Outils d'essai : repliés, ils ne s'adressent pas au grand public.
        TextButton(
            onClick = { outilsOuverts = !outilsOuverts },
            modifier = Modifier.heightIn(min = CibleTactileMin),
        ) {
            Icon(
                if (outilsOuverts) Icons.Filled.KeyboardArrowUp else Icons.Filled.KeyboardArrowDown,
                contentDescription = null,
            )
            Spacer(Modifier.width(8.dp))
            Text("Outils de développement")
        }
        if (outilsOuverts && permissionsGranted) {
            // Essais du transport réel sur deux téléphones (US-213).
            CarteAction(Icons.Filled.LocationOn, "Transport BLE (debug)", "Liens, pairs et journal radio.", onOpenTransport)
            // Écran de debug jetable (US-103) — voir ble/spike/HelloMeshSpikeScreen.kt.
            CarteAction(Icons.Filled.LocationOn, "Spike C : hello mesh (debug)", "Essai du premier échange.", onOpenSpike)
        } else if (outilsOuverts) {
            Text(
                "Les outils de développement demandent l'autorisation Bluetooth.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/** Grande carte cliquable : pictogramme, titre, phrase d'explication. */
@Composable
private fun CarteAction(icone: ImageVector, titre: String, description: String, onClick: () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth().clickable(role = Role.Button, onClick = onClick),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
    ) {
        Row(
            modifier = Modifier.padding(16.dp).heightIn(min = CibleTactileMin),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(icone, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
            Spacer(Modifier.width(16.dp))
            Column {
                Text(titre, style = MaterialTheme.typography.titleMedium)
                Text(
                    description,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

/** Interrupteur du service de fond, en toutes lettres (« Recevoir en arrière-plan »). */
@Composable
private fun LigneService(actif: Boolean, onChange: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = CibleTactileMin)
            .toggleable(value = actif, role = Role.Switch, onValueChange = { onChange() })
            .semantics { stateDescription = if (actif) "Activé" else "Désactivé" },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text("Recevoir en arrière-plan", style = MaterialTheme.typography.titleMedium)
            Text(
                if (actif) "Vous recevez vos messages même écran éteint." else "Vous ne recevrez rien tant que c'est éteint.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(Modifier.width(16.dp))
        // Le clic est géré par la ligne entière.
        Switch(checked = actif, onCheckedChange = null)
    }
}

private fun ouvrirReglagesApp(contexte: Context) {
    contexte.startActivity(
        Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", contexte.packageName, null))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
    )
}
