package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The 3D orbit/zoom gatekeeper decides — and LOCKS — whether a two-finger gesture
 * ORBITS the camera (a centroid drag: bearing + pitch) or ZOOMS it (a spread
 * pinch), so the two are SEPARATED: one gesture pivots or zooms, never both at
 * once. This is the fix for "in 3D I can still zoom, and not just rotate." Pure
 * unit driven with synthetic cumulative deltas — no Compose, no touch.
 */
class OrbitZoomGatekeeperTest {

    private val dragGate = 24f

    /** A centroid drag that leads, with no meaningful pinch, orbits. */
    @Test
    fun `drag-first engages orbit`() {
        val gate = OrbitZoomGatekeeper(dragGatePx = dragGate)
        assertEquals(OrbitZoomVerdict.Undecided, gate.update(dragPxFromStart = 8f, pinchRatioFromStart = 1.0f))
        assertEquals(OrbitZoomVerdict.Undecided, gate.update(dragPxFromStart = 18f, pinchRatioFromStart = 1.0f))
        assertEquals(OrbitZoomVerdict.Orbit, gate.update(dragPxFromStart = 30f, pinchRatioFromStart = 1.0f))
    }

    /** A pinch that leads zooms, and the verdict then holds for the whole gesture
     *  even if the fingers later drift well past the drag gate. */
    @Test
    fun `pinch-first engages zoom and stays locked`() {
        val gate = OrbitZoomGatekeeper(dragGatePx = dragGate)
        assertEquals(OrbitZoomVerdict.Zoom, gate.update(dragPxFromStart = 3f, pinchRatioFromStart = 1.06f))
        // A later big drag must NOT flip it — the sequence is locked to zoom.
        assertEquals(OrbitZoomVerdict.Zoom, gate.update(dragPxFromStart = 80f, pinchRatioFromStart = 1.10f))
    }

    /**
     * A real orbit drag never keeps the spread perfectly constant — the pinch
     * ratio wobbles a percent or two. As long as the drag dominates, it still
     * orbits (it does not leak a zoom), which is the separation the user asked for.
     */
    @Test
    fun `a dominant drag orbits even with incidental pinch wobble`() {
        val gate = OrbitZoomGatekeeper(dragGatePx = dragGate)
        // drag 30 px (normalised 1.25) leads a 3% spread wobble (normalised 0.75).
        assertEquals(OrbitZoomVerdict.Orbit, gate.update(dragPxFromStart = 30f, pinchRatioFromStart = 1.03f))
    }

    /**
     * A real pinch also drifts the centroid a little. As long as the pinch
     * dominates it still zooms and does not leak an orbit (bearing/pitch change).
     */
    @Test
    fun `a dominant pinch zooms even with incidental centroid drift`() {
        val gate = OrbitZoomGatekeeper(dragGatePx = dragGate)
        // 8% spread change (normalised 2.0) leads a 20 px drift (normalised 0.83).
        assertEquals(OrbitZoomVerdict.Zoom, gate.update(dragPxFromStart = 20f, pinchRatioFromStart = 1.08f))
    }

    /** Once orbit is engaged it stays engaged through the later pinch of a combined
     *  movement — the lock prevents accidental zoom entry, not the drag that follows. */
    @Test
    fun `engaged orbit stays locked through later pinch`() {
        val gate = OrbitZoomGatekeeper(dragGatePx = dragGate)
        assertEquals(OrbitZoomVerdict.Orbit, gate.update(dragPxFromStart = 30f, pinchRatioFromStart = 1.0f))
        assertEquals(OrbitZoomVerdict.Orbit, gate.update(dragPxFromStart = 60f, pinchRatioFromStart = 1.3f))
    }

    /** Below every gate the verdict stays undecided (the gesture hasn't committed). */
    @Test
    fun `below both gates stays undecided`() {
        val gate = OrbitZoomGatekeeper(dragGatePx = dragGate)
        assertEquals(OrbitZoomVerdict.Undecided, gate.update(dragPxFromStart = 10f, pinchRatioFromStart = 1.02f))
    }
}
