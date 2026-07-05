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

    /** `[lat, lng, valid]` — FLAT-plane unproject (pan / freehand capture). */
    external fun nativeUnproject(handle: Long, xPx: Double, yPx: Double): DoubleArray

    /**
     * `[lat, lng, worldZ, hitTerrain, valid]` — TERRAIN-AWARE screen→ground: where
     * the view ray meets the relief, not the flat plane. Use for marker
     * placement/drag so a dropped pin lands on the exact ground point under the
     * finger in 3D. `hitTerrain` is 0.0 when it fell back to the flat plane.
     */
    external fun nativeUnprojectGround(handle: Long, xPx: Double, yPx: Double): DoubleArray

    // ── Host-driven tile IO: the STREAMING PLAN ────────────────────────────
    /**
     * Grant [freeLanes] fetch lanes and drain every plan minted since the last
     * call, as a JSON array of plan objects:
     * `[{"start":[{"id","kind","layer","z","x","y"}],"cancel":[ids]}, …]`.
     * Consume-once — each `start` appears in exactly one take. Honour every
     * `cancel` (abort the fetch, then [nativeReportFetchCancelled]); a start
     * the host declines must also be reported cancelled so the engine
     * re-issues it. Deliveries complete through the ordinary `nativeIngest*`;
     * failures report via [nativeReportFetchFailed] (retry/backoff policy is
     * the HOST's — the engine re-pends immediately).
     */
    external fun nativeTakeStreamingPlanJson(handle: Long, freeLanes: Int): String

    /** Report a plan-issued fetch as failed; the tile re-pends while wanted. */
    external fun nativeReportFetchFailed(handle: Long, id: Long)

    /** Report a plan `cancel` as honoured (or a `start` the host declined). */
    external fun nativeReportFetchCancelled(handle: Long, id: Long)

    /** Push a fetched raster tile (encoded image bytes); false if it didn't decode. */
    external fun nativeIngestRaster(handle: Long, layerId: String, z: Int, x: Int, y: Int, bytes: ByteArray): Boolean

    /** Push a fetched vector tile (raw MVT bytes) into [layerId]; false if oversized/closed.
     *  The engine tessellates it — the water layer's polygons feed the water pipeline. */
    external fun nativeIngestVector(handle: Long, layerId: String, z: Int, x: Int, y: Int, bytes: ByteArray): Boolean

    /** Push a fetched DEM tile (Mapbox-Terrain-RGB PNG) into the shared heightmap (3D terrain). */
    external fun nativeIngestTerrain(handle: Long, z: Int, x: Int, y: Int, bytes: ByteArray): Boolean

    // ── Weather-cloud overlay: transport + clock only (plan P5.2) ───────────
    // What renders (grid, geo bounds, visibility, sun, shadows) is SCENE
    // state — declare it in the IR's `environment` and `nativeApplyScene` it.

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
