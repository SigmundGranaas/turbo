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
    private val jumpMaxM: Double = 200.0,
) {
    private var lastAccepted: LatLng? = null
    private var pendingJump: LatLng? = null

    /**
     * Whether to accept this fix (and advance the filter state).
     * @param accuracyM horizontal accuracy in metres.
     * @param ageMs how old the fix is (now − fix timestamp), in milliseconds.
     */
    fun accept(position: LatLng, accuracyM: Double, ageMs: Double): Boolean {
        if (accuracyM > accuracyMaxM) return false
        if (ageMs > stalenessMaxMs) return false

        val last = lastAccepted ?: run {
            lastAccepted = position; pendingJump = null
            return true
        }
        if (GeoMetrics.haversineMeters(last, position) <= jumpMaxM) {
            lastAccepted = position; pendingJump = null
            return true
        }
        // A big jump — accept only if a second consistent fix confirms it (a real
        // fast move), otherwise it's a one-off glitch.
        val pending = pendingJump
        if (pending != null && GeoMetrics.haversineMeters(pending, position) <= jumpMaxM) {
            lastAccepted = position; pendingJump = null
            return true
        }
        pendingJump = position
        return false
    }
}
