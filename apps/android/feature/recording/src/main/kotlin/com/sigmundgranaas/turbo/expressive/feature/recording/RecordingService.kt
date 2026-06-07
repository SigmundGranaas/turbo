package com.sigmundgranaas.turbo.expressive.feature.recording

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.drawable.Icon
import android.os.Build
import android.os.IBinder
import com.sigmundgranaas.turbo.expressive.core.data.RecordingController
import com.sigmundgranaas.turbo.expressive.core.data.RecordingSession
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.core.geo.Units
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Foreground service that keeps a GPS [RecordingController] session alive while
 * the app is backgrounded or the screen is locked. The service owns only the
 * foreground notification + process lifetime; the recording data lives in the
 * (singleton) controller, which both this and the ViewModel read.
 */
@AndroidEntryPoint
class RecordingService : Service() {

    @Inject lateinit var controller: RecordingController
    @Inject lateinit var settings: SettingsRepository

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var notifyJob: Job? = null
    private var settingsJob: Job? = null

    /** Live metric/imperial preference so the notification matches the in-app stats. */
    private var metric = true

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                controller.stop()
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
            }
            // Pause/resume straight from the notification (shade / lock screen) — like a
            // music app. The session collector below re-posts with the flipped state.
            ACTION_PAUSE -> controller.togglePause()
            else -> {
                createChannel()
                startInForeground(controller.session.value)
                controller.start()
                settingsJob?.cancel()
                settingsJob = scope.launch { settings.settings.collect { metric = it.metricUnits } }
                notifyJob?.cancel()
                notifyJob = scope.launch {
                    controller.session.collectLatest { session ->
                        if (session.active) {
                            manager().notify(NOTIF_ID, buildNotification(session))
                        }
                    }
                }
            }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        notifyJob?.cancel()
        settingsJob?.cancel()
        super.onDestroy()
    }

    private fun startInForeground(session: RecordingSession) {
        val notification = buildNotification(session)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIF_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_LOCATION)
        } else {
            startForeground(NOTIF_ID, notification)
        }
    }

    private fun buildNotification(session: RecordingSession): Notification {
        val openApp = packageManager.getLaunchIntentForPackage(packageName)
        val content = PendingIntent.getActivity(
            this, 0, openApp, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val distance = Units.distance(session.distanceM, metric)
        // Music-app-style transport controls, usable without opening the app.
        val pauseResume = Notification.Action.Builder(
            Icon.createWithResource(this, if (session.paused) android.R.drawable.ic_media_play else android.R.drawable.ic_media_pause),
            getString(if (session.paused) R.string.rec_notif_resume else R.string.rec_notif_pause),
            servicePendingIntent(ACTION_PAUSE, reqCode = 1),
        ).build()
        val stop = Notification.Action.Builder(
            Icon.createWithResource(this, android.R.drawable.ic_menu_close_clear_cancel),
            getString(R.string.rec_notif_stop),
            servicePendingIntent(ACTION_STOP, reqCode = 2),
        ).build()
        return Notification.Builder(this, CHANNEL_ID)
            .setContentTitle(getString(if (session.paused) R.string.rec_notif_paused else R.string.rec_notif_recording))
            .setContentText(getString(R.string.rec_notif_content, distance, formatElapsed(session.elapsedSec)))
            .setSmallIcon(android.R.drawable.ic_menu_mylocation)
            .setContentIntent(content)
            .setCategory(Notification.CATEGORY_WORKOUT)
            .setVisibility(Notification.VISIBILITY_PUBLIC)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .addAction(pauseResume)
            .addAction(stop)
            .build()
    }

    /** A PendingIntent that re-enters this service with [action] (notification buttons). */
    private fun servicePendingIntent(action: String, reqCode: Int): PendingIntent = PendingIntent.getService(
        this, reqCode, Intent(this, RecordingService::class.java).setAction(action),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    private fun createChannel() {
        val channel = NotificationChannel(CHANNEL_ID, getString(R.string.rec_notif_channel), NotificationManager.IMPORTANCE_LOW).apply {
            description = getString(R.string.rec_notif_channel_desc)
            setShowBadge(false)
        }
        manager().createNotificationChannel(channel)
    }

    private fun manager() = getSystemService(NotificationManager::class.java)

    private fun formatElapsed(seconds: Int): String {
        val h = seconds / 3600
        val m = (seconds % 3600) / 60
        val s = seconds % 60
        return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%02d:%02d".format(m, s)
    }

    companion object {
        private const val CHANNEL_ID = "track_recording"
        private const val NOTIF_ID = 42
        private const val ACTION_STOP = "com.sigmundgranaas.turbo.expressive.RECORDING_STOP"
        private const val ACTION_PAUSE = "com.sigmundgranaas.turbo.expressive.RECORDING_PAUSE"

        /** Start recording and bring the service to the foreground. */
        fun start(context: Context) {
            context.startForegroundService(Intent(context, RecordingService::class.java))
        }

        /** Stop recording and dismiss the foreground notification. */
        fun stop(context: Context) {
            context.startService(Intent(context, RecordingService::class.java).setAction(ACTION_STOP))
        }
    }
}
