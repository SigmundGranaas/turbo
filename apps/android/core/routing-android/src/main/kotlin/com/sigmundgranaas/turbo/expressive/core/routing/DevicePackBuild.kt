package com.sigmundgranaas.turbo.expressive.core.routing

import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.core.map.WgpuOfflineTileManager
import kotlinx.coroutines.flow.first

/**
 * Connects the offline downloader to the on-device pack builder.
 *
 * The downloader knows a region has no server-side pack and nothing
 * about how to make one; [DevicePackBuilder] knows how to make one and
 * nothing about downloads. This is the joint, and it is also where the
 * user's choice is read.
 *
 * The setting is read per build, not held: it is a toggle in Settings,
 * and someone who turns it on and immediately starts a download expects
 * that download to honour it.
 */
class DevicePackBuild(
    private val builder: DevicePackBuilder,
    private val settings: SettingsRepository,
) : WgpuOfflineTileManager.DevicePackBuild {

    override suspend fun build(
        spec: DownloadSpec,
        onProgress: (Float) -> Unit,
    ): WgpuOfflineTileManager.DeviceBuildResult {
        if (!settings.settings.first().buildPacksOnDevice) {
            return WgpuOfflineTileManager.DeviceBuildResult.Disabled
        }
        return when (val r = builder.build(spec.bounds, onProgress = { _, f -> onProgress(f) })) {
            is DevicePackBuilder.Outcome.Done ->
                WgpuOfflineTileManager.DeviceBuildResult.Done(r.bytes)

            // Both of these are "no offline routing for this region",
            // not "the download broke". Failing the region would take
            // away the map tiles the user actually asked for, and in the
            // TooLarge case the dialog already said routing was not
            // included.
            is DevicePackBuilder.Outcome.TooLarge ->
                WgpuOfflineTileManager.DeviceBuildResult.Disabled

            DevicePackBuilder.Outcome.Cancelled ->
                WgpuOfflineTileManager.DeviceBuildResult.Disabled

            is DevicePackBuilder.Outcome.Failed ->
                WgpuOfflineTileManager.DeviceBuildResult.Failed(r.reason)
        }
    }
}
