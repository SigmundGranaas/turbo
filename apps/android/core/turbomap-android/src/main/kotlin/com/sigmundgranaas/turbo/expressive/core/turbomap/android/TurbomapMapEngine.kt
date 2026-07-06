package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.MapEngine
import com.sigmundgranaas.turbo.expressive.domain.MapHit
import com.sigmundgranaas.turbo.expressive.domain.TerrainSunOverlay
import com.sigmundgranaas.turbo.expressive.domain.WeatherCloudOverlay

/**
 * The wgpu/Rust implementation of [MapEngine] — the second engine behind the
 * seam, peer to MapLibre's `MapLibreEngine`. A thin view over an on-screen
 * surface map ([NativeSurfaceMap] handle); the host that created the handle
 * owns its lifecycle (`nativeDestroy`).
 *
 * Camera + projection go straight to the engine over JNI; the derived contract
 * methods (zoom step, north reset, visible box, frame-to-fit) are composed from
 * those primitives here so the Rust surface stays minimal.
 */
class TurbomapMapEngine(
    private val handle: Long,
    private var widthPx: Int,
    private var heightPx: Int,
) : MapEngine, WeatherCloudOverlay, TerrainSunOverlay {

    /**
     * Invoked after any camera/projection mutation so the host can request a
     * render (render-on-demand). The host wires this to its render-thread nudge.
     */
    var onMutated: () -> Unit = {}

    /**
     * The surface controller that owns the Scene document (plan P5.2). Sun,
     * shadow, and cloud mutations are SCENE state, so they route here and
     * become scene rebuilds — there are no imperative engine side-doors left.
     * Null (e.g. the bare contract test) → environment mutations no-op.
     */
    internal var environmentHost: TurbomapSurfaceController? = null

    /** The host calls this when the surface is resized so [visibleBounds] stays correct. */
    fun onResized(width: Int, height: Int) {
        widthPx = width
        heightPx = height
    }

    /** `[lat, lng, zoom, bearingDeg]`, or empty if the handle is gone. */
    private fun camera(): DoubleArray = NativeSurfaceMap.nativeCamera(handle)

    /** Ease (accel/decel) to a pose — every programmatic move animates, never jumps. */
    private fun easeTo(lat: Double, lng: Double, zoom: Double, bearingDeg: Double) {
        NativeSurfaceMap.nativeEaseTo(handle, lat, lng, zoom, bearingDeg, EASE_DURATION_MS)
        onMutated()
    }

    private fun zoomBy(factor: Double) {
        NativeSurfaceMap.nativeZoomAroundAnimated(handle, factor, widthPx / 2.0, heightPx / 2.0, ZOOM_DURATION_MS)
        onMutated()
    }

    override fun zoomIn() = zoomBy(2.0) // +1 zoom level, eased about the centre

    override fun zoomOut() = zoomBy(0.5) // −1 zoom level

    override fun flyTo(target: LatLng, zoom: Double) {
        easeTo(target.lat, target.lng, zoom, camera().getOrElse(3) { 0.0 })
    }

    override fun center(): LatLng = camera().let { if (it.size >= 2) LatLng(it[0], it[1]) else LatLng(0.0, 0.0) }

    override fun zoom(): Double = camera().getOrElse(2) { 0.0 }

    override fun bearing(): Double = camera().getOrElse(3) { 0.0 }

    override fun resetNorth() = camera().let { if (it.size >= 4) easeTo(it[0], it[1], it[2], 0.0) }

    override fun fromScreen(xPx: Float, yPx: Float): LatLng {
        val r = NativeSurfaceMap.nativeUnproject(handle, xPx.toDouble(), yPx.toDouble())
        return if (r.size >= 3 && r[2] == 1.0) LatLng(r[0], r[1]) else center()
    }

    override fun screenToGround(xPx: Float, yPx: Float): LatLng {
        // [lat, lng, worldZ, hitTerrain, valid] — terrain raycast (falls back to flat).
        val r = NativeSurfaceMap.nativeUnprojectGround(handle, xPx.toDouble(), yPx.toDouble())
        return if (r.size >= 5 && r[4] == 1.0) LatLng(r[0], r[1]) else fromScreen(xPx, yPx)
    }

    override fun toScreen(point: LatLng): Pair<Float, Float> {
        val r = NativeSurfaceMap.nativeProject(handle, point.lat, point.lng)
        return if (r.size >= 3 && r[2] == 1.0) r[0].toFloat() to r[1].toFloat() else 0f to 0f
    }

    override fun hitTest(xPx: Float, yPx: Float, tolPx: Float): List<MapHit> {
        val json = NativeSurfaceMap.nativeHitTest(handle, xPx.toDouble(), yPx.toDouble(), tolPx.toDouble())
        return parseHits(json)
    }

    override fun visibleBounds(): GeoBounds {
        val w = widthPx.toDouble()
        val h = heightPx.toDouble()
        val corners = listOf(0.0 to 0.0, w to 0.0, 0.0 to h, w to h)
        val pts = corners.mapNotNull { (x, y) ->
            val r = NativeSurfaceMap.nativeUnproject(handle, x, y)
            if (r.size >= 3 && r[2] == 1.0) LatLng(r[0], r[1]) else null
        }
        if (pts.isEmpty()) {
            val c = center()
            return GeoBounds(south = c.lat, west = c.lng, north = c.lat, east = c.lng)
        }
        return GeoBounds(
            south = pts.minOf { it.lat },
            west = pts.minOf { it.lng },
            north = pts.maxOf { it.lat },
            east = pts.maxOf { it.lng },
        )
    }

    /**
     * Reserve [bottomPx] at the bottom of the viewport (the live sheet). Handled
     * engine-side: the projection and the GPU view-matrix shift the principal point
     * up by half the inset, so framing *and* overlay projection stay consistent —
     * `flyTo`/`frameTo` centre into the visible band above the sheet for free.
     */
    override fun setBottomInset(bottomPx: Int) {
        NativeSurfaceMap.nativeSetViewportInset(handle, bottomPx.coerceAtLeast(0).toDouble())
        onMutated()
    }

    override fun frameTo(points: List<LatLng>, paddingPx: Int) {
        when {
            points.isEmpty() -> Unit
            points.size == 1 -> flyTo(points.first(), DEFAULT_POINT_ZOOM)
            else -> {
                // Approximate fit: centre on the centroid at the current zoom. A
                // precise bounds-fit lands with the on-screen host (Stage E remainder).
                val cam = camera()
                easeTo(
                    points.map { it.lat }.average(),
                    points.map { it.lng }.average(),
                    cam.getOrElse(2) { DEFAULT_POINT_ZOOM },
                    cam.getOrElse(3) { 0.0 },
                )
            }
        }
    }

    // ── WeatherCloudOverlay ─────────────────────────────────────────────────
    // The overlay's DECLARATION (grid, geo box, visibility) is scene state →
    // [environmentHost] rebuilds + re-applies the Scene (which also redraws).
    // Frame data + the playback clock stay native verbs: data is transport
    // (like tiles) and the clock is control (like the camera).

    override fun enableClouds(gridW: Int, gridH: Int) {
        environmentHost?.enableClouds(gridW, gridH)
    }

    override fun setCloudsVisible(visible: Boolean) {
        environmentHost?.setCloudsVisible(visible)
    }

    override fun setCloudGeoBounds(west: Double, south: Double, east: Double, north: Double) {
        environmentHost?.setCloudGeoBounds(west, south, east, north)
    }

    override fun ingestRadarFrame(slot: Int, gridW: Int, gridH: Int, precip: ByteArray, coverage: ByteArray) {
        NativeSurfaceMap.nativeIngestRadarFrame(handle, slot, gridW, gridH, precip, coverage)
        onMutated()
    }

    override fun setCloudTime(timeSeconds: Float, blend: Float) {
        NativeSurfaceMap.nativeSetCloudTime(handle, timeSeconds, blend)
        onMutated()
    }

    // ── TerrainSunOverlay ───────────────────────────────────────────────────
    // Scene state (the environment's lighting + terrain-shadows) → the host.

    override fun setSunTime(unixSeconds: Double) {
        environmentHost?.setSunTime(unixSeconds)
    }

    override fun setTerrainShadows(strength: Float) {
        environmentHost?.setTerrainShadows(strength)
    }

    private companion object {
        const val DEFAULT_POINT_ZOOM = 14.0
        const val EASE_DURATION_MS = 450 // fly-to / locate / frame-to
        const val ZOOM_DURATION_MS = 250 // rail zoom step
    }
}

/** Parse the JNI hit-test JSON (`[{"layer","feature_id","properties"}]`). */
private fun parseHits(json: String): List<MapHit> = try {
    val arr = org.json.JSONArray(json)
    (0 until arr.length()).map { i ->
        val o = arr.getJSONObject(i)
        val props = o.optJSONObject("properties")
        MapHit(
            layer = o.optString("layer"),
            featureId = if (o.isNull("feature_id")) null else o.optString("feature_id"),
            properties = props?.let { p ->
                p.keys().asSequence().associateWith { k -> p.optString(k) }
            } ?: emptyMap(),
        )
    }
} catch (_: org.json.JSONException) {
    emptyList()
}
