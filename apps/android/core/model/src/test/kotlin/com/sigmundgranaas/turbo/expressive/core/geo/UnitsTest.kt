package com.sigmundgranaas.turbo.expressive.core.geo

import org.junit.Assert.assertEquals
import org.junit.Test

class UnitsTest {

    @Test
    fun `metric distance uses km above 1000m and m below`() {
        assertEquals("1.5 km", Units.distance(1500.0, metric = true))
        assertEquals("450 m", Units.distance(450.0, metric = true))
    }

    @Test
    fun `imperial distance uses miles, falling back to feet when short`() {
        assertEquals("0.9 mi", Units.distance(1500.0, metric = false))
        // 30 m ≈ 98 ft, below the 0.1 mi threshold → feet.
        assertEquals("98 ft", Units.distance(30.0, metric = false))
    }

    @Test
    fun `elevation converts to feet when imperial`() {
        assertEquals("100 m", Units.elevation(100.0, metric = true))
        assertEquals("328 ft", Units.elevation(100.0, metric = false))
    }

    @Test
    fun `pace formats per km or per mile and guards undefined`() {
        // 1 km in 330 s → 5:30 /km
        assertEquals("5:30 /km", Units.pace(1000.0, 330, metric = true))
        assertEquals("—", Units.pace(0.0, 0, metric = true))
        assertEquals("/mi", Units.pace(1609.344, 600, metric = false).takeLast(3))
    }
}
