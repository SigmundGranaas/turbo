package com.sigmundgranaas.turbo.expressive.core.geo

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class GeoMetricsTest {

    @Test
    fun `haversine of identical points is zero`() {
        assertEquals(0.0, GeoMetrics.haversineMeters(LatLng(69.6, 18.9), LatLng(69.6, 18.9)), 1e-6)
    }

    @Test
    fun `one degree of latitude is about 111 km`() {
        val d = GeoMetrics.haversineMeters(LatLng(0.0, 0.0), LatLng(1.0, 0.0))
        assertEquals(111_195.0, d, 500.0)
    }

    @Test
    fun `path length sums segments`() {
        val pts = listOf(LatLng(0.0, 0.0), LatLng(0.0, 1.0), LatLng(0.0, 2.0))
        val total = GeoMetrics.pathLengthMeters(pts)
        val seg = GeoMetrics.haversineMeters(LatLng(0.0, 0.0), LatLng(0.0, 1.0))
        assertEquals(seg * 2, total, 1.0)
    }

    @Test
    fun `gain and loss accumulate only deltas of the right sign`() {
        val (asc, desc) = GeoMetrics.gainLoss(listOf(100.0, 120.0, 110.0, 130.0, null, 90.0))
        assertEquals(40.0, asc!!, 1e-6) // +20 (100->120), +20 (110->130)
        assertEquals(50.0, desc!!, 1e-6) // -10 (120->110), -40 (130->90, null skipped)
    }

    @Test
    fun `naismith eta grows with ascent`() {
        val flat = GeoMetrics.etaSeconds(5000.0, 0.0)
        val climby = GeoMetrics.etaSeconds(5000.0, 600.0)
        assertEquals(3600, flat)
        assertTrue(climby > flat)
    }

    @Test
    fun `progress at start is zero and near end approaches one`() {
        val pts = listOf(LatLng(0.0, 0.0), LatLng(0.0, 1.0), LatLng(0.0, 2.0))
        val atStart = GeoMetrics.progress(pts, LatLng(0.0, 0.0))!!
        val nearEnd = GeoMetrics.progress(pts, LatLng(0.0, 1.99))!!
        assertEquals(0.0, atStart.fraction, 1e-3)
        assertTrue(nearEnd.fraction > 0.9)
    }

    @Test
    fun `distanceToPath is ~0 on the line and large when off it`() {
        val route = listOf(LatLng(69.0, 18.0), LatLng(69.0, 18.01))
        // A point on the segment.
        assertTrue(GeoMetrics.distanceToPath(route, LatLng(69.0, 18.005)) < 1.0)
        // ~111 m north of the line (0.001° latitude).
        val off = GeoMetrics.distanceToPath(route, LatLng(69.001, 18.005))
        assertTrue("expected ~111 m, was $off", off in 90.0..130.0)
    }

    @Test
    fun `estimateKcal grows with distance and ascent and is zero when idle`() {
        assertEquals(0, GeoMetrics.estimateKcal(0.0, 0.0))
        // 5 km flat for a 70 kg hiker ≈ 0.5 * 70 * 5 = 175 kcal.
        assertEquals(175, GeoMetrics.estimateKcal(5_000.0, 0.0))
        // Adding 400 m of climb burns strictly more.
        assertTrue(GeoMetrics.estimateKcal(5_000.0, 400.0) > GeoMetrics.estimateKcal(5_000.0, 0.0))
    }
}
