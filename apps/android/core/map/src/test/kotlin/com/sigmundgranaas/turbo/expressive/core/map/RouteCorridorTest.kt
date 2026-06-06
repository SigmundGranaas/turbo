package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RouteCorridorTest {

    @Test
    fun `null for fewer than two points`() {
        assertNull(RouteCorridor.bounds(emptyList()))
        assertNull(RouteCorridor.bounds(listOf(LatLng(60.0, 10.0))))
    }

    @Test
    fun `box encloses all points and is padded outward`() {
        val pts = listOf(LatLng(60.0, 10.0), LatLng(60.2, 10.4))
        val b = RouteCorridor.bounds(pts, paddingMeters = 1000.0)!!
        assertTrue(b.south < 60.0 && b.north > 60.2)
        assertTrue(b.west < 10.0 && b.east > 10.4)
    }

    @Test
    fun `padding widens the box`() {
        val pts = listOf(LatLng(60.0, 10.0), LatLng(60.2, 10.4))
        val small = RouteCorridor.bounds(pts, 500.0)!!
        val big = RouteCorridor.bounds(pts, 5000.0)!!
        assertTrue(RouteCorridor.spanDegrees(big) > RouteCorridor.spanDegrees(small))
    }
}
