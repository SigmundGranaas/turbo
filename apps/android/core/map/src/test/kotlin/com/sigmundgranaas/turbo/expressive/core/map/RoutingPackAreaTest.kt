package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.RoutingPack
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The client's copy of the server's size rule.
 *
 * `RoutingPack.areaSqKm` and `PackKey::area_sq_km` are the same formula
 * written twice in two languages, and the whole value of the client's
 * copy rests on them agreeing. The client checks locally so it never
 * asks for a region the server would refuse; if it computed a *smaller*
 * area than the server it would ask anyway and fail the very download
 * the check exists to protect.
 *
 * Two copies of a formula cannot share a test, so they share numbers.
 * These come from `area_reference_values_for_the_android_side` in
 * `turbo-geodata-pack`; changing one side without the other fails here
 * or there.
 */
class RoutingPackAreaTest {

    /** Tolerance in km². Tighter than any rounding these two do differently. */
    private val eps = 0.5

    @Test
    fun `the area formula agrees with the server's, value for value`() {
        val cases = listOf(
            // bounds                                              expected km²
            GeoBounds(south = 67.03, west = 15.00, north = 67.13, east = 15.28) to 232.54,
            GeoBounds(south = 60.00, west = 8.00, north = 61.20, east = 11.00) to 23_407.79,
            GeoBounds(south = 58.90, west = 5.20, north = 59.10, east = 5.60) to 762.00,
        )
        for ((bounds, expected) in cases) {
            assertEquals(
                "RoutingPack.areaSqKm drifted from PackKey::area_sq_km for $bounds — " +
                    "update both, in the same commit",
                expected,
                RoutingPack.areaSqKm(bounds),
                eps,
            )
        }
    }

    @Test
    fun `the cap matches the server's`() {
        // Not a tautology: this is the number that decides whether the
        // client asks. If the server lowers its cap and this is not
        // lowered with it, every oversized region costs a round trip and
        // comes back 400 — which the downloader survives, but only
        // because it was written to.
        assertEquals(5_500.0, RoutingPack.MAX_AREA_SQ_KM, 0.0)
    }

    @Test
    fun `the cap is judged on what the pack covers, not what was asked for`() {
        // `keyFor` snaps OUTWARD to the grid, so the server always builds
        // something larger than the viewport. Judging the request rather
        // than the coverage would let a region just under the cap snap to
        // one just over it and be refused after the client cleared it —
        // the exact failure the local check exists to prevent.
        val bounds = GeoBounds(south = 67.03, west = 15.00, north = 67.13, east = 15.28)
        val covered = RoutingPack.extentOf(RoutingPack.keyFor(bounds))!!
        assertTrue(covered.north >= bounds.north && covered.south <= bounds.south)
        assertTrue(covered.east >= bounds.east && covered.west <= bounds.west)

        val raw = (bounds.north - bounds.south) * 111.32 *
            (bounds.east - bounds.west) * 111.32 * Math.cos(Math.toRadians(67.08))
        assertTrue(
            "the covered area (${RoutingPack.areaSqKm(bounds)}) must exceed the requested one ($raw)",
            RoutingPack.areaSqKm(bounds) > raw,
        )
    }

    @Test
    fun `a Norwegian valley fits and a county does not`() {
        // The two ends of the band the cap divides, at real coordinates.
        // Sjunkhatten — what someone actually downloads before a hike.
        assertTrue(
            RoutingPack.fitsOnePack(
                GeoBounds(south = 67.03, west = 15.00, north = 67.13, east = 15.28),
            ),
        )
        // Most of Buskerud. Downloadable as a map, not as one pack.
        assertFalse(
            RoutingPack.fitsOnePack(
                GeoBounds(south = 60.00, west = 8.00, north = 61.20, east = 11.00),
            ),
        )
    }

    @Test
    fun `the cap admits the same area in the north as in the south`() {
        // The bug the cap's units used to hide. It was expressed in grid
        // cells, and a z12 cell is square on the ground but shrinks with
        // latitude — 5.2 km a side at 58degN, 3.8 km at 67degN — so the
        // same cell count was 1.9x more ground in the south, which is
        // where most of Norway is.
        //
        // Whatever the rule is written in, it must admit the same amount
        // of GROUND everywhere, because ground is what the build and the
        // download cost.
        val admitted = listOf(58.0, 62.0, 67.0, 71.0).map { lat ->
            var span = 0.02
            var last = 0.0
            while (span < 8.0) {
                val b = GeoBounds(
                    south = lat,
                    west = 15.0,
                    north = lat + span / 2.0,
                    east = 15.0 + span,
                )
                if (!RoutingPack.fitsOnePack(b)) break
                last = RoutingPack.areaSqKm(b)
                span += 0.02
            }
            last
        }
        val hi = admitted.max()
        val lo = admitted.min()
        assertTrue(
            "the cap admits $hi km² at one latitude and $lo km² at another — " +
                "it is bounding cells again, not work. All: $admitted",
            hi / lo < 1.35,
        )
    }
}
