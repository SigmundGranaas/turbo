package com.sigmundgranaas.turbo.expressive.core.geo

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin
import kotlin.math.sqrt

/**
 * Single source of truth for path geometry math: haversine distance, elevation
 * gain/loss, ETA (Naismith-ish), and off-path distance. Pure + testable. Live
 * route progress lives in [RouteProgressTracker] (a monotonic arc-length cursor,
 * loop-safe) — the old global-nearest `progress`/`JourneyProgress` was removed.
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
     * Rough energy estimate (kcal) for a hike of [distanceM] with [ascentM] of
     * climb, for a ~70 kg hiker. Flat walking burns ≈ 0.5 kcal/kg/km; climbing
     * adds the potential energy of lifting that mass (mgh, ~25 % muscular
     * efficiency → ≈ 1.6 kcal per kg per 100 m of ascent). Deliberately simple
     * and surfaced as an estimate, not a measurement.
     */
    fun estimateKcal(distanceM: Double, ascentM: Double?, weightKg: Double = 70.0): Int {
        if (distanceM <= 0.0 && (ascentM ?: 0.0) <= 0.0) return 0
        val flat = 0.5 * weightKg * (distanceM / 1000.0)
        val climb = 0.016 * weightKg * (ascentM ?: 0.0)
        return (flat + climb).toInt()
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

    /**
     * The covered **prefix** of [route] up to [fraction] (0..1) of its total length, including
     * the interpolated split point at the cursor. Empty when fraction ≤ 0; the whole route when
     * ≥ 1. Used to dim the already-walked portion of a followed guide (US-3). Pure + tested.
     */
    fun routePrefix(route: List<LatLng>, fraction: Double): List<LatLng> {
        if (route.size < 2 || fraction <= 0.0) return emptyList()
        val total = pathLengthMeters(route)
        if (total <= 0.0) return emptyList()
        if (fraction >= 1.0) return route
        val target = total * fraction
        val out = ArrayList<LatLng>(route.size)
        out.add(route[0])
        var cum = 0.0
        for (i in 1 until route.size) {
            val seg = haversineMeters(route[i - 1], route[i])
            if (cum + seg < target) {
                out.add(route[i])
                cum += seg
            } else {
                val t = if (seg > 0.0) (target - cum) / seg else 0.0
                val a = route[i - 1]
                val b = route[i]
                out.add(LatLng(a.lat + (b.lat - a.lat) * t, a.lng + (b.lng - a.lng) * t))
                break
            }
        }
        return out
    }

    /**
     * The remaining **suffix** of [route] from [fraction] (0..1) onward, including the
     * interpolated split point at the cursor. The whole route when fraction ≤ 0; empty when
     * ≥ 1. Complements [routePrefix] so the guide can be drawn as two segments — covered
     * (dim) + remaining (bright) — that meet exactly at the cursor (US-3). Pure + tested.
     */
    fun routeSuffix(route: List<LatLng>, fraction: Double): List<LatLng> {
        if (route.size < 2 || fraction >= 1.0) return emptyList()
        if (fraction <= 0.0) return route
        val total = pathLengthMeters(route)
        if (total <= 0.0) return route
        val target = total * fraction
        val out = ArrayList<LatLng>(route.size)
        var cum = 0.0
        var started = false
        for (i in 1 until route.size) {
            val seg = haversineMeters(route[i - 1], route[i])
            if (!started && cum + seg >= target) {
                val t = if (seg > 0.0) (target - cum) / seg else 0.0
                val a = route[i - 1]
                val b = route[i]
                out.add(LatLng(a.lat + (b.lat - a.lat) * t, a.lng + (b.lng - a.lng) * t))
                out.add(b)
                started = true
            } else if (started) {
                out.add(route[i])
            }
            cum += seg
        }
        return out
    }

    /** Shortest distance (metres) from [position] to the polyline [points]; ∞ if degenerate. */
    fun distanceToPath(points: List<LatLng>, position: LatLng): Double {
        if (points.isEmpty()) return Double.MAX_VALUE
        if (points.size == 1) return haversineMeters(points[0], position)
        var best = Double.MAX_VALUE
        for (i in 1 until points.size) {
            val (proj, _) = projectFraction(points[i - 1], points[i], position)
            val d = haversineMeters(position, proj)
            if (d < best) best = d
        }
        return best
    }

    // Planar approximation of the closest point on segment a→b to p (fine at trail scale).
    internal fun projectFraction(a: LatLng, b: LatLng, p: LatLng): Pair<LatLng, Double> {
        val ax = a.lng; val ay = a.lat
        val bx = b.lng; val by = b.lat
        val px = p.lng; val py = p.lat
        val dx = bx - ax; val dy = by - ay
        val len2 = dx * dx + dy * dy
        val t = if (len2 == 0.0) 0.0 else max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / len2))
        return LatLng(ay + dy * t, ax + dx * t) to t
    }
}
