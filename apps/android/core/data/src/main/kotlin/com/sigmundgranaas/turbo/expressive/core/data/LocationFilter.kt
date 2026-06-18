package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.domain.LatLng

/**
 * Gates raw location fixes before any consumer (map dot, recording, following)
 * sees them, so a stale or wildly-inaccurate reading can't teleport the user.
 * Stateful (holds the last accepted fix). Mirrors the iOS `LocationFilter`; both
 * are pinned by the shared fixtures under `fixtures/tracking/filter/`.
 */
class LocationFilter(
    private val accuracyMaxM: Double = 50.0,
    private val stalenessMaxMs: Double = 5000.0,
    /** Absolute distance backstop (m) — a step beyond this is a jump regardless of timing. */
    private val jumpMaxM: Double = 200.0,
    /** Implausible on-foot speed (m/s) vs the previous accepted fix → a jump (D5). */
    private val maxSpeedMps: Double = 30.0,
) {
    private var lastAccepted: LatLng? = null
    private var pendingJump: LatLng? = null

    /**
     * Whether to accept this fix (and advance the filter state).
     * @param accuracyM horizontal accuracy in metres.
     * @param ageMs how old the fix is (now − fix timestamp), in milliseconds.
     * @param intervalMs time since the previous fix, in ms (drives the speed gate); the 1 s
     *   default keeps the gate sane when the caller can't supply it (e.g. shared fixtures).
     */
    fun accept(position: LatLng, accuracyM: Double, ageMs: Double, intervalMs: Double = 1000.0): Boolean {
        if (accuracyM > accuracyMaxM) return false
        if (ageMs > stalenessMaxMs) return false

        val last = lastAccepted ?: run {
            lastAccepted = position; pendingJump = null
            return true
        }
        if (!isJump(last, position, intervalMs)) {
            lastAccepted = position; pendingJump = null
            return true
        }
        // A jump — accept only if a second consistent fix confirms it (a real fast move),
        // otherwise it's a one-off glitch.
        val pending = pendingJump
        if (pending != null && !isJump(pending, position, intervalMs)) {
            lastAccepted = position; pendingJump = null
            return true
        }
        pendingJump = position
        return false
    }

    /** A fix is a jump if it's too far (absolute) OR implies an implausible speed (rate). */
    private fun isJump(from: LatLng, to: LatLng, intervalMs: Double): Boolean {
        val d = GeoMetrics.haversineMeters(from, to)
        if (d > jumpMaxM) return true
        val seconds = (intervalMs / 1000.0).coerceAtLeast(0.001)
        return d / seconds > maxSpeedMps
    }
}
