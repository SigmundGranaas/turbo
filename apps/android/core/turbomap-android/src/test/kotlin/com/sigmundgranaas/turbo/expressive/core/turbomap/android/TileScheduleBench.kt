package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Scheduling diagnostic suite (Part B). Drives the REAL [planReconcile] through
 * a discrete-event simulation of the tile pipeline to measure what the policy
 * actually delivers — DEM time-to-first-paint, viewport-coverage time, and
 * wasted (cancelled) fetches — under steady and adversarial (fast-zoom) traces.
 *
 * The latency model is seeded from the Rust `tile_profiler` network measurement
 * (Part A), including the key finding: the DEM endpoint (our tileserver) suffers
 * CONGESTION COLLAPSE past ~8 concurrent — p50 430ms @8 → 1948ms @16 → 9558ms
 * @64 — while the raster CDN scales cleanly to ~32. So latency here is a
 * function of how many same-kind fetches are in flight, which is what makes
 * "separate lanes with opposite caps" win over "one big pool".
 *
 * Deterministic (no RNG) → stable baseline + regression gate, runnable with no
 * device:
 *   ./gradlew :core:turbomap-android:testDebugUnitTest --tests '*TileScheduleBench'
 */
class TileScheduleBench {

    // ── Latency model (ms), measured by tile_profiler against the real hosts.
    private companion object {
        // Raster CDN: flat ~110ms warm to conc 32 (scales); 134ms cold.
        const val RAS_MS = 110.0
        // DEM tileserver: ~250ms base warm, but congestion-collapses past a knee
        // of ~8 concurrent, climbing ~150ms per extra concurrent connection.
        const val DEM_BASE_MS = 250.0
        const val DEM_KNEE = 8
        const val DEM_CONGEST_SLOPE = 150.0
        const val DEM_COLD_MS = 2054.0 // first-touch generate (cache-busted p50)
        const val RECONCILE_TICK = 16L // vsync-paced reconcile, ms
    }

    private enum class Kind { RASTER, DEM }
    private data class Tile(val kind: Kind, val z: Int, val x: Int, val y: Int, val distRank: Int) {
        val key get() = "${if (kind == Kind.DEM) "__terrain" else "base"}/$z/$x/$y"
    }

    /**
     * Per-fetch latency given how many same-kind fetches are already in flight
     * (congestion). Raster is flat (CDN scales); DEM climbs steeply past its
     * knee (server compute-bound). `cold` uses first-touch generate cost.
     * A per-key hash adds deterministic ±20% jitter so percentiles are realistic.
     */
    private fun latency(t: Tile, sameKindInFlight: Int, cold: Boolean): Long {
        val jitter = 0.8 + 0.4 * ((t.key.hashCode().toLong() and 0xFFFF) / 65535.0)
        val base = when {
            t.kind == Kind.RASTER -> RAS_MS
            cold -> DEM_COLD_MS
            else -> DEM_BASE_MS
        }
        val congestion = if (t.kind == Kind.DEM) {
            maxOf(0, sameKindInFlight - DEM_KNEE) * DEM_CONGEST_SLOPE
        } else {
            0.0
        }
        return ((base + congestion) * jitter).toLong().coerceAtLeast(1)
    }

    private data class Outcome(
        val demFirstPaintMs: Long,
        val demFullMs: Long,
        val viewportCoverageMs: Long,
        val started: Int,
        val cancelled: Int,
    )

    /**
     * Discrete-event sim driving the REAL [planReconcile] per lane. [pools] maps
     * a lane name → its cap (one entry = single shared pool; two = raster/DEM
     * lanes). [laneOf] assigns a key to its lane.
     */
    private fun simulate(
        trace: List<Pair<Long, List<Tile>>>,
        pools: Map<String, Int>,
        laneOf: (String) -> String,
        finalViewport: List<Tile>,
        cold: Boolean,
    ): Outcome {
        val byKey = (trace.flatMap { it.second } + finalViewport).associateBy { it.key }
        val inFlight = HashMap<String, Long>() // key → completion time
        val retryAt = HashMap<String, Long>()
        val done = HashSet<String>()
        var started = 0
        var cancelled = 0
        var demFirstPaint = -1L
        val finalKeys = finalViewport.map { it.key }
        val finalDem = finalViewport.filter { it.kind == Kind.DEM }.map { it.key }

        fun desiredAt(t: Long): List<Tile> =
            trace.lastOrNull { it.first <= t }?.second ?: emptyList()

        val end = trace.last().first + 60_000
        var t = 0L
        var demFull = -1L
        var coverage = -1L
        while (t <= end) {
            val landed = inFlight.filterValues { it <= t }.keys.toList()
            for (k in landed) {
                inFlight.remove(k)
                done.add(k)
                if (byKey[k]?.kind == Kind.DEM && demFirstPaint < 0 && k in finalDem) demFirstPaint = t
            }
            if (demFull < 0 && finalDem.isNotEmpty() && done.containsAll(finalDem)) demFull = t
            if (coverage < 0 && finalKeys.isNotEmpty() && done.containsAll(finalKeys)) coverage = t
            if (demFull >= 0 && coverage >= 0) break

            val desired = desiredAt(t).map { it.key }.filter { it !in done }
            for ((lane, cap) in pools) {
                val laneDesired = desired.filter { laneOf(it) == lane }
                val laneInFlight = inFlight.keys.filter { laneOf(it) == lane }.toSet()
                val decision = planReconcile(laneDesired, laneInFlight, retryAt, t, cap)
                decision.toCancel.forEach { if (inFlight.remove(it) != null) cancelled++ }
                // Count same-kind in-flight as we add, so congestion reflects the
                // real concurrent load the server would see.
                var sameKind = inFlight.keys.count { byKey[it]?.kind == byKey[decision.toStart.firstOrNull()]?.kind }
                decision.toStart.forEach { k ->
                    byKey[k]?.let {
                        inFlight[k] = t + latency(it, sameKind, cold)
                        sameKind++
                        started++
                    }
                }
            }
            t += RECONCILE_TICK
        }
        return Outcome(
            if (demFirstPaint < 0) -1 else demFirstPaint,
            demFull, coverage, started, cancelled,
        )
    }

    private fun viewport(z: Int, cx: Int, cy: Int, radius: Int): List<Tile> {
        val out = ArrayList<Tile>()
        for (dy in -radius..radius) for (dx in -radius..radius) {
            val rank = kotlin.math.abs(dx) + kotlin.math.abs(dy)
            out.add(Tile(Kind.RASTER, z, cx + dx, cy + dy, rank))
            out.add(Tile(Kind.DEM, z, cx + dx, cy + dy, rank))
        }
        return out
    }

    private fun rasterFirstOrder(v: List<Tile>): List<Tile> =
        v.filter { it.kind == Kind.RASTER }.sortedBy { it.distRank } +
            v.filter { it.kind == Kind.DEM }.sortedBy { it.distRank }

    private fun interleavedOrder(v: List<Tile>): List<Tile> =
        v.sortedWith(compareBy({ it.distRank }, { it.kind != Kind.RASTER }))

    @Test
    fun report_and_gate() {
        val sb = StringBuilder("\n========== TILE SCHEDULING BENCH (real planReconcile) ==========\n")
        sb.append("latency: RAS ${RAS_MS.toInt()}ms flat | DEM ${DEM_BASE_MS.toInt()}ms base, ")
            .append("+${DEM_CONGEST_SLOPE.toInt()}ms/conn past ${DEM_KNEE} (congestion collapse), ")
            .append("cold ${DEM_COLD_MS.toInt()}ms\n")

        val v = viewport(13, 4300, 2400, 3) // 49 raster + 49 DEM
        sb.append("\n-- Scenario 1: cold open of one viewport (${v.size} tiles), DEM cold-generate --\n")
        sb.append(row("CURRENT pool=8 raster-1st", simulate(listOf(0L to rasterFirstOrder(v)), shared(8), laneAll(), v, true)))
        sb.append(row("pool=8 interleaved", simulate(listOf(0L to interleavedOrder(v)), shared(8), laneAll(), v, true)))
        sb.append(row("pool=32 interleaved", simulate(listOf(0L to interleavedOrder(v)), shared(32), laneAll(), v, true)))
        sb.append(row("LANES ras=24,dem=6", simulate(listOf(0L to rasterFirstOrder(v)), lanes(24, 6), ::laneOf, v, true)))
        sb.append(row("LANES ras=32,dem=8", simulate(listOf(0L to rasterFirstOrder(v)), lanes(32, 8), ::laneOf, v, true)))

        // Fast zoom across terrain (warm-ish revisit), then settle.
        fun zoomTrace(order: (List<Tile>) -> List<Tile>): Pair<List<Pair<Long, List<Tile>>>, List<Tile>> {
            val steps = (0..6).map { i -> viewport(13, 4300 + i * 5, 2400 + i * 5, 3) }
            return steps.mapIndexed { i, vp -> (i * 100L) to order(vp) } to steps.last()
        }
        sb.append("\n-- Scenario 2: fast zoom/pan across terrain, then settle (warm) --\n")
        val (zCur, zCurF) = zoomTrace(::rasterFirstOrder)
        val (zInt, zIntF) = zoomTrace(::interleavedOrder)
        val warmCurrent = simulate(zCur, shared(8), laneAll(), zCurF, false)
        val warmBigPool = simulate(zInt, shared(32), laneAll(), zIntF, false)
        val warmLanes = simulate(zCur, lanes(32, 8), ::laneOf, zCurF, false)
        sb.append(row("CURRENT pool=8 raster-1st", warmCurrent))
        sb.append(row("pool=32 interleaved", warmBigPool))
        sb.append(row("LANES ras=32,dem=8 (rec.)", warmLanes))

        sb.append("\ncolumns: demFirstPaint / demFull / viewportCovered (ms) | started / cancelled\n")
        sb.append("(−1 = never within the run window)\n")
        sb.append("RECOMMENDATION (from Part-A saturation + this sim): SEPARATE LANES,\n")
        sb.append("  raster max≈32 / min≈20, DEM max≈8 (the congestion knee) / min≈4.\n")
        sb.append("  One big shared pool collapses warm DEM (pool=32 above); DEM<8 starves cold bulk.\n")
        sb.append("================================================================\n")
        println(sb)

        // Gate the RECOMMENDED config (lanes 32/8) against the two failure modes
        // the measurement exposed:
        //  1) it must terminate — cover the viewport + paint DEM (no stall);
        //  2) on warm fast-zoom it must NOT collapse DEM the way a big shared
        //     pool does (the congestion trap), i.e. ≤ both current and big-pool.
        assertTrue("lanes cover viewport", warmLanes.viewportCoverageMs in 0..60_000)
        assertTrue("lanes paint DEM", warmLanes.demFullMs in 0..60_000)
        assertTrue(
            "lanes(32,8) must avoid the big-pool DEM congestion collapse " +
                "(bigPool=${warmBigPool.demFullMs} lanes=${warmLanes.demFullMs})",
            warmLanes.demFullMs <= warmBigPool.demFullMs,
        )
        assertTrue(
            "lanes(32,8) must be at least as good as the current shared pool on warm DEM-full " +
                "(current=${warmCurrent.demFullMs} lanes=${warmLanes.demFullMs})",
            warmLanes.demFullMs <= warmCurrent.demFullMs,
        )
    }

    private fun shared(cap: Int) = mapOf("all" to cap)
    private fun lanes(r: Int, d: Int) = mapOf("raster" to r, "dem" to d)
    private fun laneAll(): (String) -> String = { "all" }
    private fun laneOf(key: String) = if (key.startsWith("__terrain")) "dem" else "raster"

    private fun row(label: String, o: Outcome): String =
        "  %-26s %6d / %6d / %6d ms | %4d started / %3d cancelled\n"
            .format(label, o.demFirstPaintMs, o.demFullMs, o.viewportCoverageMs, o.started, o.cancelled)
}
