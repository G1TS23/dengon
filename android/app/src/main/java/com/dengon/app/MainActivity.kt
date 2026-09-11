package com.dengon.app

import android.content.Intent
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
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

class MainActivity : ComponentActivity() {

    private var permissionsGranted = mutableStateOf(false)

    private val requestPermissions =
        registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { results ->
            permissionsGranted.value = results.values.all { it }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        permissionsGranted.value = BlePermissions.allGranted(this)

        setContent {
            DengonApp(
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
    permissionsGranted: MutableState<Boolean>,
    onRequestPermissions: () -> Unit,
    onStartService: () -> Unit,
    onStopService: () -> Unit,
) {
    val granted by permissionsGranted
    var serviceRunning by remember { mutableStateOf(false) }
    var showSpike by remember { mutableStateOf(false) }

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
            if (showSpike) {
                HelloMeshSpikeScreen(onBack = { showSpike = false })
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
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(text = stringResource(R.string.app_name))

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
            // Écran de debug jetable (US-103) — voir ble/spike/HelloMeshSpikeScreen.kt.
            Button(onClick = onOpenSpike) {
                Text(text = "Spike C : hello mesh (debug)")
            }
        }
    }
}
