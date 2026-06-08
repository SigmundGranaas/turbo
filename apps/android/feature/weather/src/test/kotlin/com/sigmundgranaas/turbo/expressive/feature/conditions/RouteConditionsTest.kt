package com.sigmundgranaas.turbo.expressive.feature.conditions

import com.sigmundgranaas.turbo.expressive.domain.AvalancheNow
import com.sigmundgranaas.turbo.expressive.domain.Conditions
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.WeatherNow
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RouteConditionsTest {

    private fun line(n: Int) = (0 until n).map { LatLng(60.0 + it * 0.01, 8.0 + it * 0.01) }

    @Test
    fun `sampleAlong keeps first and last and caps the count`() {
        val pts = sampleAlong(line(100), 4)
        assertEquals(4, pts.size)
        assertEquals(LatLng(60.0, 8.0), pts.first())
        assertEquals(LatLng(60.0 + 99 * 0.01, 8.0 + 99 * 0.01), pts.last())
    }

    @Test
    fun `sampleAlong returns the whole short line without padding`() {
        assertEquals(2, sampleAlong(line(2), 4).size)
        assertTrue(sampleAlong(emptyList(), 4).isEmpty())
    }

    @Test
    fun `aggregate reports temp range and the worst danger`() {
        val conds = listOf(
            cond(temp = -2.0, danger = 1),
            cond(temp = 5.0, danger = 3),
            cond(temp = 1.0, danger = 2),
        )
        val summary = aggregateConditions(conds)
        assertEquals(-2.0, summary.tempMinC!!, 0.001)
        assertEquals(5.0, summary.tempMaxC!!, 0.001)
        assertEquals(3, summary.worstDanger)
        assertTrue(summary.hasData)
    }

    @Test
    fun `aggregate ignores level-0 (no rating) and missing data`() {
        val summary = aggregateConditions(
            listOf(
                cond(temp = null, danger = 0),
                cond(temp = 4.0, danger = 0),
            ),
        )
        assertEquals(4.0, summary.tempMinC!!, 0.001)
        assertNull(summary.worstDanger)
    }

    @Test
    fun `aggregate of nothing has no data`() {
        val summary = aggregateConditions(listOf(Conditions(weather = null, avalanche = null)))
        assertNull(summary.tempMinC)
        assertNull(summary.worstDanger)
        assertTrue(!summary.hasData)
    }

    private fun cond(temp: Double?, danger: Int) = Conditions(
        weather = WeatherNow(temperatureC = temp, windSpeedMs = null, windFromDeg = null, precipitationMm = null, symbolCode = null),
        avalanche = AvalancheNow(dangerLevel = danger, mainText = "", region = "Test"),
    )
}
