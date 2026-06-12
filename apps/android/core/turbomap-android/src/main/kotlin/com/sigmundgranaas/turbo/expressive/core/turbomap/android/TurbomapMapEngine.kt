package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.MapEngine

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
) : MapEngine {

    private var bottomInsetPx = 0

    /** The host calls this when the surface is resized so [visibleBounds] stays correct. */
    fun onResized(width: Int, height: Int) {
        widthPx = width
        heightPx = height
    }

    /** `[lat, lng, zoom, bearingDeg]`, or empty if the handle is gone. */
    private fun camera(): DoubleArray = NativeSurfaceMap.nativeCamera(handle)

    private fun setCamera(lat: Double, lng: Double, zoom: Double, bearingDeg: Double) =
        NativeSurfaceMap.nativeSetCamera(handle, lat, lng, zoom, bearingDeg)

    override fun zoomIn() = camera().let { if (it.size >= 4) setCamera(it[0], it[1], it[2] + 1.0, it[3]) }

    override fun zoomOut() = camera().let { if (it.size >= 4) setCamera(it[0], it[1], it[2] - 1.0, it[3]) }

    override fun flyTo(target: LatLng, zoom: Double) {
        recenter(target.lat, target.lng, zoom, camera().getOrElse(3) { 0.0 })
    }

    /**
     * Centre on (lat,lng) at [zoom], honouring [setBottomInset]: when a bottom inset is
     * set (the live sheet), shift the camera so the target sits in the visible band above
     * the sheet instead of behind it. Adapter-side framing only — the engine projection is
     * untouched (overlays still project truthfully), which is enough for "centre on me".
     */
    private fun recenter(lat: Double, lng: Double, zoom: Double, bearing: Double) {
        setCamera(lat, lng, zoom, bearing)
        if (bottomInsetPx > 0 && heightPx > 0) {
            val r = NativeSurfaceMap.nativeUnproject(handle, widthPx / 2.0, heightPx / 2.0 + bottomInsetPx / 2.0)
            if (r.size >= 3 && r[2] == 1.0) setCamera(r[0], r[1], zoom, bearing)
        }
    }

    override fun center(): LatLng = camera().let { if (it.size >= 2) LatLng(it[0], it[1]) else LatLng(0.0, 0.0) }

    override fun zoom(): Double = camera().getOrElse(2) { 0.0 }

    override fun bearing(): Double = camera().getOrElse(3) { 0.0 }

    override fun resetNorth() = camera().let { if (it.size >= 4) setCamera(it[0], it[1], it[2], 0.0) }

    override fun fromScreen(xPx: Float, yPx: Float): LatLng {
        val r = NativeSurfaceMap.nativeUnproject(handle, xPx.toDouble(), yPx.toDouble())
        return if (r.size >= 3 && r[2] == 1.0) LatLng(r[0], r[1]) else center()
    }

    override fun toScreen(point: LatLng): Pair<Float, Float> {
        val r = NativeSurfaceMap.nativeProject(handle, point.lat, point.lng)
        return if (r.size >= 3 && r[2] == 1.0) r[0].toFloat() to r[1].toFloat() else 0f to 0f
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

    /** Reserve [bottomPx] for an overlay (the live sheet); honoured by [flyTo]/[frameTo]. */
    override fun setBottomInset(bottomPx: Int) {
        bottomInsetPx = bottomPx.coerceAtLeast(0)
    }

    override fun frameTo(points: List<LatLng>, paddingPx: Int) {
        when {
            points.isEmpty() -> Unit
            points.size == 1 -> flyTo(points.first(), DEFAULT_POINT_ZOOM)
            else -> {
                // Approximate fit: centre on the centroid at the current zoom. A
                // precise bounds-fit lands with the on-screen host (Stage E remainder).
                val cam = camera()
                recenter(
                    points.map { it.lat }.average(),
                    points.map { it.lng }.average(),
                    cam.getOrElse(2) { DEFAULT_POINT_ZOOM },
                    cam.getOrElse(3) { 0.0 },
                )
            }
        }
    }

    private companion object {
        const val DEFAULT_POINT_ZOOM = 14.0
    }
}
