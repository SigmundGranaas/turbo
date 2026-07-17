package com.sigmundgranaas.turbo.expressive.domain

/**
 * A weather pin's cached forecast — the minimal slice of MET's [Conditions] a pin
 * needs to render offline: the standard readout (temp + condition glyph) plus the
 * expanded fields (wind, precipitation, and marine/wave data when coastal). Stored on
 * the [Marker] node so the pin paints instantly from cache and only re-fetches when stale.
 */
data class WeatherSnapshot(
    // ── standard view ──
    val temperatureC: Double?,
    /** MET symbol code, e.g. "partlycloudy_day"; drives the condition glyph. */
    val symbolCode: String?,
    // ── expanded view ──
    val windSpeedMs: Double?,
    val windFromDeg: Double?,
    val precipitationMm: Double?,
    // ── expanded: marine (null inland) ──
    val waveHeightM: Double? = null,
    val waveFromDeg: Double? = null,
    val seaTemperatureC: Double? = null,
) {
    companion object {
        /** Project a live [Conditions] fetch down to the fields a weather pin caches. */
        fun from(conditions: Conditions): WeatherSnapshot = WeatherSnapshot(
            temperatureC = conditions.weather?.temperatureC,
            symbolCode = conditions.weather?.symbolCode,
            windSpeedMs = conditions.weather?.windSpeedMs,
            windFromDeg = conditions.weather?.windFromDeg,
            precipitationMm = conditions.weather?.precipitationMm,
            waveHeightM = conditions.marine?.waveHeightM,
            waveFromDeg = conditions.marine?.waveFromDeg,
            seaTemperatureC = conditions.marine?.seaTemperatureC,
        )
    }
}

/** How long a cached forecast stays "fresh" — past this age a weather pin re-fetches (when online). */
const val WEATHER_PIN_STALE_MS: Long = 60L * 60L * 1000L // 1 hour

/** The decision a weather pin makes on open: hit the network, or paint from cache. */
enum class WeatherPinFetch { Fetch, UseCache }

/**
 * The pure drivable seam behind "live + cached". A weather pin fetches only when it is
 * **online** and the cache is either **absent** or **stale** (older than [WEATHER_PIN_STALE_MS]);
 * offline it always keeps the cache (never throws, never blanks). [cacheAgeMs] is `null` when
 * the pin has never been fetched (forcing a fetch when online).
 */
fun weatherPinFetchDecision(
    cacheAgeMs: Long?,
    online: Boolean,
    staleAfterMs: Long = WEATHER_PIN_STALE_MS,
): WeatherPinFetch = when {
    !online -> WeatherPinFetch.UseCache
    cacheAgeMs == null -> WeatherPinFetch.Fetch
    cacheAgeMs > staleAfterMs -> WeatherPinFetch.Fetch
    else -> WeatherPinFetch.UseCache
}

/**
 * The render-ready state of a weather pin, derived purely from its cached [WeatherSnapshot]
 * and the current time. The standard slice is always present when a snapshot exists; the
 * [expanded] slice carries the wind/precip/marine detail. `null` when the pin has no cache yet.
 */
data class WeatherPinUiState(
    val temperatureC: Double?,
    val symbolCode: String?,
    /** Whole hours since the cache was fetched; `null` when never fetched. */
    val updatedHoursAgo: Long?,
    val expanded: Expanded,
) {
    data class Expanded(
        val windSpeedMs: Double?,
        val windFromDeg: Double?,
        val precipitationMm: Double?,
        val waveHeightM: Double?,
        val waveFromDeg: Double?,
        val seaTemperatureC: Double?,
    ) {
        /** Whether this pin sits on the coast (MET returned marine data to surface). */
        val hasMarine: Boolean
            get() = waveHeightM != null || seaTemperatureC != null
    }
}

/**
 * Build the render state for a weather pin from its cached forecast. Pure and total —
 * returns `null` only when there is no cache to show (a freshly dropped, still-fetching pin).
 */
fun weatherPinUiState(marker: Marker, nowMs: Long): WeatherPinUiState? {
    val snap = marker.forecast ?: return null
    val ageMs = marker.forecastFetchedAtEpochMs?.let { (nowMs - it).coerceAtLeast(0L) }
    return WeatherPinUiState(
        temperatureC = snap.temperatureC,
        symbolCode = snap.symbolCode,
        updatedHoursAgo = ageMs?.let { it / (60L * 60L * 1000L) },
        expanded = WeatherPinUiState.Expanded(
            windSpeedMs = snap.windSpeedMs,
            windFromDeg = snap.windFromDeg,
            precipitationMm = snap.precipitationMm,
            waveHeightM = snap.waveHeightM,
            waveFromDeg = snap.waveFromDeg,
            seaTemperatureC = snap.seaTemperatureC,
        ),
    )
}
