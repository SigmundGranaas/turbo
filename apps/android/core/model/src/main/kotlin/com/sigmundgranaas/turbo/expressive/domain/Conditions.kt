package com.sigmundgranaas.turbo.expressive.domain

/** Current weather at a point, from MET Norway's locationforecast. */
data class WeatherNow(
    val temperatureC: Double?,
    val windSpeedMs: Double?,
    val windFromDeg: Double?,
    val precipitationMm: Double?,
    /** MET symbol code, e.g. "partlycloudy_day"; null if unknown. */
    val symbolCode: String?,
    val humidityPct: Double? = null,
    val cloudCoverPct: Double? = null,
    val uvIndex: Double? = null,
)

/** One hour of the forecast (an entry in MET's timeseries). */
data class AtmosphericPoint(
    /** ISO-8601 UTC instant, e.g. "2026-06-06T12:00:00Z". */
    val timeIso: String,
    val temperatureC: Double?,
    val windSpeedMs: Double?,
    val windFromDeg: Double?,
    val humidityPct: Double?,
    val cloudCoverPct: Double?,
    val uvIndex: Double?,
    /** Precipitation in the next hour (mm); null at the tail where MET drops it. */
    val precipitation1hMm: Double?,
    /** Symbol for the next-1h window, e.g. "lightrain"; null at the tail. */
    val symbol1h: String?,
) {
    /** Local calendar date "yyyy-MM-dd" (ISO instant prefix). */
    val date: String get() = timeIso.take(10)

    /** Hour-of-day 0–23 parsed from the ISO instant; -1 if unparseable. */
    val hour: Int get() = timeIso.substringAfter('T', "").take(2).toIntOrNull() ?: -1
}

/** A per-day rollup derived from [AtmosphericPoint]s. */
data class DailySummary(
    val date: String,
    val minTempC: Double?,
    val maxTempC: Double?,
    val totalPrecipMm: Double,
    /** Representative symbol (nearest midday), e.g. "cloudy". */
    val middaySymbol: String?,
)

/** A multi-day forecast: the raw hourly [points] plus derived daily [days]. */
data class WeatherForecast(
    val points: List<AtmosphericPoint>,
    val days: List<DailySummary>,
)

/** Today's avalanche danger at a point, from NVE Varsom. */
data class AvalancheNow(
    val dangerLevel: Int,
    val mainText: String,
    val region: String,
)

/** Combined conditions for a point; either field may be null if unavailable. */
data class Conditions(
    val weather: WeatherNow?,
    val avalanche: AvalancheNow?,
)
