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
import com.sigmundgranaas.turbo.expressive.core.data.FollowController
import com.sigmundgranaas.turbo.expressive.core.data.LiveStats
import com.sigmundgranaas.turbo.expressive.core.data.RecordingController
import com.sigmundgranaas.turbo.expressive.core.geo.Units
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import javax.inject.Inject

/**
 * Foreground service that keeps a GPS journey alive while the app is backgrounded
 * or the screen is locked, and surfaces it as an Android Live Update. It runs in
 * one of two modes — **recording** a track or **following** a route — and posts a
 * glanceable notification for each, built from the same [LiveStats] read-model the
 * in-app sheet renders, so the lock screen and the sheet can't disagree. The
 * service owns only the notification + process lifetime; the data lives in the
 * (singleton) controllers.
 */
@AndroidEntryPoint
class RecordingService : Service() {

    @Inject lateinit var controller: RecordingController
    @Inject lateinit var follow: FollowController
    @Inject lateinit var settings: SettingsRepository

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var notifyJob: Job? = null
    private var settingsJob: Job? = null

    /** Recording and following share one service + notification, so a stop for one
     *  mode must not tear down the other. */
    private enum class Mode { None, Recording, Following }
    private var mode = Mode.None

    /** Live metric/imperial preference so the notification matches the in-app stats. */
    private var metric = true

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                controller.stop()
                if (mode == Mode.Recording) teardown()
            }
            ACTION_STOP_FOLLOW -> {
                follow.stop()
                if (mode == Mode.Following) teardown()
            }
            // Pause/resume straight from the notification (shade / lock screen) — like a
            // music app. The session collector below re-posts with the flipped state. If you
            // walked while paused (the nudge case), resuming from here KEEPS that walk (US-4) —
            // discarding needs the explicit in-app prompt, never a single lock-screen tap.
            ACTION_PAUSE -> {
                val s = controller.session.value
                if (s.paused && s.hasBufferedMovement) controller.resume(includeBuffered = true) else controller.togglePause()
            }
            ACTION_FOLLOW -> startFollowing()
            else -> startRecording()
        }
        return START_STICKY
    }

    override fun onDestroy() {
        notifyJob?.cancel()
        settingsJob?.cancel()
        super.onDestroy()
    }

    private fun startRecording() {
        mode = Mode.Recording
        createChannel()
        startForegroundCompat(buildRecordingNotification(LiveStats.of(controller.session.value), controller.session.value.paused))
        controller.start()
        trackSettings()
        notifyJob?.cancel()
        notifyJob = scope.launch {
            controller.session.collectLatest { session ->
                if (session.active) manager().notify(NOTIF_ID, buildRecordingNotification(LiveStats.of(session), session.paused))
            }
        }
    }

    private fun startFollowing() {
        // Never hijack an in-progress recording; recording owns the surface.
        if (mode == Mode.Recording) return
        mode = Mode.Following
        createChannel()
        startForegroundCompat(buildFollowingNotification(LiveStats.of(follow.session.value), follow.session.value.name))
        trackSettings()
        notifyJob?.cancel()
        notifyJob = scope.launch {
            follow.session.collectLatest { session ->
                if (session.active) {
                    manager().notify(NOTIF_ID, buildFollowingNotification(LiveStats.of(session), session.name))
                } else if (mode == Mode.Following) {
                    teardown()
                }
            }
        }
    }

    private fun teardown() {
        mode = Mode.None
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun trackSettings() {
        settingsJob?.cancel()
        settingsJob = scope.launch { settings.settings.collect { metric = it.metricUnits } }
    }

    private fun startForegroundCompat(notification: Notification) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIF_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_LOCATION)
        } else {
            startForeground(NOTIF_ID, notification)
        }
    }

    private fun buildRecordingNotification(stats: LiveStats, paused: Boolean): Notification {
        val distance = Units.distance(stats.distanceM, metric)
        val elapsed = formatElapsed(stats.elapsedSec ?: 0)
        val rich = getString(
            R.string.rec_notif_big, distance,
            Units.elevation(stats.ascentM ?: 0.0, metric),
            "${Units.speedValue(stats.speedMps ?: 0.0, metric)} ${Units.speedUnit(metric)}",
        )
        val pauseResume = action(
            if (paused) android.R.drawable.ic_media_play else android.R.drawable.ic_media_pause,
            getString(if (paused) R.string.rec_notif_resume else R.string.rec_notif_pause),
            ACTION_PAUSE, reqCode = 1,
        )
        val stop = action(android.R.drawable.ic_menu_close_clear_cancel, getString(R.string.rec_notif_stop), ACTION_STOP, reqCode = 2)
        // Proactive nudge (US-4): if you've walked while paused, the notification itself
        // alerts you on the lock screen instead of silently sitting paused.
        val title = when {
            stats.showResumeNudge -> getString(R.string.rec_notif_nudge_title)
            paused -> getString(R.string.rec_notif_paused)
            else -> getString(R.string.rec_notif_recording)
        }
        val content = if (stats.showResumeNudge) {
            getString(R.string.rec_notif_nudge_content, Units.distance(stats.bufferedDistanceM, metric))
        } else {
            getString(R.string.rec_notif_content, distance, elapsed)
        }
        val builder = baseBuilder()
            .setContentTitle(title)
            .setContentText(content)
            .setStyle(Notification.BigTextStyle().bigText(rich))
            .addAction(pauseResume)
            .addAction(stop)
        promote(builder, distance)
        return builder.build()
    }

    private fun buildFollowingNotification(stats: LiveStats, name: String?): Notification {
        val left = Units.distance(stats.distanceRemainingM ?: 0.0, metric)
        val eta = formatElapsed(stats.etaSeconds ?: 0)
        val title = if (name != null) getString(R.string.rec_notif_following_named, name) else getString(R.string.rec_notif_following)
        val stop = action(android.R.drawable.ic_menu_close_clear_cancel, getString(R.string.rec_notif_stop_following), ACTION_STOP_FOLLOW, reqCode = 3)
        val builder = baseBuilder()
            .setContentTitle(title)
            .setContentText(getString(R.string.rec_notif_follow_content, left, eta))
            .addAction(stop)
        // A determinate progress bar mirrors the in-app route-progress hero.
        val pct = ((stats.fraction ?: 0.0) * 100).toInt().coerceIn(0, 100)
        builder.setProgress(100, pct, false)
        promote(builder, left)
        return builder.build()
    }

    private fun baseBuilder() = Notification.Builder(this, CHANNEL_ID)
        .setSmallIcon(android.R.drawable.ic_menu_mylocation)
        .setContentIntent(openAppIntent())
        .setCategory(Notification.CATEGORY_WORKOUT)
        .setVisibility(Notification.VISIBILITY_PUBLIC)
        .setOngoing(true)
        .setOnlyAlertOnce(true)

    private fun openAppIntent(): PendingIntent = PendingIntent.getActivity(
        this, 0, packageManager.getLaunchIntentForPackage(packageName),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    /**
     * "Live Updates": promote the ongoing notification to a glanceable status-bar
     * chip showing [chipText]. These APIs shifted across platform previews (the
     * compileSdk stub has them, an older runtime may not), so call them reflectively
     * — a no-op where unavailable, never a NoSuchMethodError.
     */
    private fun promote(builder: Notification.Builder, chipText: String) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.BAKLAVA) {
            runCatching {
                Notification.Builder::class.java.getMethod("setShortCriticalText", String::class.java).invoke(builder, chipText)
            }
            runCatching {
                Notification.Builder::class.java
                    .getMethod("setRequestPromotedOngoing", Boolean::class.javaPrimitiveType).invoke(builder, true)
            }
        }
    }

    private fun action(icon: Int, label: String, action: String, reqCode: Int): Notification.Action =
        Notification.Action.Builder(Icon.createWithResource(this, icon), label, servicePendingIntent(action, reqCode)).build()

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
        private const val ACTION_FOLLOW = "com.sigmundgranaas.turbo.expressive.FOLLOW_START"
        private const val ACTION_STOP_FOLLOW = "com.sigmundgranaas.turbo.expressive.FOLLOW_STOP"

        /**
         * Launch-intent extra the Quick Settings tile sets to ask the app to begin
         * recording. A location foreground service can't be started from a background
         * tile tap (foreground-only location permission), so the tile opens the app and
         * the map auto-starts recording once it's in the foreground.
         */
        const val EXTRA_START_RECORDING = "com.sigmundgranaas.turbo.expressive.START_RECORDING"

        /** Start recording and bring the service to the foreground. */
        fun start(context: Context) {
            context.startForegroundService(Intent(context, RecordingService::class.java))
        }

        /** Stop recording and dismiss the foreground notification. */
        fun stop(context: Context) {
            context.startService(Intent(context, RecordingService::class.java).setAction(ACTION_STOP))
        }

        /** Bring the route-following Live Update to the foreground (fed by FollowController). */
        fun startFollowing(context: Context) {
            context.startForegroundService(Intent(context, RecordingService::class.java).setAction(ACTION_FOLLOW))
        }

        /** Dismiss the following Live Update. */
        fun stopFollowing(context: Context) {
            context.startService(Intent(context, RecordingService::class.java).setAction(ACTION_STOP_FOLLOW))
        }
    }
}
