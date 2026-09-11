package com.dengon.app.ble

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import com.dengon.app.MainActivity
import com.dengon.app.R

/**
 * Service de fond du maillage BLE (US-109 : squelette uniquement).
 *
 * Déclaré `foregroundServiceType="connectedDevice"` dans le manifest : c'est
 * le type imposé par Android 14+ pour un service qui maintient une connexion
 * radio active en arrière-plan (docs/synthese/10-benchmarks-mvp-tests.md,
 * §2.7 « Contraintes d'arrière-plan Android »). Sans notification permanente
 * et sans ce type déclaré, le système tue le service peu après l'extinction
 * de l'écran.
 *
 * La logique BLE réelle (GATT server + scanner + advertiser) arrive avec
 * `AndroidTransport` (US-213) ; pour l'instant ce service ne fait que
 * démontrer qu'il survit écran éteint.
 */
class MeshForegroundService : Service() {

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        ServiceCompat.startForeground(
            this,
            NOTIFICATION_ID,
            buildNotification(),
            ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE,
        )
        // START_STICKY : le système relance le service (intent=null) s'il a
        // dû le tuer pour libérer de la mémoire — comportement voulu pour un
        // relais qui doit rester joignable.
        return START_STICKY
    }

    private fun buildNotification(): Notification {
        val openAppIntent = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE,
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(getString(R.string.mesh_notification_title))
            .setContentText(getString(R.string.mesh_notification_text))
            .setSmallIcon(android.R.drawable.stat_sys_data_bluetooth)
            .setContentIntent(openAppIntent)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return

        val channel = NotificationChannel(
            CHANNEL_ID,
            getString(R.string.mesh_notification_channel_name),
            NotificationManager.IMPORTANCE_LOW,
        )
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(channel)
    }

    companion object {
        const val CHANNEL_ID = "dengon_mesh_service"
        const val NOTIFICATION_ID = 1
    }
}
