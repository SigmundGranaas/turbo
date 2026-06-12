package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The tile-reconcile policy is what fixes "slow / inconsistent / stops after
 * panning": continuous diff of desired-vs-in-flight with stale cancellation,
 * nearest-first priority under a cap, and self-healing retry. These prove the
 * decision logic directly, with no device.
 */
class TileReconcilePlanTest {

    @Test
    fun starts_nearest_first_up_to_the_cap() {
        val desired = listOf("a", "b", "c", "d", "e") // engine order = nearest-first
        val d = planReconcile(desired, inFlight = emptySet(), retryAt = emptyMap(), now = 0, cap = 3)
        assertEquals("nearest three, in order", listOf("a", "b", "c"), d.toStart)
        assertTrue(d.toCancel.isEmpty())
    }

    @Test
    fun cancels_tiles_no_longer_desired() {
        // Panned away: x/y are in flight but no longer desired.
        val d = planReconcile(
            desiredOrdered = listOf("a", "b"),
            inFlight = setOf("x", "y"),
            retryAt = emptyMap(),
            now = 0,
            cap = 4,
        )
        assertEquals(setOf("x", "y"), d.toCancel.toSet())
    }

    @Test
    fun cancelling_stale_frees_slots_in_the_same_pass() {
        // Cap is full of stale tiles; the current viewport must not be starved —
        // cancelling them this pass frees their slots so the new tiles start now.
        val d = planReconcile(
            desiredOrdered = listOf("a", "b"),
            inFlight = setOf("x", "y"),
            retryAt = emptyMap(),
            now = 0,
            cap = 2,
        )
        assertEquals(setOf("x", "y"), d.toCancel.toSet())
        assertEquals("freed slots let the current tiles start immediately", listOf("a", "b"), d.toStart)
    }

    @Test
    fun skips_in_flight_and_does_not_recount_their_slots() {
        val d = planReconcile(
            desiredOrdered = listOf("a", "b", "c"),
            inFlight = setOf("a"),
            retryAt = emptyMap(),
            now = 0,
            cap = 2,
        )
        assertTrue("a already loading", "a" !in d.toStart)
        assertEquals("one free slot → next-nearest only", listOf("b"), d.toStart)
    }

    @Test
    fun respects_backoff_window_then_retries_when_it_elapses() {
        val desired = listOf("a", "b")
        val backed = planReconcile(desired, inFlight = emptySet(), retryAt = mapOf("a" to 1000L), now = 500, cap = 4)
        assertEquals("a is still backing off → skip it, start b", listOf("b"), backed.toStart)

        val healed = planReconcile(desired, inFlight = emptySet(), retryAt = mapOf("a" to 1000L), now = 1500, cap = 4)
        assertEquals("backoff elapsed → a retried (self-healing)", listOf("a", "b"), healed.toStart)
    }

    @Test
    fun nothing_to_do_when_all_desired_are_in_flight() {
        val d = planReconcile(listOf("a", "b"), inFlight = setOf("a", "b"), retryAt = emptyMap(), now = 0, cap = 8)
        assertTrue(d.toStart.isEmpty())
        assertTrue(d.toCancel.isEmpty())
    }
}
