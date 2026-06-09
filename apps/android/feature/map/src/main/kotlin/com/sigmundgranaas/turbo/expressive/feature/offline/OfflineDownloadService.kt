package com.sigmundgranaas.turbo.expressive.feature.offline

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
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.core.map.DownloadPolicy
import com.sigmundgranaas.turbo.expressive.core.map.NetworkMonitor
import com.sigmundgranaas.turbo.expressive.core.map.OfflineTileManager
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import com.sigmundgranaas.turbo.expressive.feature.map.R
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Foreground service that keeps offline map downloads running while the app is
 * backgrounded, surfaces aggregate progress as an ongoing notification, and gates
 * downloads on the connectivity policy (auto-pausing on metered/no network when
 * "Wi-Fi only" is on, resuming when un-metered returns). It owns only the
 * notification, the process lifetime and the policy loop; the downloads themselves
 * live in the singleton [OfflineTileManager]. Started via [start] from the manager
 * when a download begins, it stops itself once nothing is downloading or paused.
 */
@AndroidEntryPoint
class OfflineDownloadService : Service() {

    @Inject lateinit var manager: OfflineTileManager
    @Inject lateinit var network: NetworkMonitor
    @Inject lateinit var settings: SettingsRepository

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var policyJob: Job? = null
    private var notifyJob: Job? = null
    private var graceJob: Job? = null
    private var started = false
    /** Have we observed actual work yet? Guards against tearing down during the gap
     *  between startForegroundService and the new region appearing in the flow. */
    private var sawWork = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_PAUSE -> manager.regions.value.filter { it.status == OfflineStatus.Downloading }.forEach { manager.pause(it.id) }
            ACTION_RESUME -> manager.regions.value.filter { it.status == OfflineStatus.Paused }.forEach { manager.resume(it.id) }
            else -> Unit
        }
        ensureStarted()
        return START_STICKY
    }

    private fun ensureStarted() {
        if (started) return
        started = true
        createChannel()
        startForegroundCompat(buildNotification(manager.regions.value))

        // Apply the connectivity policy continuously so downloads pause on metered/no
        // network (when Wi-Fi-only) and resume when un-metered comes back.
        policyJob = scope.launch {
            combine(network.state, settings.settings) { net, prefs ->
                DownloadPolicy.shouldDownload(prefs.downloadOverWifiOnly, net)
            }.distinctUntilChanged().collect { manager.setNetworkAllowed(it) }
        }

        // Re-post the notification on progress; tear down once work that we've actually
        // seen is finished. Don't tear down before any work appears (warm-up gap).
        notifyJob = scope.launch {
            manager.regions.collect { regions ->
                val pending = regions.any { it.status == OfflineStatus.Downloading || it.status == OfflineStatus.Paused }
                when {
                    pending -> { sawWork = true; notificationManager().notify(NOTIF_ID, buildNotification(regions)) }
                    sawWork -> teardown()
                    else -> Unit // still warming up — keep foreground, wait for the region
                }
            }
        }

        // Safety net: if a download never materialises (e.g. it failed instantly), don't
        // hold the foreground service open forever.
        graceJob = scope.launch {
            kotlinx.coroutines.delay(STARTUP_GRACE_MS)
            if (!sawWork) teardown()
        }
    }

    private fun teardown() {
        policyJob?.cancel()
        notifyJob?.cancel()
        graceJob?.cancel()
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        policyJob?.cancel()
        notifyJob?.cancel()
        graceJob?.cancel()
        super.onDestroy()
    }

    private fun startForegroundCompat(notification: Notification) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIF_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        } else {
            startForeground(NOTIF_ID, notification)
        }
    }

    private fun buildNotification(regions: List<OfflineRegionInfo>): Notification {
        val pending = regions.filter { it.status == OfflineStatus.Downloading || it.status == OfflineStatus.Paused }
        val pct = if (pending.isEmpty()) 0 else (pending.map { it.progress }.average() * 100).toInt().coerceIn(0, 100)
        val anyActive = pending.any { it.status == OfflineStatus.Downloading }
        val builder = Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.stat_sys_download)
            .setContentTitle(getString(R.string.offline_notif_title))
            .setContentText(getString(R.string.offline_notif_content, pending.size, pct))
            .setContentIntent(openAppIntent())
            .setVisibility(Notification.VISIBILITY_PUBLIC)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setProgress(100, pct, !anyActive)
        if (anyActive) {
            builder.addAction(action(android.R.drawable.ic_media_pause, getString(R.string.offline_notif_pause), ACTION_PAUSE, reqCode = 1))
        } else {
            builder.addAction(action(android.R.drawable.ic_media_play, getString(R.string.offline_notif_resume), ACTION_RESUME, reqCode = 2))
        }
        return builder.build()
    }

    private fun openAppIntent(): PendingIntent = PendingIntent.getActivity(
        this, 0, packageManager.getLaunchIntentForPackage(packageName),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    private fun action(icon: Int, label: String, action: String, reqCode: Int): Notification.Action =
        Notification.Action.Builder(
            Icon.createWithResource(this, icon), label,
            PendingIntent.getService(
                this, reqCode, Intent(this, OfflineDownloadService::class.java).setAction(action),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            ),
        ).build()

    private fun createChannel() {
        val channel = NotificationChannel(CHANNEL_ID, getString(R.string.offline_notif_channel), NotificationManager.IMPORTANCE_LOW).apply {
            description = getString(R.string.offline_notif_channel_desc)
            setShowBadge(false)
        }
        notificationManager().createNotificationChannel(channel)
    }

    private fun notificationManager() = getSystemService(NotificationManager::class.java)

    companion object {
        private const val CHANNEL_ID = "offline_downloads"
        private const val NOTIF_ID = 43
        private const val STARTUP_GRACE_MS = 15_000L
        private const val ACTION_PAUSE = "com.sigmundgranaas.turbo.expressive.OFFLINE_PAUSE"
        private const val ACTION_RESUME = "com.sigmundgranaas.turbo.expressive.OFFLINE_RESUME"

        /** Start (or keep alive) the foreground download service. Idempotent. */
        fun start(context: Context) {
            context.startForegroundService(Intent(context, OfflineDownloadService::class.java))
        }
    }
}
