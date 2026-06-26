package com.sigmundgranaas.turbo.expressive.core.tracking

import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.core.geo.PhaseSplit

/** Whether the live surface is capturing a track or following a planned route. */
enum class LiveMode { Recording, Following }

/**
 * The numbers a live surface shows — distilled from a [RecordingSession] or a
 * [FollowSession] into one shape. Both the in-app draggable sheet and the
 * lock-screen Live Update render from this single read-model, so the two
 * surfaces are guaranteed to agree. Raw SI units; each surface formats with
 * [com.sigmundgranaas.turbo.expressive.core.geo.Units] against the metric pref.
 */
data class LiveStats(
    val mode: LiveMode,
    val paused: Boolean = false,
    /** Distance covered (recording) — the hero number for [LiveMode.Recording]. */
    val distanceM: Double = 0.0,
    /** Distance still to walk (following) — the hero number for [LiveMode.Following]. */
    val distanceRemainingM: Double? = null,
    /** Total route length (following), for the progress bar denominator. */
    val routeDistanceM: Double? = null,
    val elapsedSec: Int? = null,
    val speedMps: Double? = null,
    val maxSpeedMps: Double? = null,
    val ascentM: Double? = null,
    val descentM: Double? = null,
    val altitudeM: Double? = null,
    val ascentRemainingM: Double? = null,
    val etaSeconds: Int? = null,
    /** Progress along the route, 0..1 (following only). */
    val fraction: Double? = null,
    val kcal: Int = 0,
    /** Distance (m) walked while paused, pending Include/Discard — drives the resume nudge (US-4). */
    val bufferedDistanceM: Double = 0.0,
    /** Checkpoints crossed so far with split times (following, US-3). */
    val phaseSplits: List<PhaseSplit> = emptyList(),
    /** Next checkpoint name + distance to it (following), or null when none remain. */
    val nextPhaseName: String? = null,
    val nextPhaseDistanceM: Double? = null,
) {
    val recording: Boolean get() = mode == LiveMode.Recording

    /** Show the "still moving while paused?" nudge — paused with a meaningful buffered walk. */
    val showResumeNudge: Boolean get() = paused && bufferedDistanceM >= RESUME_PROMPT_M

    companion object {
        /** Build the live read-model for an active recording session. */
        fun of(session: RecordingSession): LiveStats {
            val (asc, desc) = GeoMetrics.gainLoss(session.elevations)
            return LiveStats(
                mode = LiveMode.Recording,
                paused = session.paused,
                distanceM = session.distanceM,
                elapsedSec = session.elapsedSec,
                speedMps = session.speedMps,
                maxSpeedMps = session.maxSpeedMps,
                ascentM = asc,
                descentM = desc,
                altitudeM = session.elevations.lastOrNull { it != null },
                kcal = GeoMetrics.estimateKcal(session.distanceM, asc),
                bufferedDistanceM = session.bufferedDistanceM,
            )
        }

        /**
         * Build the live read-model for an active follow session. Follow = Record, so the
         * accumulated travelled distance + captured ascent/descent/altitude come from the
         * real track (same as a recording), while the route-relative fields (remaining,
         * ETA, fraction, to-climb) come from the planned route + arc-cursor progress.
         */
        fun of(session: FollowSession): LiveStats {
            val plan = session.plan
            val fraction = session.progress?.fraction
            val ascentRemaining = plan?.ascentM?.let { total -> fraction?.let { total * (1 - it) } ?: total }
            val (asc, desc) = GeoMetrics.gainLoss(session.elevations)
            return LiveStats(
                mode = LiveMode.Following,
                paused = session.paused,
                distanceM = session.capturedDistanceM,
                distanceRemainingM = session.progress?.distanceRemainingM ?: plan?.distanceM,
                routeDistanceM = plan?.distanceM,
                elapsedSec = session.elapsedSec,
                speedMps = session.speedMps,
                maxSpeedMps = session.maxSpeedMps.takeIf { it > 0.0 },
                ascentM = asc,
                descentM = desc,
                altitudeM = session.elevations.lastOrNull { it != null },
                ascentRemainingM = ascentRemaining,
                etaSeconds = session.progress?.etaSeconds,
                fraction = fraction,
                kcal = plan?.let { GeoMetrics.estimateKcal(it.distanceM, it.ascentM) } ?: 0,
                bufferedDistanceM = session.bufferedDistanceM,
                phaseSplits = session.phaseSplits,
                nextPhaseName = session.nextPhaseName,
                nextPhaseDistanceM = session.nextPhaseDistanceM,
            )
        }
    }
}
