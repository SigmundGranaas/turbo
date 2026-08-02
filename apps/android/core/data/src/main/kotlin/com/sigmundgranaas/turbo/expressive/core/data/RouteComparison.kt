package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RouteDivergence
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min

/**
 * How far apart two routes are, for the divergence metric.
 *
 * Both engines read the same pack through the same solver, so agreement
 * is expected — but the server's graph is cut nationally and the
 * device's per region, so a boundary edge can differ without either
 * being wrong. The question is whether a walker would notice, which is
 * a geometric question, not a hash one.
 */
object RouteComparison {

    /**
     * Discrete Fréchet distance in metres, plus both lengths.
     *
     * Returns null if either line is empty — no comparison exists, and
     * returning 0.0 would report perfect agreement between a route and
     * nothing at all, which is the most misleading answer available.
     */
    fun compare(server: List<LatLng>, device: List<LatLng>): RouteDivergence? {
        if (server.isEmpty() || device.isEmpty()) return null
        val serverLen = lengthM(server)
        val deviceLen = lengthM(device)
        // Resample BOTH before comparing. Discrete Fréchet only ever
        // matches vertex to vertex, so it is sensitive to how densely a
        // line happens to be sampled: two identical paths, one drawn
        // with a vertex every 50 m and one with a vertex every 200 m,
        // score 100 m apart purely because the sparse line has no vertex
        // near the dense line's midpoint. Since the two engines densify
        // differently, that is not a corner case — it is every route.
        // Resampling to a common spacing makes the discrete measure
        // approximate the continuous one to within that spacing.
        val step = spacingFor(max(serverLen, deviceLen))
        return RouteDivergence(
            serverLengthM = serverLen,
            deviceLengthM = deviceLen,
            frechetM = discreteFrechetM(resample(server, step), resample(device, step)),
        )
    }

    /**
     * Vertex spacing for the comparison, in metres.
     *
     * [MIN_SPACING_M] on any ordinary route, growing only when a line is
     * long enough that a fixed spacing would make the O(n*m) table
     * expensive: at 10 m a 40 km route is 4 000 points a side, and 16 M
     * cells is more than a diagnostic should spend behind a route the
     * user already has.
     */
    private fun spacingFor(lengthM: Double): Double =
        max(MIN_SPACING_M, lengthM / MAX_SAMPLES)

    /**
     * Points every [step] metres along the line, endpoints included.
     *
     * Interpolates along segments rather than dropping or duplicating
     * vertices, so the resampled line follows the same path — which is
     * the only property the comparison needs from it.
     */
    private fun resample(line: List<LatLng>, step: Double): List<LatLng> {
        if (line.size < 2) return line
        val out = ArrayList<LatLng>(line.size)
        out += line.first()
        var carry = 0.0
        for (i in 1 until line.size) {
            val a = line[i - 1]
            val b = line[i]
            val segLen = lengthM(listOf(a, b))
            if (segLen <= 0.0) continue
            var t = step - carry
            while (t <= segLen) {
                val f = t / segLen
                out += LatLng(a.lat + (b.lat - a.lat) * f, a.lng + (b.lng - a.lng) * f)
                t += step
            }
            carry = (carry + segLen) % step
        }
        out += line.last()
        return out
    }

    /**
     * Discrete Fréchet distance, iteratively.
     *
     * The textbook formulation is recursive with memoisation; this is
     * the same recurrence filled row by row. Deliberate: routes here run
     * to hundreds of points, and the recursive form would go that many
     * frames deep on a device thread whose stack is not the JVM's
     * default. A stack overflow inside the *diagnostics* would be an
     * absurd way to lose a route.
     *
     * Only two rows are ever live, so the memory is O(n) rather than
     * O(n*m) — 500x500 doubles would be 2 MB per comparison otherwise.
     */
    private fun discreteFrechetM(p: List<LatLng>, q: List<LatLng>): Double {
        val n = p.size
        val m = q.size
        // Project once, around the shared centre. Both lines are the same
        // route, so one origin is right for both, and doing the cos()
        // per pair would be n*m transcendentals for no extra accuracy.
        val lat0 = (p.first().lat + q.first().lat) / 2.0
        val kx = KM_PER_DEG * 1000.0 * cos(Math.toRadians(lat0))
        val ky = KM_PER_DEG * 1000.0

        var prev = DoubleArray(m)
        var curr = DoubleArray(m)

        for (i in 0 until n) {
            for (j in 0 until m) {
                val d = hypot((p[i].lng - q[j].lng) * kx, (p[i].lat - q[j].lat) * ky)
                curr[j] = when {
                    i == 0 && j == 0 -> d
                    i == 0 -> max(curr[j - 1], d)
                    j == 0 -> max(prev[j], d)
                    else -> max(min(min(prev[j], prev[j - 1]), curr[j - 1]), d)
                }
            }
            val swap = prev
            prev = curr
            curr = swap
        }
        return prev[m - 1]
    }

    /** Polyline length in metres, equirectangular. */
    fun lengthM(line: List<LatLng>): Double {
        if (line.size < 2) return 0.0
        var total = 0.0
        for (i in 1 until line.size) {
            val a = line[i - 1]
            val b = line[i]
            val midLat = (a.lat + b.lat) / 2.0
            val dx = (b.lng - a.lng) * KM_PER_DEG * 1000.0 * cos(Math.toRadians(midLat))
            val dy = (b.lat - a.lat) * KM_PER_DEG * 1000.0
            total += hypot(dx, dy)
        }
        return total
    }

    /** Comparison resolution; also the measure's accuracy floor. */
    private const val MIN_SPACING_M = 10.0

    /** Cap on points per side, so a very long route stays cheap. */
    private const val MAX_SAMPLES = 2_000

    private const val KM_PER_DEG = 111.320
}
