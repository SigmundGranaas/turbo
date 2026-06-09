package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.domain.AtmosphericPoint
import com.sigmundgranaas.turbo.expressive.domain.AvalancheNow
import com.sigmundgranaas.turbo.expressive.domain.AvalancheProblem
import com.sigmundgranaas.turbo.expressive.domain.Conditions
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.MarineNow
import com.sigmundgranaas.turbo.expressive.domain.WeatherForecast
import com.sigmundgranaas.turbo.expressive.domain.WeatherNow
import com.sigmundgranaas.turbo.expressive.domain.WeatherSummary
import java.time.ZonedDateTime
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import javax.inject.Inject
import kotlin.math.cos
import kotlin.math.sin

/**
 * Offline stand-in for [ConditionsRepository] — the MET / Varsom / ocean clients
 * can't be reached from the emulator (DNS) and only cover Norway, so the whole
 * conditions UX (now-card, hourly + daily forecast, avalanche detail, marine)
 * otherwise shows only its "unavailable offline" state. This fabricates a
 * plausible, deterministic forecast from the coordinate + clock so the screens
 * can be driven anywhere. Selected in DEBUG via NetworkModule.
 */
class SyntheticConditionsRepository @Inject constructor() : ConditionsRepository {

    override suspend fun forPoint(point: LatLng): Outcome<Conditions> {
        val base = baseTempC(point)
        return Outcome.Success(
            Conditions(
                weather = WeatherNow(
                    temperatureC = base,
                    windSpeedMs = 4.5,
                    windFromDeg = 225.0,
                    precipitationMm = 0.4,
                    symbolCode = "partlycloudy_day",
                    humidityPct = 72.0,
                    cloudCoverPct = 45.0,
                    uvIndex = 2.0,
                ),
                // Level 3 so the card always shows (the real repo suppresses L1/L2-when-warm).
                avalanche = AvalancheNow(
                    dangerLevel = 3,
                    mainText = "Considerable — wind slabs on lee aspects; avoid steep loaded gullies.",
                    region = "Simulated region",
                    problems = listOf(
                        AvalancheProblem("Wind slab", "Low additional load", "Specific N–E aspects", "Size 2"),
                        AvalancheProblem("Persistent weak layer", "Natural", "Widespread", "Size 3"),
                    ),
                ),
                marine = syntheticMarine(),
            ),
        )
    }

    override suspend fun marine(point: LatLng): Outcome<MarineNow?> = Outcome.Success(syntheticMarine())

    private fun syntheticMarine() =
        MarineNow(waveHeightM = 0.8, waveFromDeg = 270.0, seaTemperatureC = 9.5, seaCurrentSpeedMs = 0.4)

    override suspend fun forecast(point: LatLng): Outcome<WeatherForecast> {
        val start = ZonedDateTime.now(ZoneOffset.UTC).withMinute(0).withSecond(0).withNano(0)
        val base = baseTempC(point)
        val points = (0 until HOURS).map { h ->
            val t = start.plusHours(h.toLong())
            val hour = t.hour
            // Diurnal swing (warmest mid-afternoon), a slow precip wave, cycling symbols.
            val temp = base + 4.0 * sin((hour - 9) / 24.0 * 2 * Math.PI)
            val precip = (0.6 * (1 + sin(h / 7.0))).coerceAtLeast(0.0).takeIf { h % 6 < 3 } ?: 0.0
            AtmosphericPoint(
                timeIso = t.format(ISO),
                temperatureC = "%.1f".format(temp).toDouble(),
                windSpeedMs = 3.0 + 2.0 * (1 + cos(h / 5.0)),
                windFromDeg = (200 + h * 7 % 160).toDouble(),
                humidityPct = 60.0 + 20.0 * (1 + sin(h / 6.0)) / 2,
                cloudCoverPct = 30.0 + 50.0 * (1 + sin(h / 4.0)) / 2,
                uvIndex = if (hour in 9..16) 2.0 else 0.0,
                precipitation1hMm = precip,
                symbol1h = symbolFor(hour, precip),
            )
        }
        return Outcome.Success(WeatherForecast(points = points, days = WeatherSummary.dailySummaries(points)))
    }

    /** Colder the further north, with a small east/west wobble — feels location-specific. */
    private fun baseTempC(point: LatLng): Double = "%.1f".format(18.0 - (point.lat - 60.0) * 0.8).toDouble()

    private fun symbolFor(hour: Int, precip: Double): String = when {
        precip > 0.5 -> "rain"
        precip > 0.0 -> "lightrain"
        hour in 7..18 -> "partlycloudy_day"
        else -> "partlycloudy_night"
    }

    private companion object {
        const val HOURS = 72
        val ISO: DateTimeFormatter = DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm:ss'Z'")
    }
}
