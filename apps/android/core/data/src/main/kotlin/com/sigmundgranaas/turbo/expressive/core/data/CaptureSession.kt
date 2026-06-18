package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.domain.LatLng

/** Buffered walking past this (m) prompts Include/Discard on resume; below it, just resume (D4). */
const val RESUME_PROMPT_M = 80.0

/**
 * The travelled-track capture state machine shared by recording AND following
 * (Follow = Record). It wraps the running [CapturedTrack] with the pause-buffer
 * behaviour (US-4): while paused, fixes accumulate into a side [buffer] anchored to
 * the last track point instead of the track itself, so forgetting to unpause never
 * silently loses the walk; on resume the caller chooses Include (stitch the buffer
 * in, back-dated) or Discard (drop it and lift the pen so the gap isn't counted).
 *
 * Pure + immutable: both [RecordingController] and [FollowController] hold one and
 * apply transitions, which is what makes pause "just work" identically in either
 * mode. Pinned by `CaptureSessionTest`.
 */
data class CaptureSession(
    /** The committed track — the line that has actually been walked and counted. */
    val track: CapturedTrack = CapturedTrack(),
    val paused: Boolean = false,
    /** Movement captured while paused, not yet folded into [track]. */
    val buffer: CapturedTrack = CapturedTrack(),
    /** The track point at the moment of pausing — buffered distance is measured from here. */
    val pauseAnchor: LatLng? = null,
    /** After a Discard resume the next live fix starts a fresh segment (no gap distance). */
    val penUpNext: Boolean = false,
) {
    val points: List<LatLng> get() = track.points
    val elevations: List<Double?> get() = track.elevations
    val distanceM: Double get() = track.distanceM
    val speedMps: Double? get() = track.speedMps
    val maxSpeedMps: Double get() = track.maxSpeedMps
    val userLocation: LatLng? get() = track.points.lastOrNull()

    /**
     * Distance (m) walked while paused, measured from the [pauseAnchor] so the very first
     * paused fix already counts (otherwise a single-point buffer would read 0). 0 when idle.
     */
    val bufferedDistanceM: Double
        get() {
            val first = buffer.points.firstOrNull() ?: return 0.0
            val join = pauseAnchor?.let { GeoMetrics.haversineMeters(it, first) } ?: 0.0
            return join + buffer.distanceM
        }

    /** Whether enough was walked while paused to be worth asking about on resume. */
    val hasBufferedMovement: Boolean get() = bufferedDistanceM >= RESUME_PROMPT_M

    /**
     * Fold one (already accuracy/jump-filtered) fix in: into the side [buffer] while paused,
     * else into the [track] (detaching the first post-Discard fix so the gap isn't counted).
     */
    fun fix(position: LatLng, altitude: Double?, speedMps: Double?): CaptureSession {
        if (paused) return copy(buffer = TrackCapture.append(buffer, position, altitude, speedMps))
        val next = if (penUpNext) {
            TrackCapture.appendDetached(track, position, altitude, speedMps)
        } else {
            TrackCapture.append(track, position, altitude, speedMps)
        }
        return copy(track = next, penUpNext = false)
    }

    /** Begin a pause: capture continues into a fresh buffer anchored at the last track point. */
    fun pause(): CaptureSession =
        if (paused) this else copy(paused = true, buffer = CapturedTrack(), pauseAnchor = track.points.lastOrNull())

    /**
     * Resume from a pause. [include] = true stitches the buffered walk onto the track (it IS
     * the path you walked, back-dated); false drops it and lifts the pen so the gap back to the
     * pre-pause point isn't counted. Either way the buffer is cleared.
     */
    fun resume(include: Boolean): CaptureSession {
        if (!paused) return this
        if (include && buffer.points.isNotEmpty()) {
            val join = track.points.lastOrNull()?.let { GeoMetrics.haversineMeters(it, buffer.points.first()) } ?: 0.0
            val merged = track.copy(
                points = track.points + buffer.points,
                elevations = track.elevations + buffer.elevations,
                distanceM = track.distanceM + join + buffer.distanceM,
                maxSpeedMps = maxOf(track.maxSpeedMps, buffer.maxSpeedMps),
            )
            return copy(track = merged, paused = false, buffer = CapturedTrack(), pauseAnchor = null, penUpNext = false)
        }
        return copy(paused = false, buffer = CapturedTrack(), pauseAnchor = null, penUpNext = true)
    }

    companion object {
        /**
         * Rebuild a session from a flat persisted track (process-death recovery). When
         * [pausedFromIndex] is a valid split point the tail past it is the held buffer and the
         * session restores **paused**; otherwise the whole thing is the committed track.
         */
        fun restore(
            points: List<LatLng>,
            elevations: List<Double?>,
            pausedFromIndex: Int,
        ): CaptureSession {
            if (pausedFromIndex !in 0..points.size) {
                // Not a paused draft (sentinel -1, or a stale index) → the whole thing is committed.
                return CaptureSession(track = CapturedTrack(points, elevations, GeoMetrics.pathLengthMeters(points)))
            }
            val mainPts = points.take(pausedFromIndex)
            val mainElev = elevations.take(pausedFromIndex)
            val bufPts = points.drop(pausedFromIndex)
            val bufElev = elevations.drop(pausedFromIndex)
            return CaptureSession(
                track = CapturedTrack(mainPts, mainElev, GeoMetrics.pathLengthMeters(mainPts)),
                paused = true,
                buffer = CapturedTrack(bufPts, bufElev, GeoMetrics.pathLengthMeters(bufPts)),
                pauseAnchor = mainPts.lastOrNull(),
            )
        }
    }
}
