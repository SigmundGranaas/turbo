package com.sigmundgranaas.turbo.expressive.core.geo

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin
import kotlin.math.sqrt

/** Live progress of a position along a path. */
data class JourneyProgress(
    val fraction: Double,
    val distanceRemainingM: Double,
    val etaSeconds: Int?,
)

/**
 * Single source of truth for path geometry math: haversine distance, elevation
 * gain/loss, ETA (Naismith-ish), and progress projection. Pure + testable.
 */
object GeoMetrics {
    private const val EARTH_RADIUS_M = 6_371_000.0

    fun haversineMeters(a: LatLng, b: LatLng): Double {
        val dLat = Math.toRadians(b.lat - a.lat)
        val dLng = Math.toRadians(b.lng - a.lng)
        val la1 = Math.toRadians(a.lat)
        val la2 = Math.toRadians(b.lat)
        val h = sin(dLat / 2) * sin(dLat / 2) + cos(la1) * cos(la2) * sin(dLng / 2) * sin(dLng / 2)
        return 2 * EARTH_RADIUS_M * atan2(sqrt(h), sqrt(1 - h))
    }

    fun pathLengthMeters(points: List<LatLng>): Double {
        if (points.size < 2) return 0.0
        var total = 0.0
        for (i in 1 until points.size) total += haversineMeters(points[i - 1], points[i])
        return total
    }

    /** Cumulative ascent/descent from a per-point elevation series (nulls skipped). */
    fun gainLoss(elevations: List<Double?>?): Pair<Double?, Double?> {
        if (elevations == null) return null to null
        var asc = 0.0
        var desc = 0.0
        var prev: Double? = null
        for (e in elevations) {
            val cur = e ?: continue
            val p = prev
            if (p != null) {
                val delta = cur - p
                if (delta > 0) asc += delta else desc += -delta
            }
            prev = cur
        }
        return asc to desc
    }

    /**
     * Naismith with Langmuir-style correction: 1 h / 5 km on the flat, +1 h per
     * 600 m ascent. Returns seconds.
     */
    fun etaSeconds(distanceM: Double, ascentM: Double?): Int {
        val flat = distanceM / 5000.0 * 3600.0
        val climb = (ascentM ?: 0.0) / 600.0 * 3600.0
        return (flat + climb).toInt()
    }

    /** Project [position] onto [points], returning fractional progress + remaining distance. */
    fun progress(points: List<LatLng>, position: LatLng, ascentM: Double? = null): JourneyProgress? {
        if (points.size < 2) return null
        val total = pathLengthMeters(points)
        if (total <= 0.0) return null
        var bestDist = Double.MAX_VALUE
        var bestAlong = 0.0
        var cum = 0.0
        for (i in 1 until points.size) {
            val a = points[i - 1]
            val b = points[i]
            val seg = haversineMeters(a, b)
            val (proj, t) = projectFraction(a, b, position)
            val d = haversineMeters(position, proj)
            if (d < bestDist) {
                bestDist = d
                bestAlong = cum + seg * t
            }
            cum += seg
        }
        val fraction = (bestAlong / total).coerceIn(0.0, 1.0)
        val remaining = total - bestAlong
        return JourneyProgress(
            fraction = fraction,
            distanceRemainingM = remaining,
            etaSeconds = etaSeconds(remaining, ascentM?.let { it * (1 - fraction) }),
        )
    }

    // Planar approximation of the closest point on segment a→b to p (fine at trail scale).
    private fun projectFraction(a: LatLng, b: LatLng, p: LatLng): Pair<LatLng, Double> {
        val ax = a.lng; val ay = a.lat
        val bx = b.lng; val by = b.lat
        val px = p.lng; val py = p.lat
        val dx = bx - ax; val dy = by - ay
        val len2 = dx * dx + dy * dy
        val t = if (len2 == 0.0) 0.0 else max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / len2))
        return LatLng(ay + dy * t, ax + dx * t) to t
    }
}
