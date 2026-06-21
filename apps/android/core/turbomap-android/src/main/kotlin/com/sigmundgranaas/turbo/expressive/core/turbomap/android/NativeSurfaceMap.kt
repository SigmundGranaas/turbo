package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import android.view.Surface

/**
 * The on-screen render path: a `wgpu::Surface` built from an Android [Surface]
 * via hand-written JNI (the uniffi control plane in `uniffi.turbomap_ffi` can't
 * carry an `ANativeWindow`). Backed by `turbomap-ffi/src/surface.rs`.
 *
 * [nativeCreate] returns an opaque handle (0 = failure) the caller threads
 * through the other calls and finally frees with [nativeDestroy]. A
 * `SurfaceView`/`Choreographer` host (or an `ImageReader` in tests) drives
 * [nativeRender] each frame. All methods are safe to call off the main thread.
 */
internal object NativeSurfaceMap {
    init {
        // Same .so as the uniffi bindings (JNA loads it too); dlopen refcounts.
        System.loadLibrary("turbomap_ffi")
    }

    external fun nativeCreate(
        surface: Surface,
        width: Int,
        height: Int,
        lat: Double,
        lng: Double,
        zoom: Double,
    ): Long

    /** The last GPU/surface init or caught-panic reason (and clears it), or null. */
    external fun nativeLastError(): String?

    external fun nativeApplyScene(handle: Long, sceneJson: String): Boolean

    external fun nativePumpLocal(handle: Long)

    external fun nativeRender(handle: Long)

    /** True while a camera animation or tile fade-in is running (keep drawing). */
    external fun nativeIsAnimating(handle: Long): Boolean

    /** Compact JSON of last-frame cache telemetry: tiles/bytes/budget/evictions/hits/misses. */
    external fun nativeStats(handle: Long): String

    external fun nativeResize(handle: Long, width: Int, height: Int)

    /** Reserve [bottomPx] at the bottom of the viewport (projection + render shift up). */
    external fun nativeSetViewportInset(handle: Long, bottomPx: Double)

    external fun nativeDestroy(handle: Long)

    // ── Control plane (the MapEngine contract) ──────────────────────────────
    external fun nativeSetCamera(handle: Long, lat: Double, lng: Double, zoom: Double, bearingDeg: Double)

    /** One-finger pan step: translate the camera by a screen-space finger delta
     *  (px). Applied render-side against the live camera so successive deltas
     *  accumulate without a stale-snapshot recompute → smooth pan in 2D + 3D. */
    external fun nativePanBy(handle: Long, dx: Double, dy: Double)

    // ── Physics / motion ────────────────────────────────────────────────────
    /** Start an inertial pan fling at screen-pixel velocity (drag-release). */
    external fun nativeFling(handle: Long, vx: Double, vy: Double)

    /** Start a momentum zoom at [zoomVelocity] (zoom-levels/s, +=in) about ([fx],[fy]). */
    external fun nativeZoomFling(handle: Long, zoomVelocity: Double, fx: Double, fy: Double)

    /** Ease the camera to a pose over [durationMs] (accel/decel). */
    external fun nativeEaseTo(handle: Long, lat: Double, lng: Double, zoom: Double, bearingDeg: Double, durationMs: Int)

    /** Animate a focus-invariant zoom by [factor] about ([fx],[fy]) over [durationMs]. */
    external fun nativeZoomAroundAnimated(handle: Long, factor: Double, fx: Double, fy: Double, durationMs: Int)

    /** Immediate focus-invariant zoom by [factor] about screen px ([fx],[fy]) — live pinch. */
    external fun nativeZoomAround(handle: Long, factor: Double, fx: Double, fy: Double)

    /**
     * One 3D-mode orbit step: rotate the bearing by [dBearingDeg] and tilt by
     * [dPitchDeg], both about the pinned focus pixel ([fx],[fy]) so that pixel
     * stays over the same world point. Pitch is clamped to the engine limit.
     */
    external fun nativeOrbitAround(handle: Long, dBearingDeg: Double, dPitchDeg: Double, fx: Double, fy: Double)

    /** Ease only the tilt to [pitchDeg] over [durationMs] (2D↔3D transition). */
    external fun nativeEasePitch(handle: Long, pitchDeg: Double, durationMs: Int)

    /** Catch any in-flight camera animation, freezing the camera where it is. */
    external fun nativeCancelAnimation(handle: Long)

    /** `[lat, lng, zoom, bearingDeg]` (empty if the handle is gone). */
    external fun nativeCamera(handle: Long): DoubleArray

    /** `[xPx, yPx, valid]` — `valid` is 1.0 when the point projects on-screen. */
    external fun nativeProject(handle: Long, lat: Double, lng: Double): DoubleArray

    /** `[lat, lng, valid]`. */
    external fun nativeUnproject(handle: Long, xPx: Double, yPx: Double): DoubleArray

    // ── Host-driven tile IO ─────────────────────────────────────────────────
    /** Tiles the engine awaits, as JSON `[{"kind","layer","z","x","y"}, ...]`. */
    external fun nativePendingTilesJson(handle: Long): String

    /** Push a fetched raster tile (encoded image bytes); false if it didn't decode. */
    external fun nativeIngestRaster(handle: Long, layerId: String, z: Int, x: Int, y: Int, bytes: ByteArray): Boolean

    /** Push a fetched DEM tile (Mapbox-Terrain-RGB PNG) into the shared heightmap (3D terrain). */
    external fun nativeIngestTerrain(handle: Long, z: Int, x: Int, y: Int, bytes: ByteArray): Boolean

    /**
     * Track the sun (terrain shading + sky colour) to a real UTC instant,
     * so the scene's light matches the time of day. [unixSeconds] is UTC
     * seconds since the epoch; a negative value reverts to a fixed default.
     */
    external fun nativeSetSunTime(handle: Long, unixSeconds: Double)

    /**
     * Enable terrain *cast* shadows (a peak shadows the valley behind it) at
     * [strength] in `[0,1]`; 0 disables the feature (zero per-frame cost).
     * Only affects 3D terrain; distinct from the always-on relief self-shading.
     */
    external fun nativeSetTerrainShadows(handle: Long, strength: Float)

    // ── Weather-cloud overlay ───────────────────────────────────────────────
    /** Enable the procedural cloud overlay with a [gridW]×[gridH] radar grid. */
    external fun nativeEnableClouds(handle: Long, gridW: Int, gridH: Int)

    /** Hide/show the overlay without discarding uploaded frames. */
    external fun nativeSetCloudsVisible(handle: Long, visible: Boolean)

    /** Geo-register the radar to its lat/lng box → world-locked overlay. */
    external fun nativeSetCloudGeoBounds(
        handle: Long,
        west: Double,
        south: Double,
        east: Double,
        north: Double,
    )

    /**
     * Upload a radar frame into [slot] (0 = current timestep, 1 = next) from two
     * [gridW]×[gridH] byte planes — [precip] and [coverage], each 0..255.
     */
    external fun nativeIngestRadarFrame(
        handle: Long,
        slot: Int,
        gridW: Int,
        gridH: Int,
        precip: ByteArray,
        coverage: ByteArray,
    )

    /**
     * Set the cloud animation clock ([time], seconds) and the slot-0→slot-1
     * crossfade ([blend], 0..1) — what a time slider scrubs (forward or back).
     */
    external fun nativeSetCloudTime(handle: Long, time: Float, blend: Float)
}
