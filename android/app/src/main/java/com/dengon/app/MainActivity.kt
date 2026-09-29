package com.dengon.app

import android.content.Intent
import android.content.pm.ApplicationInfo
import android.os.Build
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import com.dengon.app.ble.BlePermissions
import com.dengon.app.ble.MeshForegroundService
import com.dengon.app.ble.spike.HelloMeshSpikeScreen
import com.dengon.app.ble.transport.TransportActif
import com.dengon.app.ble.transport.TransportDebugScreen
import com.dengon.app.ui.appairage.AppairageScreen
import com.dengon.app.ui.appairage.AppairageViewModel
import com.dengon.app.ui.conversations.ConversationsViewModel
import com.dengon.app.ui.conversations.MessagerieRoute
import com.dengon.app.ui.reseau.ReseauRoute

class MainActivity : ComponentActivity() {

    private var permissionsGranted = mutableStateOf(false)

    // Écran d'appairage ouvert : aussi piloté par la carte de debug (US-312).
    private val afficherAppairage = mutableStateOf(false)

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
            permissionsGranted.value = results.values.all { it }
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
                onRequestPermissions = { requestPermissions.launch(BlePermissions.required()) },
                onStartService = ::startMeshService,
                onStopService = ::stopMeshService,
            )
        }
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
    onRequestPermissions: () -> Unit,
    onStartService: () -> Unit,
    onStopService: () -> Unit,
) {
    val granted by permissionsGranted
    var serviceRunning by remember { mutableStateOf(false) }
    var showSpike by remember { mutableStateOf(false) }
    var showMessagerie by remember { mutableStateOf(false) }
    var showAppairage by afficherAppairage
    var showTransport by remember { mutableStateOf(false) }
    var showReseau by remember { mutableStateOf(false) }

    // Démarrage auto dès que les permissions sont accordées (une
    // seule fois par passage à `true`, pas à chaque recomposition).
    LaunchedEffect(granted) {
        if (granted && !serviceRunning) {
            onStartService()
            serviceRunning = true
        }
    }

    MaterialTheme {
        Surface(modifier = Modifier.fillMaxSize()) {
            if (showMessagerie) {
                MessagerieRoute(viewModel = conversationsViewModel, onQuitter = { showMessagerie = false })
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
                    serviceRunning = serviceRunning,
                    onRequestPermissions = onRequestPermissions,
                    onToggleService = {
                        if (serviceRunning) {
                            onStopService()
                        } else {
                            onStartService()
                        }
                        serviceRunning = !serviceRunning
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
    serviceRunning: Boolean,
    onRequestPermissions: () -> Unit,
    onToggleService: () -> Unit,
    onOpenSpike: () -> Unit,
    onOpenMessagerie: () -> Unit,
    onOpenAppairage: () -> Unit,
    onOpenTransport: () -> Unit,
    onOpenReseau: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(text = stringResource(R.string.app_name))

        // Messagerie : accessible sans permissions BLE, puisqu'aucune radio
        // n'alimente encore le nœud (AndroidTransport, US-213 / US-306).
        Button(onClick = onOpenMessagerie) {
            Text(text = "Conversations")
        }

        // L'appairage par QR ne dépend pas du Bluetooth : accessible même
        // sans les permissions BLE.
        Button(onClick = onOpenAppairage) {
            Text(text = stringResource(R.string.appairage_ouvrir))
        }

        if (!permissionsGranted) {
            Text(text = stringResource(R.string.permissions_rationale_body))
            Button(onClick = onRequestPermissions) {
                Text(text = stringResource(R.string.permissions_grant_button))
            }
        } else {
            Text(
                text = stringResource(
                    if (serviceRunning) R.string.mesh_status_running else R.string.mesh_status_stopped,
                ),
            )
            Button(onClick = onToggleService) {
                Text(text = if (serviceRunning) "Arrêter" else "Démarrer")
            }
            Button(onClick = onOpenReseau) {
                Text(text = "Réseau")
            }
            // Essais du transport réel sur deux téléphones (US-213).
            Button(onClick = onOpenTransport) {
                Text(text = "Transport BLE (debug)")
            }
            // Écran de debug jetable (US-103) — voir ble/spike/HelloMeshSpikeScreen.kt.
            Button(onClick = onOpenSpike) {
                Text(text = "Spike C : hello mesh (debug)")
            }
        }
    }
}
