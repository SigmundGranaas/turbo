package com.sigmundgranaas.turbo.expressive.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The four numbers release 2's decision rests on.
 *
 * Each test here is about a *denominator*, because that is where this
 * kind of metric goes wrong: a rate computed over the wrong population
 * is not obviously broken, it is just quietly misleading, and it gets
 * quoted in a decision months later.
 */
class RouteSolveStatsTest {

    private fun rec(
        engine: RouteEngine,
        lane: SolveLane,
        durationMs: Long = 100,
        spanKm: Double = 5.0,
        outcome: RouteSolveRecord.Outcome = RouteSolveRecord.Outcome.Ok,
        divergence: RouteDivergence? = null,
    ) = RouteSolveRecord(
        engine = engine,
        durationMs = durationMs,
        waypoints = 2,
        spanKm = spanKm,
        lane = lane,
        outcome = outcome,
        divergence = divergence,
    )

    /**
     * The fallback rate counts only solves where a fallback was possible.
     *
     * This is the whole reason [SolveLane] exists. An offline solve and a
     * no-pack solve both "did not fall back", but neither could have, and
     * including them makes the server look more reliable the more the
     * user walks out of coverage.
     */
    @Test
    fun `fallback rate excludes solves that could never have fallen back`() {
        val stats = RouteSolveStats.from(
            listOf(
                rec(RouteEngine.Server, SolveLane.ServerAnswered),
                rec(RouteEngine.Device, SolveLane.ServerTimedOut),
                // Neither of these belongs in the denominator.
                rec(RouteEngine.Device, SolveLane.Offline),
                rec(RouteEngine.Server, SolveLane.NoPack),
            ),
        )
        assertEquals(2, stats.fallbackEligible)
        assertEquals(0.5, stats.fallbackRate, 1e-9)
    }

    /** A forced solve is a tester's choice, not evidence about Auto. */
    @Test
    fun `forced solves are excluded from every automatic rate`() {
        val stats = RouteSolveStats.from(
            listOf(
                rec(RouteEngine.Device, SolveLane.Forced),
                rec(RouteEngine.Device, SolveLane.Forced),
                rec(RouteEngine.Server, SolveLane.ServerAnswered),
            ),
        )
        assertEquals(1, stats.fallbackEligible)
        assertEquals(0.0, stats.fallbackRate, 1e-9)
        assertEquals("only the automatic solve counts", 0.0, stats.coverageMissRate, 1e-9)
        assertEquals("but totals still see them", 3, stats.total)
    }

    /** A transport error is a fallback just as much as a timeout is. */
    @Test
    fun `a server error counts as a fallback`() {
        val stats = RouteSolveStats.from(
            listOf(
                rec(RouteEngine.Device, SolveLane.ServerErrored),
                rec(RouteEngine.Server, SolveLane.ServerAnswered),
            ),
        )
        assertEquals(0.5, stats.fallbackRate, 1e-9)
    }

    @Test
    fun `coverage misses are counted over automatic solves`() {
        val stats = RouteSolveStats.from(
            listOf(
                rec(RouteEngine.Server, SolveLane.NoPack),
                rec(RouteEngine.Server, SolveLane.NoPack),
                rec(RouteEngine.Server, SolveLane.ServerAnswered),
                rec(RouteEngine.Device, SolveLane.Forced),
            ),
        )
        assertEquals(2.0 / 3.0, stats.coverageMissRate, 1e-9)
    }

    /**
     * A 2 km solve and a 40 km solve are different questions, so they
     * must not share a percentile.
     */
    @Test
    fun `latency is bucketed by distance`() {
        val stats = RouteSolveStats.from(
            listOf(
                rec(RouteEngine.Device, SolveLane.Offline, durationMs = 50, spanKm = 1.0),
                rec(RouteEngine.Device, SolveLane.Offline, durationMs = 9000, spanKm = 30.0),
            ),
        )
        assertEquals(50L, stats.devicePercentiles[DistanceBucket.Under2]!!.p95Ms)
        assertEquals(9000L, stats.devicePercentiles[DistanceBucket.Over25]!!.p95Ms)
        assertTrue(
            "an empty bucket is absent, not zero",
            !stats.devicePercentiles.containsKey(DistanceBucket.Under10),
        )
    }

    /**
     * Nearest-rank, so p95 is a measurement that happened.
     *
     * With 20 samples the naive `size * 0.95` index is 19 — the last
     * element — which would make p95 and max the same number for every
     * bucket and hide the tail this metric exists to expose.
     */
    @Test
    fun `p95 picks an observed value not an interpolated one`() {
        val s = LatencySummary.of((1L..20L).map { it * 100 })
        assertEquals(2000L, s.maxMs)
        assertEquals(1900L, s.p95Ms)
        assertEquals(1000L, s.medianMs)
        assertTrue("p95 must be a real observation", s.p95Ms % 100L == 0L)
    }

    @Test
    fun `an empty history has no rates rather than NaN`() {
        val stats = RouteSolveStats.from(emptyList())
        assertEquals(0.0, stats.fallbackRate, 0.0)
        assertEquals(0.0, stats.coverageMissRate, 0.0)
        assertEquals(0.0, stats.failureRate, 0.0)
        assertEquals(0, stats.total)
    }

    @Test
    fun `divergences are surfaced with the worst one`() {
        val stats = RouteSolveStats.from(
            listOf(
                rec(
                    RouteEngine.Server, SolveLane.ServerAnswered,
                    divergence = RouteDivergence(1000.0, 1010.0, 12.0),
                ),
                rec(
                    RouteEngine.Server, SolveLane.ServerAnswered,
                    divergence = RouteDivergence(1000.0, 1400.0, 220.0),
                ),
                rec(RouteEngine.Server, SolveLane.ServerAnswered),
            ),
        )
        assertEquals(2, stats.divergences.size)
        assertEquals(220.0, stats.worstDivergenceM, 1e-9)
        assertEquals(1, stats.significantDivergences)
    }
}
