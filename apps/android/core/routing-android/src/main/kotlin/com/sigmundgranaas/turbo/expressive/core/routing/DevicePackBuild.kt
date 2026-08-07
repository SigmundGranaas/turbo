package com.sigmundgranaas.turbo.expressive.core.routing

import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import uniffi.turbo_route_ffi.BuildPhase
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
        return when (
            val r = builder.build(
                spec.bounds,
                onProgress = { phase, f -> onProgress(overall(phase, f)) },
            )
        ) {
            is DevicePackBuilder.Outcome.Done ->
                WgpuOfflineTileManager.DeviceBuildResult.Done(r.bytes)

            // "No offline routing for this region", not "the download
            // broke". Failing the region would take away the map tiles
            // the user actually asked for, and the dialog already said
            // routing was not included for an area this size.
            is DevicePackBuilder.Outcome.TooLarge ->
                WgpuOfflineTileManager.DeviceBuildResult.Disabled

            // Not Disabled. Disabled means "carry on without routing",
            // which for a build the user stopped would finish the region
            // and mark it Complete — the opposite of what they asked for.
            DevicePackBuilder.Outcome.Cancelled ->
                WgpuOfflineTileManager.DeviceBuildResult.Cancelled

            is DevicePackBuilder.Outcome.Failed ->
                WgpuOfflineTileManager.DeviceBuildResult.Failed(r.reason)
        }
    }

    private companion object {
        /**
         * Where each phase ends, as a fraction of the whole build.
         *
         * The builder reports progress *within* a phase, so each one
         * runs 0..1 on its own. Passing that straight through would
         * sweep the bar from empty to full five times and read as four
         * restarts. These weights are the shares actually measured on a
         * 9x14 km region — terrain dominates because the DEM is most of
         * a pack, both in requests and in bytes.
         */
        val ENDS = mapOf(
            BuildPhase.TERRAIN to (0f to 0.55f),
            BuildPhase.VECTORS to (0.55f to 0.75f),
            BuildPhase.WATER to (0.75f to 0.85f),
            BuildPhase.TRAILS to (0.85f to 0.97f),
            BuildPhase.FINISHING to (0.97f to 1f),
        )

        /** Map a phase-local fraction onto the whole build. */
        fun overall(phase: BuildPhase, f: Float): Float {
            val (from, to) = ENDS[phase] ?: return f
            return (from + (to - from) * f.coerceIn(0f, 1f)).coerceIn(0f, 1f)
        }
    }
}
