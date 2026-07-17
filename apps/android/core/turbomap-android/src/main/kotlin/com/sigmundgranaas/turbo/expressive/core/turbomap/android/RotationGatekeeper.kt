package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import kotlin.math.abs

/** The verdict for a two-finger gesture: whether it rotates or pans-and-zooms. */
internal enum class TwoFingerVerdict { Undecided, Rotate, PanZoom }

/**
 * Decides — once per two-finger gesture — whether the gesture rotates the map or
 * pans-and-zooms it, and LOCKS that verdict for the rest of the gesture (the
 * "sequence lock" from the spec). Rotation engages only when finger-twist is the
 * *primary* movement: the accumulated twist must cross [rotationGateDeg] while
 * neither pinch nor pan has crossed its own threshold. If pinch or pan leads (or
 * ties in the same frame), the gesture is pan-and-zoom and rotation is suppressed
 * for its whole duration — so a natural pinch never wobbles the bearing.
 *
 * Pure — fed accumulated deltas measured from gesture start, no Compose, no
 * touch. This is the seam the 2D-rotate gesture and the compass "lock rotation"
 * setting both drive. See docs/architecture/2026-07-turbo-map-overhaul-spec.md.
 */
internal class RotationGatekeeper(
    private val rotationGateDeg: Float = GestureConfig.DEFAULT_ROTATION_GATE_DEG,
    /** Pinch magnitude (|ratio − 1|) that counts as "zoom led". ~4%. */
    private val pinchThreshold: Float = DEFAULT_PINCH_THRESHOLD,
    /** Centroid travel (px) that counts as "pan led" — the platform touch slop. */
    private val panSlopPx: Float = DEFAULT_PAN_SLOP_PX,
    /** When true (compass lock / rotation disabled), rotation never engages —
     *  every two-finger gesture is pan-and-zoom. */
    private val rotationLocked: Boolean = false,
) {
    private var verdict = TwoFingerVerdict.Undecided

    /** The locked-in verdict (or [TwoFingerVerdict.Undecided] before either
     *  threshold is crossed). */
    val current: TwoFingerVerdict get() = verdict

    /**
     * Fold in the gesture's cumulative state since it started:
     * [twistDegFromStart] net finger-pair rotation (signed degrees),
     * [pinchRatioFromStart] current spread ÷ start spread (1 = no pinch),
     * [panPxFromStart] centroid displacement from the start point (px).
     * Returns the (possibly newly-locked) verdict.
     *
     * The decision is DOMINANCE, not first-past-the-post: each candidate motion is
     * normalised against its own engage-gate (twist/[rotationGateDeg],
     * pinch/[pinchThreshold], pan/[panSlopPx]) and rotation wins only when the
     * TWIST is the largest of the three once any of them reaches its gate. That
     * matters because a real two-finger twist is never perfectly pivoted — the
     * centroid always drifts a little (an asymmetric pivot) and the spread wobbles
     * a percent or two. The old "twist crosses first, clean of ANY pan/pinch" rule
     * meant that incidental centroid drift crossed the pan-slop before the twist
     * reached its gate, so a deliberate rotation resolved to PanZoom and rotation
     * was effectively unreachable. Dominance lets the twist win as long as it is
     * the dominant motion, while a pan/pinch that only incidentally twists still
     * resolves to PanZoom.
     */
    fun update(twistDegFromStart: Float, pinchRatioFromStart: Float, panPxFromStart: Float): TwoFingerVerdict {
        if (verdict != TwoFingerVerdict.Undecided) return verdict
        // Locked (compass "Lock rotation" / rotation disabled): the twist can never
        // lead, so zero its progress — only pan/pinch can engage, always PanZoom.
        val twist = if (rotationLocked) 0f else abs(twistDegFromStart) / rotationGateDeg
        val pinch = abs(pinchRatioFromStart - 1f) / pinchThreshold
        val pan = panPxFromStart / panSlopPx
        val lead = maxOf(twist, pinch, pan)
        // No candidate has reached its gate yet — keep waiting.
        if (lead < 1f) return TwoFingerVerdict.Undecided
        // A gate was reached: rotate iff the twist is the dominant motion.
        verdict = if (twist >= 1f && twist >= pinch && twist >= pan) {
            TwoFingerVerdict.Rotate
        } else {
            TwoFingerVerdict.PanZoom
        }
        return verdict
    }

    companion object {
        const val DEFAULT_PINCH_THRESHOLD = 0.04f
        const val DEFAULT_PAN_SLOP_PX = 24f // ~8dp at mdpi; the detector passes a density-scaled value
    }
}

/** The verdict for a 3D two-finger gesture: whether it orbits (pivots) or zooms. */
internal enum class OrbitZoomVerdict { Undecided, Orbit, Zoom }

/**
 * The 3D counterpart to [RotationGatekeeper]: decides — once per two-finger
 * gesture — whether the gesture ORBITS the camera (a centroid drag: horizontal →
 * bearing, vertical → pitch) or ZOOMS it (a spread pinch), and LOCKS that verdict
 * for the rest of the gesture. In 3D the two are SEPARATED — one two-finger
 * gesture either pivots or zooms, never both at once (the previous behaviour, a
 * drag that also zoomed on every frame, is the "I can still zoom, not just
 * rotate" complaint). Same dominance rule as [RotationGatekeeper]: whichever
 * normalised motion leads once a gate is reached wins; a tie favours Orbit
 * because dragging to pivot is the primary 3D interaction and zoom should only
 * take over when the pinch clearly dominates.
 *
 * Pure — fed accumulated deltas from gesture start; no Compose, no touch.
 */
internal class OrbitZoomGatekeeper(
    /** Centroid travel (px) that counts as an orbit drag — the platform touch slop. */
    private val dragGatePx: Float = RotationGatekeeper.DEFAULT_PAN_SLOP_PX,
    /** Pinch magnitude (|ratio − 1|) that counts as a zoom. ~4%. */
    private val pinchThreshold: Float = RotationGatekeeper.DEFAULT_PINCH_THRESHOLD,
) {
    private var verdict = OrbitZoomVerdict.Undecided
    val current: OrbitZoomVerdict get() = verdict

    /**
     * Fold in the gesture's cumulative state since it started:
     * [dragPxFromStart] centroid displacement from the start point (px),
     * [pinchRatioFromStart] current spread ÷ start spread (1 = no pinch).
     */
    fun update(dragPxFromStart: Float, pinchRatioFromStart: Float): OrbitZoomVerdict {
        if (verdict != OrbitZoomVerdict.Undecided) return verdict
        val drag = dragPxFromStart / dragGatePx
        val pinch = abs(pinchRatioFromStart - 1f) / pinchThreshold
        val lead = maxOf(drag, pinch)
        if (lead < 1f) return OrbitZoomVerdict.Undecided
        verdict = if (drag >= pinch) OrbitZoomVerdict.Orbit else OrbitZoomVerdict.Zoom
        return verdict
    }
}
