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
     */
    fun update(twistDegFromStart: Float, pinchRatioFromStart: Float, panPxFromStart: Float): TwoFingerVerdict {
        if (verdict != TwoFingerVerdict.Undecided) return verdict
        if (rotationLocked) {
            // Locked: only pan/zoom can ever engage; twist is inert.
            if (abs(pinchRatioFromStart - 1f) >= pinchThreshold || panPxFromStart >= panSlopPx) {
                verdict = TwoFingerVerdict.PanZoom
            }
            return verdict
        }
        val twistCrossed = abs(twistDegFromStart) >= rotationGateDeg
        val pinchCrossed = abs(pinchRatioFromStart - 1f) >= pinchThreshold
        val panCrossed = panPxFromStart >= panSlopPx
        verdict = when {
            // Rotation only if twist LEADS — clean of pinch and pan (a same-frame
            // tie falls through to PanZoom, the conservative choice).
            twistCrossed && !pinchCrossed && !panCrossed -> TwoFingerVerdict.Rotate
            pinchCrossed || panCrossed -> TwoFingerVerdict.PanZoom
            else -> TwoFingerVerdict.Undecided
        }
        return verdict
    }

    companion object {
        const val DEFAULT_PINCH_THRESHOLD = 0.04f
        const val DEFAULT_PAN_SLOP_PX = 24f // ~8dp at mdpi; the detector passes a density-scaled value
    }
}
