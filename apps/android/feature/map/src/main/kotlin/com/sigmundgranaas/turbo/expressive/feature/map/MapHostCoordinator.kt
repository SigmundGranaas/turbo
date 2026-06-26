package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.feature.map.live.LiveDetent

/**
 * The map host's *decision logic*, lifted out of `MapScreen`'s Compose effects
 * so it can be reasoned about and unit-tested without a renderer, a device, or
 * the Compose runtime.
 *
 * Deliberately a plain object of pure functions: the Compose-bound machinery
 * (the `LaunchedEffect`s, permission launchers, camera controller, snackbars)
 * stays in `MapScreen` and calls into here. Each function answers one question
 * the screen used to answer inline; the screen then performs the side effect.
 */
object MapHostCoordinator {

    /** Re-solve the route if the user strays at least this far (m) from its line. */
    const val OFF_ROUTE_THRESHOLD_M = 50.0

    /** Zoom used for the one-shot startup centre (restore or first-fix fly-to). */
    const val INITIAL_LOCATION_ZOOM = 13.0

    /** True when [here] has drifted off [geometry] far enough to warrant a reroute. */
    fun isOffRoute(
        geometry: List<LatLng>,
        here: LatLng,
        thresholdM: Double = OFF_ROUTE_THRESHOLD_M,
    ): Boolean = geometry.size >= 2 && GeoMetrics.distanceToPath(geometry, here) > thresholdM

    /**
     * Bottom map inset (px) that keeps the user dot in the band above the live
     * sheet at [detent], capped at half the screen so a tall sheet can't shove
     * the target off the top. Pure given the screen height and density — the
     * caller decides *whether* a sheet is showing.
     */
    fun bottomInsetPx(detent: LiveDetent, screenHeightPx: Float, density: Density): Int {
        val sheetPx = when (detent) {
            LiveDetent.Mini -> with(density) { 208.dp.toPx() }.coerceAtMost(screenHeightPx * 0.64f)
            LiveDetent.Peek -> with(density) { 340.dp.toPx() }.coerceAtMost(screenHeightPx * 0.64f)
            LiveDetent.Half -> screenHeightPx * 0.56f
            LiveDetent.Full -> screenHeightPx * 0.92f
        }
        return sheetPx.coerceAtMost(screenHeightPx * 0.5f).toInt()
    }

    /** What the one-shot startup camera should do — see [cameraRestore]. */
    sealed interface CameraRestore {
        /** Restore the persisted camera from the last session. */
        data class RestoreSaved(val target: LatLng, val zoom: Double) : CameraRestore
        /** First-ever launch: fly to the first GPS fix. */
        data class FlyToFix(val target: LatLng, val zoom: Double) : CameraRestore
        /** Do nothing (already centred, an external focus is pending, or no input yet). */
        data object None : CameraRestore
    }

    /**
     * Decide the one-shot startup centre. Folds the two `MapScreen` restore
     * effects into one rule: restore the saved camera if there is one, else (only
     * on a first-ever launch) fly to the first fix — but never while an external
     * focus request is pending, already centred, or actively following/recording
     * (which own the camera themselves).
     */
    fun cameraRestore(
        didInitialCenter: Boolean,
        hasFocusRequest: Boolean,
        lastCamera: LatLng?,
        lastCameraZoom: Double?,
        userLocation: LatLng?,
        following: Boolean,
        recording: Boolean,
    ): CameraRestore {
        if (didInitialCenter || hasFocusRequest) return CameraRestore.None
        if (lastCamera != null) return CameraRestore.RestoreSaved(lastCamera, lastCameraZoom ?: INITIAL_LOCATION_ZOOM)
        if (userLocation != null && !following && !recording) {
            return CameraRestore.FlyToFix(userLocation, INITIAL_LOCATION_ZOOM)
        }
        return CameraRestore.None
    }

    /**
     * Whether the continuously-debounced camera write should fire for [current].
     * Skips null-island (uninitialised), the exact world-overview [fallback] (so a
     * brand-new user's pre-navigation view isn't saved as "last"), and unchanged
     * frames (DataStore would no-op anyway).
     */
    fun shouldPersistCamera(
        current: Triple<Double, Double, Double>,
        last: Triple<Double, Double, Double>?,
        fallback: LatLng,
    ): Boolean {
        if (current == last) return false
        val (lat, lng, _) = current
        val atFallback = kotlin.math.abs(lat - fallback.lat) < 1e-6 &&
            kotlin.math.abs(lng - fallback.lng) < 1e-6
        if (atFallback) return false
        return lat != 0.0 || lng != 0.0
    }
}
