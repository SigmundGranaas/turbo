package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * "Do the two engines agree?" as a geometric question.
 *
 * The requirement is equivalence, not bit-identity — which is why this
 * is Fréchet and not a hash. The tests below are mostly about the ways a
 * naive comparison would claim agreement that is not there.
 */
class RouteComparisonTest {

    private val lat = 67.06
    private val lng = 15.04

    /** ~1 m in latitude, near enough for these fixtures. */
    private fun north(m: Double) = LatLng(lat + m / 111_320.0, lng)

    @Test
    fun `identical lines have zero divergence`() {
        val line = listOf(north(0.0), north(100.0), north(200.0))
        val d = RouteComparison.compare(line, line)!!
        assertEquals(0.0, d.frechetM, 1e-6)
        assertEquals(0.0, d.lengthDeltaM, 1e-6)
        assertTrue(!d.isSignificant)
    }

    /**
     * Different point counts along the SAME path must still compare
     * equal.
     *
     * This is the case a per-index comparison gets wrong, and it is not
     * hypothetical: the two engines densify geometry differently, so
     * the same route routinely comes back with different point counts.
     * A comparison that flagged that would cry wolf on every route.
     */
    @Test
    fun `resampling the same path does not count as divergence`() {
        val coarse = listOf(north(0.0), north(200.0))
        val fine = listOf(north(0.0), north(50.0), north(100.0), north(150.0), north(200.0))
        val d = RouteComparison.compare(coarse, fine)!!
        // The tolerance is the comparison's resample spacing, not zero.
        // A raw discrete Fréchet scores these 100 m apart — the sparse
        // line simply has no vertex near the dense line's midpoint — so
        // this is the assertion that forced the resampling step, and
        // anything much above the spacing means it regressed.
        assertTrue("identical paths must agree within the spacing: ${d.frechetM}", d.frechetM <= 10.0)
        assertEquals(0.0, d.lengthDeltaM, 0.5)
    }

    /**
     * Sampling density must not change the answer.
     *
     * The generalisation of the case above: the same path at three very
     * different densities must score the same, because which engine
     * emitted more points says nothing about whether they agree.
     */
    @Test
    fun `density does not change the verdict`() {
        val path = { n: Int -> (0..n).map { north(it * 400.0 / n) } }
        val a = RouteComparison.compare(path(2), path(40))!!.frechetM
        val b = RouteComparison.compare(path(8), path(40))!!.frechetM
        val c = RouteComparison.compare(path(40), path(40))!!.frechetM
        assertTrue("2 vs 40 points: $a", a <= 10.0)
        assertTrue("8 vs 40 points: $b", b <= 10.0)
        assertTrue("40 vs 40 points: $c", c <= 10.0)
    }

    /**
     * Two routes of the SAME length that go different ways must diverge.
     *
     * The case a length-only check misses entirely — a detour around the
     * other side of a lake can come back the same distance, and it is
     * the most visible possible disagreement to a user.
     */
    @Test
    fun `equal length is not agreement`() {
        val straight = listOf(north(0.0), north(200.0))
        val detour = listOf(
            north(0.0),
            LatLng(lat + 100.0 / 111_320.0, lng + 200.0 / (111_320.0 * 0.39)),
            north(200.0),
        )
        val d = RouteComparison.compare(straight, detour)!!
        assertTrue("a real detour must register: ${d.frechetM}", d.frechetM > 100.0)
        assertTrue(d.isSignificant)
    }

    /** Fréchet is symmetric; which engine is "first" must not matter. */
    @Test
    fun `comparison is symmetric`() {
        val a = listOf(north(0.0), north(100.0), north(300.0))
        val b = listOf(north(0.0), north(180.0), north(300.0))
        val ab = RouteComparison.compare(a, b)!!.frechetM
        val ba = RouteComparison.compare(b, a)!!.frechetM
        assertEquals(ab, ba, 1e-6)
    }

    /**
     * No comparison rather than a fake perfect one.
     *
     * Returning 0.0 for "one engine produced nothing" would report
     * flawless agreement for the worst possible outcome.
     */
    @Test
    fun `an empty line yields no comparison`() {
        assertNull(RouteComparison.compare(emptyList(), listOf(north(0.0))))
        assertNull(RouteComparison.compare(listOf(north(0.0)), emptyList()))
    }

    @Test
    fun `length is measured in metres`() {
        assertEquals(200.0, RouteComparison.lengthM(listOf(north(0.0), north(200.0))), 1.0)
        assertEquals(0.0, RouteComparison.lengthM(listOf(north(0.0))), 0.0)
        assertEquals(0.0, RouteComparison.lengthM(emptyList()), 0.0)
    }

    /**
     * Hundreds of points must not overflow the stack.
     *
     * The textbook Fréchet is recursive, and a real route is 400+
     * points; losing a route to a StackOverflowError raised inside the
     * diagnostics would be an absurd failure.
     */
    @Test
    fun `a long route compares without blowing the stack`() {
        val a = (0..800).map { north(it * 10.0) }
        val b = (0..600).map { north(it * 13.0) }
        val d = RouteComparison.compare(a, b)!!
        assertTrue(d.frechetM.isFinite())
    }
}
