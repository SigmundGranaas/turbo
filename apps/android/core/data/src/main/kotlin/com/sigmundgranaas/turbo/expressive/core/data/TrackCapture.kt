package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.domain.LatLng

/**
 * The travelled track accumulated from GPS fixes — the shared capture core of BOTH
 * recording and following (Follow = Record). Holds the polyline, per-point altitude,
 * cumulative distance and speed. Pure: the *same* fixes fold into the *same* track
 * whether they arrive via the record screen or while following a route, which is
 * exactly the parity US-1 promises. Pinned by [TrackCaptureTest].
 */
data class CapturedTrack(
    val points: List<LatLng> = emptyList(),
    /** Altitude (m) per point; parallel to [points]. Null = the fix carried no altitude. */
    val elevations: List<Double?> = emptyList(),
    val distanceM: Double = 0.0,
    /** Latest instantaneous ground speed (m/s); null until a fix carries speed. */
    val speedMps: Double? = null,
    /** Fastest instantaneous speed seen so far (m/s). */
    val maxSpeedMps: Double = 0.0,
) {
    val userLocation: LatLng? get() = points.lastOrNull()
}

/** Folds GPS fixes into a [CapturedTrack]. Stateless — caller holds the running track. */
object TrackCapture {
    /** Minimum move (m) before a fix becomes a new track point — rejects GPS jitter. */
    const val MIN_STEP_M = 3.0

    /**
     * Fold one (already accuracy/jump-filtered) fix into [track]. Speed always
     * refreshes — even when the point is too close to add — so live tiles stay
     * responsive; points/distance grow only once the step clears [MIN_STEP_M].
     */
    fun append(track: CapturedTrack, position: LatLng, altitude: Double?, speedMps: Double?): CapturedTrack {
        val maxSpeed = if (speedMps != null) maxOf(track.maxSpeedMps, speedMps) else track.maxSpeedMps
        if (track.points.isEmpty()) {
            return track.copy(
                points = listOf(position), elevations = listOf(altitude),
                speedMps = speedMps, maxSpeedMps = maxSpeed,
            )
        }
        val step = GeoMetrics.haversineMeters(track.points.last(), position)
        if (step < MIN_STEP_M) return track.copy(speedMps = speedMps, maxSpeedMps = maxSpeed)
        return track.copy(
            points = track.points + position,
            elevations = track.elevations + altitude,
            distanceM = track.distanceM + step,
            speedMps = speedMps, maxSpeedMps = maxSpeed,
        )
    }
}
