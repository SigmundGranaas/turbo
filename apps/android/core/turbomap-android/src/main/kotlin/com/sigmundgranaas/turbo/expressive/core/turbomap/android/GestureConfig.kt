package com.sigmundgranaas.turbo.expressive.core.turbomap.android

/**
 * The tunable feel of map gestures — one place so the values are (a) drivable in
 * tests and (b) exposed in Settings → Gestures. Defaults match the shipped
 * hand-tuned constants, so sourcing them from here is behaviour-preserving until
 * a user changes them.
 *
 * See docs/architecture/2026-07-turbo-map-overhaul-spec.md (Phase 0).
 */
data class GestureConfig(
    /** Long-press fire delay (ms). Platform default is 500. */
    val longPressMs: Long = DEFAULT_LONG_PRESS_MS,
    /** How far a finger may wander (dp) and still count as a stationary touch —
     *  the tap-ignore / long-press-cancel radius. Larger than platform slop
     *  because the complaint here is jitter (trains, cold hands). */
    val movementGuardDp: Float = DEFAULT_MOVEMENT_GUARD_DP,
    /** Finger-twist (degrees) that must accrue *before* pinch/pan to engage
     *  rotation. Higher = harder to rotate by accident. */
    val rotationGateDeg: Float = DEFAULT_ROTATION_GATE_DEG,
    /** Fling velocity half-life (ms) — higher is floatier. The settle itself is
     *  integrated engine-side; this is the coefficient the host passes down. */
    val flingHalfLifeMs: Long = DEFAULT_FLING_HALF_LIFE_MS,
) {
    companion object {
        const val DEFAULT_LONG_PRESS_MS = 500L
        const val DEFAULT_MOVEMENT_GUARD_DP = 18f
        const val DEFAULT_ROTATION_GATE_DEG = 10f
        const val DEFAULT_FLING_HALF_LIFE_MS = 300L
    }
}
