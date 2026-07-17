package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The rotation gatekeeper decides — and LOCKS — whether a two-finger gesture
 * rotates or pans-and-zooms, so a natural pinch never wobbles the bearing and a
 * deliberate twist does rotate. These drive the pure unit with synthetic
 * cumulative deltas (no Compose, no touch), which is the whole point of the
 * Phase-0 extraction: the gesture *decision* is testable without a device.
 */
class RotationGatekeeperTest {

    private val panSlop = 24f

    /** Twist that clearly leads, with no meaningful pinch/pan, rotates the map. */
    @Test
    fun `twist-first engages rotation`() {
        val gate = RotationGatekeeper(rotationGateDeg = 10f, panSlopPx = panSlop)
        // A few clean-twist frames building past the 10° gate.
        assertEquals(TwoFingerVerdict.Undecided, gate.update(twistDegFromStart = 4f, pinchRatioFromStart = 1.0f, panPxFromStart = 2f))
        assertEquals(TwoFingerVerdict.Undecided, gate.update(twistDegFromStart = 8f, pinchRatioFromStart = 1.0f, panPxFromStart = 3f))
        assertEquals(TwoFingerVerdict.Rotate, gate.update(twistDegFromStart = 12f, pinchRatioFromStart = 1.0f, panPxFromStart = 4f))
    }

    /** A pinch that crosses first suppresses rotation for the whole gesture — even
     *  if the fingers later twist well past the gate. */
    @Test
    fun `pinch-first suppresses rotation for the gesture`() {
        val gate = RotationGatekeeper(rotationGateDeg = 10f, panSlopPx = panSlop)
        assertEquals(TwoFingerVerdict.PanZoom, gate.update(twistDegFromStart = 1f, pinchRatioFromStart = 1.06f, panPxFromStart = 2f))
        // Later big twist must NOT flip it — the sequence is locked.
        assertEquals(TwoFingerVerdict.PanZoom, gate.update(twistDegFromStart = 40f, pinchRatioFromStart = 1.10f, panPxFromStart = 5f))
    }

    /** A centroid pan that clearly dominates is pan-and-zoom, not rotation. */
    @Test
    fun `pan-first suppresses rotation`() {
        val gate = RotationGatekeeper(rotationGateDeg = 10f, panSlopPx = panSlop)
        assertEquals(TwoFingerVerdict.PanZoom, gate.update(twistDegFromStart = 2f, pinchRatioFromStart = 1.0f, panPxFromStart = 30f))
    }

    /**
     * The regression that made 2D rotation unreachable: a real two-finger twist is
     * never a perfect pivot — the centroid always drifts a little, often PAST the
     * pan-slop, before the twist reaches its gate. The old "clean of any pan" rule
     * then locked PanZoom and rotation never fired. Dominance fixes it: when the
     * twist is the dominant motion it rotates even though the incidental centroid
     * drift has crossed the pan-slop.
     */
    @Test
    fun `a dominant twist rotates even when incidental centroid drift crossed the pan-slop`() {
        val gate = RotationGatekeeper(rotationGateDeg = 10f, panSlopPx = panSlop)
        // twist 11° (normalised 1.1) leads the 26 px drift (normalised 1.08), no pinch.
        assertEquals(
            TwoFingerVerdict.Rotate,
            gate.update(twistDegFromStart = 11f, pinchRatioFromStart = 1.0f, panPxFromStart = 26f),
        )
    }

    /** When twist and pinch cross in the SAME frame, we can't tell which led —
     *  the conservative choice is pan-and-zoom (don't rotate unless twist clearly
     *  leads). */
    @Test
    fun `a same-frame twist-and-pinch tie falls back to pan-zoom`() {
        val gate = RotationGatekeeper(rotationGateDeg = 10f, panSlopPx = panSlop)
        assertEquals(TwoFingerVerdict.PanZoom, gate.update(twistDegFromStart = 12f, pinchRatioFromStart = 1.05f, panPxFromStart = 1f))
    }

    /** Once rotation is engaged it stays engaged even as the natural pinch/pan of
     *  a combined movement grows — the lock prevents accidental ENTRY, not the
     *  combined rotate+pan+zoom that follows. */
    @Test
    fun `engaged rotation stays locked through later pinch and pan`() {
        val gate = RotationGatekeeper(rotationGateDeg = 10f, panSlopPx = panSlop)
        assertEquals(TwoFingerVerdict.Rotate, gate.update(twistDegFromStart = 12f, pinchRatioFromStart = 1.0f, panPxFromStart = 2f))
        assertEquals(TwoFingerVerdict.Rotate, gate.update(twistDegFromStart = 20f, pinchRatioFromStart = 1.3f, panPxFromStart = 80f))
    }

    /** Rotation direction doesn't matter — a counter-clockwise twist engages too. */
    @Test
    fun `negative twist engages rotation`() {
        val gate = RotationGatekeeper(rotationGateDeg = 10f, panSlopPx = panSlop)
        assertEquals(TwoFingerVerdict.Rotate, gate.update(twistDegFromStart = -11f, pinchRatioFromStart = 1.0f, panPxFromStart = 3f))
    }

    /** The configured gate actually changes behaviour: a 20° gate ignores a 15°
     *  twist that a 10° gate would have rotated on — this is what the Settings →
     *  Gestures "rotation strictness" control drives. */
    @Test
    fun `a stricter configured gate rejects a twist the default would accept`() {
        val strict = RotationGatekeeper(rotationGateDeg = 20f, panSlopPx = panSlop)
        assertEquals(TwoFingerVerdict.Undecided, strict.update(twistDegFromStart = 15f, pinchRatioFromStart = 1.0f, panPxFromStart = 2f))

        val default = RotationGatekeeper(rotationGateDeg = 10f, panSlopPx = panSlop)
        assertEquals(TwoFingerVerdict.Rotate, default.update(twistDegFromStart = 15f, pinchRatioFromStart = 1.0f, panPxFromStart = 2f))
    }

    /** With rotation locked (compass "Lock rotation" / disabled), a clean twist
     *  never rotates — the gesture only ever resolves to pan-and-zoom. */
    @Test
    fun `locked rotation never engages even on a clean twist`() {
        val gate = RotationGatekeeper(rotationGateDeg = 10f, panSlopPx = panSlop, rotationLocked = true)
        assertEquals(TwoFingerVerdict.Undecided, gate.update(twistDegFromStart = 30f, pinchRatioFromStart = 1.0f, panPxFromStart = 1f))
        // A pinch still resolves it to pan-zoom.
        assertEquals(TwoFingerVerdict.PanZoom, gate.update(twistDegFromStart = 30f, pinchRatioFromStart = 1.06f, panPxFromStart = 1f))
    }
}
