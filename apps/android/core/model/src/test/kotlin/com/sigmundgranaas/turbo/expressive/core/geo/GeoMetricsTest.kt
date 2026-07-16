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
    fun `gain and loss ignore metre-scale gps jitter`() {
        // Vertical GPS noise oscillates ±1–2 m around the true elevation on every
        // fix; a naive delta sum turns an hour of that into phantom climb.
        val jitter = listOf(100.0, 101.5, 99.0, 100.5, 99.5, 101.0, 100.0)
        val (asc, desc) = GeoMetrics.gainLoss(jitter)
        assertEquals(0.0, asc!!, 1e-6)
        assertEquals(0.0, desc!!, 1e-6)
    }

    @Test
    fun `gain keeps the full height of a slow steady climb`() {
        // 100 → 112 m in 2 m steps: smaller than the hysteresis band per fix,
        // but the committed reference must ratchet so nothing is lost overall.
        val climb = (0..6).map { 100.0 + it * 2.0 }
        val (asc, desc) = GeoMetrics.gainLoss(climb)
        assertEquals(12.0, asc!!, 1e-6)
        assertEquals(0.0, desc!!, 1e-6)
    }

    @Test
    fun `naismith eta grows with ascent`() {
        val flat = GeoMetrics.etaSeconds(5000.0, 0.0)
        val climby = GeoMetrics.etaSeconds(5000.0, 600.0)
        assertEquals(3600, flat)
        assertTrue(climby > flat)
    }

    @Test
    fun `routePrefix splits the guide at the cursor fraction`() {
        val route = listOf(LatLng(0.0, 0.0), LatLng(0.0, 1.0), LatLng(0.0, 2.0)) // 2 units long
        assertTrue("empty at the start", GeoMetrics.routePrefix(route, 0.0).isEmpty())
        assertEquals("whole route at the end", route, GeoMetrics.routePrefix(route, 1.0))
        val half = GeoMetrics.routePrefix(route, 0.5)
        // Half-way is the middle vertex (lng ~1.0): start + the split point.
        assertEquals(2, half.size)
        assertEquals(0.0, half.first().lng, 1e-6)
        assertTrue("ends near the midpoint but was ${half.last().lng}", kotlin.math.abs(half.last().lng - 1.0) < 0.02)
    }

    @Test
    fun `routeSuffix is the complement of routePrefix and they meet at the cursor`() {
        val route = listOf(LatLng(0.0, 0.0), LatLng(0.0, 1.0), LatLng(0.0, 2.0))
        assertEquals("whole route at the start", route, GeoMetrics.routeSuffix(route, 0.0))
        assertTrue("empty at the end", GeoMetrics.routeSuffix(route, 1.0).isEmpty())
        val prefix = GeoMetrics.routePrefix(route, 0.5)
        val suffix = GeoMetrics.routeSuffix(route, 0.5)
        // The covered prefix's last point == the remaining suffix's first point (the cursor).
        assertEquals(prefix.last().lng, suffix.first().lng, 1e-9)
        assertEquals(prefix.last().lat, suffix.first().lat, 1e-9)
        assertTrue("suffix runs to the end", kotlin.math.abs(suffix.last().lng - 2.0) < 1e-9)
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
    fun `arcLengthAlong returns where a point projects along the route`() {
        val route = listOf(LatLng(69.0, 18.0), LatLng(69.0, 18.01))
        val total = GeoMetrics.pathLengthMeters(route)
        // Start projects to ~0, end to ~total, midpoint to ~half.
        assertTrue(GeoMetrics.arcLengthAlong(route, LatLng(69.0, 18.0)) < 1.0)
        assertEquals(total, GeoMetrics.arcLengthAlong(route, LatLng(69.0, 18.01)), 1.0)
        assertEquals(total / 2, GeoMetrics.arcLengthAlong(route, LatLng(69.0, 18.005)), 1.0)
        // An off-route point projects to its nearest along-track position, not 0.
        val off = GeoMetrics.arcLengthAlong(route, LatLng(69.001, 18.005))
        assertEquals(total / 2, off, 5.0)
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
