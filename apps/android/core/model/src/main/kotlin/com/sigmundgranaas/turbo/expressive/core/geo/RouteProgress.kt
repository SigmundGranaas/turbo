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
 * A checkpoint crossing: when the cursor passed a planned-route phase (waypoint/marker), with
 * the split (time + distance) since the previous phase — split-times like a running watch (US-3).
 */
data class PhaseSplit(
    val index: Int,
    val name: String,
    val crossedAtEpochMs: Long,
    /** Distance (m) covered since the previous crossed phase. */
    val splitDistanceM: Double,
    /** Time (s) since the previous crossed phase. */
    val splitSeconds: Int,
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
    /** Phase (checkpoint) positions in route order; the cursor passing one marks a crossing (US-3). */
    phasePositions: List<LatLng> = emptyList(),
) {
    private val cumulative: DoubleArray = DoubleArray(route.size)
    private val total: Double
    private var cursor = 0.0
    private var offRouteStreak = 0

    /** Arc-length of each phase (its projection onto the route), in input order. */
    val phaseArcLengths: List<Double>

    /** How many phases the monotonic cursor has passed so far. */
    var passedPhaseCount: Int = 0
        private set

    init {
        for (i in 1 until route.size) {
            cumulative[i] = cumulative[i - 1] + GeoMetrics.haversineMeters(route[i - 1], route[i])
        }
        total = if (route.isEmpty()) 0.0 else cumulative.last()
        phaseArcLengths = phasePositions.map { arcLengthOf(it) }
    }

    /** Arc-length of [p]'s nearest projection onto the route (where a phase sits along it). */
    private fun arcLengthOf(p: LatLng): Double {
        if (route.size < 2) return 0.0
        var bestDist = Double.MAX_VALUE
        var bestArc = 0.0
        for (i in 1 until route.size) {
            val (proj, t) = GeoMetrics.projectFraction(route[i - 1], route[i], p)
            val d = GeoMetrics.haversineMeters(p, proj)
            if (d < bestDist) {
                bestDist = d
                bestArc = cumulative[i - 1] + (cumulative[i] - cumulative[i - 1]) * t
            }
        }
        return bestArc
    }

    val fraction: Double get() = if (total > 0.0) min(max(cursor / total, 0.0), 1.0) else 0.0

    /** Index of the next not-yet-crossed phase (== how many are crossed). */
    val nextPhaseIndex: Int get() = passedPhaseCount

    /** Distance (m) still to run to reach phase [index] along the route, or null if no such phase. */
    fun distanceToPhase(index: Int): Double? = phaseArcLengths.getOrNull(index)?.let { max(0.0, it - cursor) }

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
        passedPhaseCount = phaseArcLengths.count { it <= cursor }

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
