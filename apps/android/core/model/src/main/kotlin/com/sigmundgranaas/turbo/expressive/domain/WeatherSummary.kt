package com.sigmundgranaas.turbo.expressive.domain

import kotlin.math.abs

/** Derives per-day [DailySummary] rollups from hourly [AtmosphericPoint]s. Pure. */
object WeatherSummary {

    private const val MIDDAY_HOUR = 12

    fun dailySummaries(points: List<AtmosphericPoint>): List<DailySummary> =
        points.groupBy { it.date }
            .toSortedMap()
            .map { (date, dayPoints) ->
                val temps = dayPoints.mapNotNull { it.temperatureC }
                val midday = dayPoints
                    .filter { it.symbol1h != null && it.hour >= 0 }
                    .minByOrNull { abs(it.hour - MIDDAY_HOUR) }
                    ?: dayPoints.firstOrNull { it.symbol1h != null }
                DailySummary(
                    date = date,
                    minTempC = temps.minOrNull(),
                    maxTempC = temps.maxOrNull(),
                    totalPrecipMm = dayPoints.sumOf { it.precipitation1hMm ?: 0.0 },
                    middaySymbol = midday?.symbol1h,
                )
            }
}
