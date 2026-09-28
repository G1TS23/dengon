package com.dengon.app

import android.content.Intent
import android.os.Build
import android.os.Bundle
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
import com.dengon.app.ble.transport.TransportDebugScreen
import com.dengon.app.ffi.DengonNodeStub
import com.dengon.app.ffi.generateIdentity
import com.dengon.app.identite.IdentiteLocale
import com.dengon.app.ui.appairage.AppairageScreen
import com.dengon.app.ui.appairage.AppairageViewModel
import com.dengon.app.ui.conversations.ConversationsViewModel
import com.dengon.app.ui.conversations.MessagerieRoute

class MainActivity : ComponentActivity() {

    private var permissionsGranted = mutableStateOf(false)

    // Messagerie (US-214) alimentée par le bouchon FFI (US-106). Le vrai nœud
    // (`DengonNode` généré par UniFFI) le remplacera à l'US-306 : seul ce
    // point d'injection change. Le ViewModel survit aux rotations d'écran.
    private val conversationsViewModel: ConversationsViewModel by viewModels {
        ConversationsViewModel.fabrique(DengonNodeStub(generateIdentity("moi")))
    }

    // US-215 : alimenté par le bouchon FFI (identité provisoire, voir IdentiteLocale).
    private val appairage: AppairageViewModel by viewModels {
        AppairageViewModel.fabrique(IdentiteLocale.identite(applicationContext))
    }

    private val requestPermissions =
        registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { results ->
            permissionsGranted.value = results.values.all { it }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        permissionsGranted.value = BlePermissions.allGranted(this)

        setContent {
            DengonApp(
                conversationsViewModel = conversationsViewModel,
                appairage = appairage,
                permissionsGranted = permissionsGranted,
                onRequestPermissions = { requestPermissions.launch(BlePermissions.required()) },
                onStartService = ::startMeshService,
                onStopService = ::stopMeshService,
            )
        }
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
    permissionsGranted: MutableState<Boolean>,
    onRequestPermissions: () -> Unit,
    onStartService: () -> Unit,
    onStopService: () -> Unit,
) {
    val granted by permissionsGranted
    var serviceRunning by remember { mutableStateOf(false) }
    var showSpike by remember { mutableStateOf(false) }
    var showMessagerie by remember { mutableStateOf(false) }
    var showAppairage by remember { mutableStateOf(false) }
    var showTransport by remember { mutableStateOf(false) }

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
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(text = stringResource(R.string.app_name))

        // Messagerie sur bouchon FFI (US-214) : accessible sans permissions
        // BLE, puisqu'aucune radio n'est utilisée tant que le vrai nœud n'est
        // pas branché (US-306).
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
