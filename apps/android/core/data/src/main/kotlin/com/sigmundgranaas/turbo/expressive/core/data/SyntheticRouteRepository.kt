package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import com.sigmundgranaas.turbo.expressive.domain.RouteStreamEvent
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import javax.inject.Inject

/**
 * An offline stand-in for the pathfinder: instead of calling the trail-bound SSE
 * router (northern Norway only, unreachable from the emulator / a dev box), it
 * "solves" by densifying straight segments between the waypoints. The shape is
 * obviously not trail-snapped, but it produces a real [RoutePlan] — geometry,
 * distance, ETA — so the entire Route-builder and Follow UX (place / append /
 * reorder / drag / follow) can be driven end-to-end anywhere. Selected in DEBUG
 * builds via [com.sigmundgranaas.turbo.expressive.core.data.di.NetworkModule].
 */
class SyntheticRouteRepository @Inject constructor() : RouteRepository {

    override fun planStream(
        points: List<LatLng>,
        preset: RoutePreset,
        profile: String,
    ): Flow<RouteStreamEvent> = flow {
        if (points.size < 2) {
            emit(RouteStreamEvent.Failure("Need at least two points"))
            return@flow
        }
        // Stream a straight-line "best so far" first (mirrors the real solver's progress),
        // then the densified result a beat later so the solving animation is exercised.
        emit(RouteStreamEvent.Progress(points))
        delay(SOLVE_DELAY_MS)
        val geometry = densify(points)
        val distanceM = GeoMetrics.pathLengthMeters(geometry)
        emit(
            RouteStreamEvent.Result(
                RoutePlan(
                    distanceM = distanceM,
                    durationS = GeoMetrics.etaSeconds(distanceM, ascentM = 0.0).toDouble(),
                    ascentM = 0.0, // no DEM offline — honestly flat
                    onTrailPct = 0.0,
                    surfaces = emptyMap(),
                    geometry = geometry,
                ),
            ),
        )
    }

    /** Interpolate ~[STEP_M]-spaced points along each leg so the line reads as a path. */
    private fun densify(waypoints: List<LatLng>): List<LatLng> {
        val out = mutableListOf(waypoints.first())
        for (i in 1 until waypoints.size) {
            val a = waypoints[i - 1]
            val b = waypoints[i]
            val legM = GeoMetrics.haversineMeters(a, b)
            val steps = (legM / STEP_M).toInt().coerceIn(1, MAX_STEPS_PER_LEG)
            for (s in 1..steps) {
                val t = s.toDouble() / steps
                out += LatLng(a.lat + (b.lat - a.lat) * t, a.lng + (b.lng - a.lng) * t)
            }
        }
        return out
    }

    private companion object {
        const val SOLVE_DELAY_MS = 350L
        const val STEP_M = 120.0
        const val MAX_STEPS_PER_LEG = 200
    }
}
