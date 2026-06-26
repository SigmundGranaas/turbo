package com.sigmundgranaas.turbo.expressive.e2e

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.MapEngine

/**
 * A headless stand-in for the wgpu renderer. It draws nothing — it just *records*
 * what the app asked the map to do (where the camera flew, what was framed, the
 * sheet inset) so E2E tests can assert user-visible map behaviour ("the map
 * centred on the place I picked") without a GPU.
 */
class FakeMapEngine(start: LatLng = LatLng(67.28, 14.40)) : MapEngine {
    var lastFlyTo: LatLng? = null
        private set
    var lastFramedPoints: List<LatLng>? = null
        private set
    var bottomInsetPx: Int = 0
        private set

    private var center = start
    private var zoom = 12.0
    private var bearing = 0.0

    override fun zoomIn() { zoom += 1 }
    override fun zoomOut() { zoom -= 1 }

    override fun flyTo(target: LatLng, zoom: Double) {
        center = target
        this.zoom = zoom
        lastFlyTo = target
    }

    override fun center(): LatLng = center
    override fun fromScreen(xPx: Float, yPx: Float): LatLng = center
    override fun screenToGround(xPx: Float, yPx: Float): LatLng = center
    override fun toScreen(point: LatLng): Pair<Float, Float> = 0f to 0f
    override fun visibleBounds(): GeoBounds =
        GeoBounds(south = center.lat - 0.1, west = center.lng - 0.1, north = center.lat + 0.1, east = center.lng + 0.1)

    override fun setBottomInset(bottomPx: Int) { bottomInsetPx = bottomPx }
    override fun zoom(): Double = zoom
    override fun bearing(): Double = bearing
    override fun resetNorth() { bearing = 0.0 }

    override fun frameTo(points: List<LatLng>, paddingPx: Int) {
        lastFramedPoints = points
        points.firstOrNull()?.let { center = it }
    }
}
