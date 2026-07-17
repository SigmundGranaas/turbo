package com.sigmundgranaas.turbo.expressive.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Behavior of the weather-pin seams: when a pin re-fetches vs. paints from cache, and what
 * its cached forecast renders as. These are the pure units the ViewModel and render layer
 * sit on top of — asserted here without a device.
 */
class WeatherPinTest {

    private val hour = 60L * 60L * 1000L

    // ── weatherPinFetchDecision: the "use cache vs refetch" reducer ──

    @Test
    fun `offline always keeps the cache even when stale`() {
        assertEquals(WeatherPinFetch.UseCache, weatherPinFetchDecision(cacheAgeMs = 6 * hour, online = false))
    }

    @Test
    fun `offline with no cache still does not fetch`() {
        assertEquals(WeatherPinFetch.UseCache, weatherPinFetchDecision(cacheAgeMs = null, online = false))
    }

    @Test
    fun `online with no cache yet fetches`() {
        assertEquals(WeatherPinFetch.Fetch, weatherPinFetchDecision(cacheAgeMs = null, online = true))
    }

    @Test
    fun `online with a fresh cache paints from cache`() {
        assertEquals(WeatherPinFetch.UseCache, weatherPinFetchDecision(cacheAgeMs = 10 * 60 * 1000L, online = true))
    }

    @Test
    fun `online with a stale cache refetches`() {
        assertEquals(WeatherPinFetch.Fetch, weatherPinFetchDecision(cacheAgeMs = 2 * hour, online = true))
    }

    @Test
    fun `exactly one hour old is still fresh (boundary is strictly greater-than)`() {
        assertEquals(WeatherPinFetch.UseCache, weatherPinFetchDecision(cacheAgeMs = hour, online = true))
    }

    // ── weatherPinUiState: what the cache renders as ──

    private val coastalSnapshot = WeatherSnapshot(
        temperatureC = -2.0,
        symbolCode = "cloudy",
        windSpeedMs = 7.5,
        windFromDeg = 315.0,
        precipitationMm = 1.2,
        waveHeightM = 1.8,
        waveFromDeg = 270.0,
        seaTemperatureC = 6.0,
    )

    @Test
    fun `standard view shows the cached temperature and condition glyph`() {
        val marker = weatherPin(coastalSnapshot, fetchedAt = 1_000L)
        val ui = weatherPinUiState(marker, nowMs = 1_000L)!!
        assertEquals(-2.0, ui.temperatureC!!, 1e-9)
        assertEquals("cloudy", ui.symbolCode)
    }

    @Test
    fun `expanded view surfaces wind, precipitation and marine fields`() {
        val ui = weatherPinUiState(weatherPin(coastalSnapshot, fetchedAt = 0L), nowMs = 0L)!!
        val e = ui.expanded
        assertEquals(7.5, e.windSpeedMs!!, 1e-9)
        assertEquals(315.0, e.windFromDeg!!, 1e-9)
        assertEquals(1.2, e.precipitationMm!!, 1e-9)
        assertEquals(1.8, e.waveHeightM!!, 1e-9)
        assertEquals(6.0, e.seaTemperatureC!!, 1e-9)
        assertTrue("coastal pin exposes marine data", e.hasMarine)
    }

    @Test
    fun `an inland cache reports no marine data`() {
        val inland = coastalSnapshot.copy(waveHeightM = null, seaTemperatureC = null, waveFromDeg = null)
        val ui = weatherPinUiState(weatherPin(inland, fetchedAt = 0L), nowMs = 0L)!!
        assertTrue("inland pin has no marine section", !ui.expanded.hasMarine)
    }

    @Test
    fun `updated-hours-ago reflects the fetch timestamp for the "updated Nh ago" cue`() {
        val ui = weatherPinUiState(weatherPin(coastalSnapshot, fetchedAt = 0L), nowMs = 3 * hour)!!
        assertEquals(3L, ui.updatedHoursAgo)
    }

    @Test
    fun `a pin with no cache yet renders nothing (still fetching)`() {
        val bare = Marker(id = "w", name = "Weather pin", kind = ActivityKindId.Viewpoint, position = LatLng(69.6, 18.9), markerKind = MarkerKind.WeatherPin)
        assertNull(weatherPinUiState(bare, nowMs = 5_000L))
    }

    @Test
    fun `snapshot projection keeps temp, wind, precip and marine from a live fetch`() {
        val conditions = Conditions(
            weather = WeatherNow(temperatureC = 4.0, windSpeedMs = 3.0, windFromDeg = 90.0, precipitationMm = 0.5, symbolCode = "rain"),
            avalanche = null,
            marine = MarineNow(waveHeightM = 2.1, waveFromDeg = 200.0, seaTemperatureC = 7.0),
        )
        val snap = WeatherSnapshot.from(conditions)
        assertEquals(4.0, snap.temperatureC!!, 1e-9)
        assertEquals(3.0, snap.windSpeedMs!!, 1e-9)
        assertEquals(0.5, snap.precipitationMm!!, 1e-9)
        assertEquals(2.1, snap.waveHeightM!!, 1e-9)
        assertEquals("rain", snap.symbolCode)
    }

    private fun weatherPin(snapshot: WeatherSnapshot, fetchedAt: Long) = Marker(
        id = "w-1",
        name = "Weather pin",
        kind = ActivityKindId.Viewpoint,
        position = LatLng(69.6, 18.9),
        markerKind = MarkerKind.WeatherPin,
        forecast = snapshot,
        forecastFetchedAtEpochMs = fetchedAt,
    )
}
