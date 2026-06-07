package com.sigmundgranaas.turbo.expressive.feature.conditions

import com.sigmundgranaas.turbo.expressive.domain.Conditions
import com.sigmundgranaas.turbo.expressive.domain.LatLng

/**
 * Conditions sampled across a whole journey/route, not just one point — the
 * coldest/warmest air you'll meet and the worst avalanche danger anywhere along
 * the line. Lets the route card answer "what am I walking into?" at a glance.
 */
data class RouteConditions(
    val tempMinC: Double?,
    val tempMaxC: Double?,
    /** Highest Varsom danger level seen on any sample (1–5); null if none reported. */
    val worstDanger: Int?,
    /** How many sample points returned usable data. */
    val samples: Int,
) {
    val hasData: Boolean get() = tempMinC != null || worstDanger != null
}

/**
 * Evenly-spaced sample points along [geometry] by vertex index — cheap, and the
 * MET grid (~1 km) is coarse enough that exact spacing doesn't matter for an
 * overview. Always includes the first and last vertex. Returns up to [count]
 * points (fewer if the line is shorter), de-duplicating coincident picks.
 */
fun sampleAlong(geometry: List<LatLng>, count: Int): List<LatLng> {
    if (geometry.isEmpty()) return emptyList()
    val n = count.coerceAtLeast(1)
    if (geometry.size <= n) return geometry.toList()
    if (n == 1) return listOf(geometry.first())
    return (0 until n)
        .map { i -> geometry[(i.toLong() * (geometry.size - 1) / (n - 1)).toInt()] }
        .distinct()
}

/** Roll up per-point [conditions] into a single along-route summary. */
fun aggregateConditions(conditions: List<Conditions>): RouteConditions {
    val temps = conditions.mapNotNull { it.weather?.temperatureC }
    // Level 0 means "no rating issued" in our model — exclude it from the worst-of.
    val dangers = conditions.mapNotNull { it.avalanche?.dangerLevel }.filter { it > 0 }
    return RouteConditions(
        tempMinC = temps.minOrNull(),
        tempMaxC = temps.maxOrNull(),
        worstDanger = dangers.maxOrNull(),
        samples = conditions.size,
    )
}
