package com.sigmundgranaas.turbo.expressive.core.geo

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import kotlin.math.max
import kotlin.math.min

/**
 * A snapshot of how far a position is along a planned route. Replaces the old
 * global-nearest [GeoMetrics.progress], which broke on loops / out-and-back.
 */
data class RouteProgress(
    /** 0…1 along the route (monotonic — only climbs with forward progress). */
    val fraction: Double,
    /** Metres still to run, measured along the route. */
    val distanceRemainingM: Double,
    val etaSeconds: Int?,
    val offRoute: Boolean,
    val arrived: Boolean,
)

/**
 * Tracks a position along a planned route using a monotonic arc-length **cursor**.
 * Each fix is matched to the nearest route point *within a forward window* around
 * the cursor — not the global nearest — so returning to a start that coincides
 * with the end reads as ~100% / arrived, never 0% / moving-away. Stateful; feed
 * fixes in order. Mirrors the iOS `RouteProgressTracker`; both are pinned by the
 * shared progress fixtures under `fixtures/tracking/progress/`.
 */
class RouteProgressTracker(
    private val route: List<LatLng>,
    private val ascentM: Double? = null,
    private val windowBackM: Double = 60.0,
    private val windowAheadM: Double = 400.0,
    private val offRouteM: Double = 50.0,
    private val arriveEndM: Double = 30.0,
    private val offRouteStreakNeeded: Int = 3,
) {
    private val cumulative: DoubleArray = DoubleArray(route.size)
    private val total: Double
    private var cursor = 0.0
    private var offRouteStreak = 0

    init {
        for (i in 1 until route.size) {
            cumulative[i] = cumulative[i - 1] + GeoMetrics.haversineMeters(route[i - 1], route[i])
        }
        total = if (route.isEmpty()) 0.0 else cumulative.last()
    }

    val fraction: Double get() = if (total > 0.0) min(max(cursor / total, 0.0), 1.0) else 0.0

    /** Advance the cursor with a new position and return the resulting progress. */
    fun update(position: LatLng): RouteProgress {
        if (route.size < 2 || total <= 0.0) {
            return RouteProgress(0.0, total, null, offRoute = false, arrived = false)
        }
        val lo = cursor - windowBackM
        val hi = cursor + windowAheadM
        var bestDist = Double.MAX_VALUE
        var bestS = cursor

        for (i in 0 until route.size - 1) {
            val segStart = cumulative[i]
            val segEnd = cumulative[i + 1]
            if (segEnd < lo || segStart > hi) continue // skip segments outside the window
            val (proj, t) = GeoMetrics.projectFraction(route[i], route[i + 1], position)
            val sHere = segStart + (segEnd - segStart) * t
            val d = GeoMetrics.haversineMeters(proj, position)
            if (sHere in lo..hi && d < bestDist) {
                bestDist = d
                bestS = sHere
            }
        }

        cursor = max(cursor, bestS) // monotonic; small backtracks don't yo-yo the number

        val onRoute = bestDist <= offRouteM
        offRouteStreak = if (onRoute) 0 else offRouteStreak + 1
        val offRoute = offRouteStreak >= offRouteStreakNeeded

        val remaining = max(0.0, total - cursor)
        val frac = fraction
        val eta = GeoMetrics.etaSeconds(remaining, ascentM?.let { it * (1 - frac) })
        val atEnd = cursor >= total - arriveEndM &&
            GeoMetrics.haversineMeters(position, route.last()) <= arriveEndM

        return RouteProgress(frac, remaining, eta, offRoute, atEnd)
    }
}
