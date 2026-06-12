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

    external fun nativeResize(handle: Long, width: Int, height: Int)

    /** Reserve [bottomPx] at the bottom of the viewport (projection + render shift up). */
    external fun nativeSetViewportInset(handle: Long, bottomPx: Double)

    external fun nativeDestroy(handle: Long)

    // ── Control plane (the MapEngine contract) ──────────────────────────────
    external fun nativeSetCamera(handle: Long, lat: Double, lng: Double, zoom: Double, bearingDeg: Double)

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
}
