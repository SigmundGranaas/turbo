package com.sigmundgranaas.turbo.expressive.domain

import org.junit.Assert.assertEquals
import org.junit.Test

class WeatherSummaryTest {

    private fun point(time: String, temp: Double?, precip: Double?, symbol: String?) =
        AtmosphericPoint(
            timeIso = time, temperatureC = temp, windSpeedMs = null, windFromDeg = null,
            humidityPct = null, cloudCoverPct = null, uvIndex = null,
            precipitation1hMm = precip, symbol1h = symbol,
        )

    @Test
    fun `groups by date with min-max temp, summed precip, and midday symbol`() {
        val pts = listOf(
            point("2026-06-06T06:00:00Z", -2.0, 0.0, "clearsky_day"),
            point("2026-06-06T12:00:00Z", 8.0, 1.5, "cloudy"),
            point("2026-06-06T18:00:00Z", 4.0, 0.5, "lightrain"),
            point("2026-06-07T12:00:00Z", 10.0, 0.0, "fair_day"),
        )
        val days = WeatherSummary.dailySummaries(pts)

        assertEquals(2, days.size)
        val d0 = days[0]
        assertEquals("2026-06-06", d0.date)
        assertEquals(-2.0, d0.minTempC!!, 1e-9)
        assertEquals(8.0, d0.maxTempC!!, 1e-9)
        assertEquals(2.0, d0.totalPrecipMm, 1e-9)
        assertEquals("cloudy", d0.middaySymbol) // nearest 12:00
        assertEquals("2026-06-07", days[1].date)
    }

    @Test
    fun `days are sorted and tolerate missing fields`() {
        val pts = listOf(
            point("2026-06-08T09:00:00Z", null, null, null),
            point("2026-06-06T09:00:00Z", 1.0, null, "fair_day"),
        )
        val days = WeatherSummary.dailySummaries(pts)
        assertEquals(listOf("2026-06-06", "2026-06-08"), days.map { it.date })
        assertEquals(0.0, days[1].totalPrecipMm, 1e-9) // null precip treated as 0
        assertEquals(null, days[1].middaySymbol)
    }
}
