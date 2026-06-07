package com.sigmundgranaas.turbo.expressive.feature.recording

import android.app.PendingIntent
import android.content.Intent
import android.graphics.drawable.Icon
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import com.sigmundgranaas.turbo.expressive.core.data.LocationRepository
import com.sigmundgranaas.turbo.expressive.core.data.RecordingController
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

/**
 * Quick Settings tile: one tap to start/stop track recording without opening the app.
 * Reflects whether a session is active; tapping toggles it (starting the foreground
 * [RecordingService] when the location permission is held, else opening the app to grant it).
 */
@AndroidEntryPoint
class RecordingTileService : TileService() {

    @Inject lateinit var controller: RecordingController
    @Inject lateinit var location: LocationRepository

    override fun onStartListening() {
        super.onStartListening()
        render(controller.session.value.active)
    }

    override fun onClick() {
        super.onClick()
        if (controller.session.value.active) {
            // Stopping needs no location access, so it's a true one-tap from the tile.
            RecordingService.stop(this)
            render(false)
        } else {
            // A location foreground service can't be started from a background tile tap
            // (foreground-only location permission), so open the app and let the map
            // auto-start recording once it's foregrounded.
            openApp(startRecording = true)
        }
    }

    private fun render(active: Boolean) {
        val tile = qsTile ?: return
        tile.state = if (active) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        tile.label = getString(if (active) R.string.qs_tracking_on else R.string.qs_tracking)
        tile.icon = Icon.createWithResource(this, R.drawable.ic_qs_track)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            tile.subtitle = getString(if (active) R.string.qs_tracking_sub_on else R.string.qs_tracking_sub_off)
        }
        tile.updateTile()
    }

    private fun openApp(startRecording: Boolean = false) {
        val launch = (packageManager.getLaunchIntentForPackage(packageName) ?: return).apply {
            addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            if (startRecording) putExtra(RecordingService.EXTRA_START_RECORDING, true)
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startActivityAndCollapse(
                PendingIntent.getActivity(
                    this, 0, launch, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                ),
            )
        } else {
            @Suppress("DEPRECATION", "StartActivityAndCollapseDeprecated")
            startActivityAndCollapse(launch)
        }
    }
}
