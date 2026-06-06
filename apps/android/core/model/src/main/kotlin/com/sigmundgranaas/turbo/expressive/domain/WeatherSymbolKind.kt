package com.sigmundgranaas.turbo.expressive.domain

/** Coarse weather families that MET's ~90 symbol codes collapse into for iconography. */
enum class WeatherKind { Clear, PartlyCloudy, Cloudy, Fog, Rain, Sleet, Snow, Thunder, Unknown }

/**
 * Maps a MET symbol code (e.g. "lightrainshowers_day", "heavysnowandthunder") to a
 * [WeatherKind]. Order matters: precipitation/thunder are detected before cloud
 * cover so combined codes resolve to the more salient hazard. The `_day` / `_night`
 * / `_polartwilight` suffix is ignored.
 */
fun classifyWeatherSymbol(code: String?): WeatherKind {
    val c = code?.lowercase() ?: return WeatherKind.Unknown
    return when {
        "thunder" in c -> WeatherKind.Thunder
        "snow" in c -> WeatherKind.Snow
        "sleet" in c -> WeatherKind.Sleet
        "rain" in c || "drizzle" in c -> WeatherKind.Rain
        "fog" in c -> WeatherKind.Fog
        "partlycloudy" in c -> WeatherKind.PartlyCloudy
        "cloudy" in c -> WeatherKind.Cloudy
        "fair" in c || "clearsky" in c -> WeatherKind.Clear
        else -> WeatherKind.Unknown
    }
}
