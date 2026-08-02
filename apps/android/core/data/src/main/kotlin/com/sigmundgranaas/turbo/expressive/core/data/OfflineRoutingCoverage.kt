package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.LatLng

/**
 * Can these points be routed between without the network?
 *
 * A separate, tiny interface rather than a method on [RouteRepository],
 * because it answers a different question at a different time. The
 * repository is asked to plan a route; this is asked *before* one is
 * requested, so the UI can offer a download instead of showing a
 * failure — and it is cheap enough (one terrain lookup per point) to ask
 * on every waypoint edit.
 *
 * Lives here so `:feature:map-route` can depend on the question without
 * depending on the native module that answers it.
 */
fun interface OfflineRoutingCoverage {
    fun covers(points: List<LatLng>): Boolean

    companion object {
        /** Covers nothing — the honest answer when no packs are available. */
        val NONE: OfflineRoutingCoverage = OfflineRoutingCoverage { false }
    }
}
