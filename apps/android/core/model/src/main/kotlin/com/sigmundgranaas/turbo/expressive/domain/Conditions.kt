package com.sigmundgranaas.turbo.expressive.domain

/** Current weather at a point, from MET Norway's locationforecast. */
data class WeatherNow(
    val temperatureC: Double?,
    val windSpeedMs: Double?,
    val windFromDeg: Double?,
    val precipitationMm: Double?,
    /** MET symbol code, e.g. "partlycloudy_day"; null if unknown. */
    val symbolCode: String?,
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
