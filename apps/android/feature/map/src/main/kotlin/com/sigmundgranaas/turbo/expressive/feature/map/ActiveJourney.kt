package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.feature.map.route.RouteUiState

import com.sigmundgranaas.turbo.expressive.core.tracking.RecordingSession
import com.sigmundgranaas.turbo.expressive.domain.LatLng

/**
 * The one thing the user is doing with a line on the map right now. Planning a
 * route, following one, and recording a track are the same idea — a *journey* —
 * so the map renders and the journey panel switch on this single read-model
 * instead of juggling [RouteUiState] and [RecordingSession] separately.
 */
enum class JourneyMode { Idle, Planning, Following, Recording }

data class ActiveJourney(
    val mode: JourneyMode,
    /** The polyline to draw for the journey (planned route or recorded track). */
    val geometry: List<LatLng> = emptyList(),
    val distanceM: Double = 0.0,
    val ascentM: Double? = null,
    val durationS: Double? = null,
    val elapsedSec: Int? = null,
    val paused: Boolean = false,
) {
    val isActive: Boolean get() = mode != JourneyMode.Idle
}

/**
 * Collapse the route planner and the recording session into one journey. An
 * in-progress recording takes precedence — it's an explicit capture the user
 * started — over any route state.
 */
fun resolveJourney(route: RouteUiState, recording: RecordingSession): ActiveJourney = when {
    recording.active -> ActiveJourney(
        mode = JourneyMode.Recording,
        geometry = recording.points,
        distanceM = recording.distanceM,
        elapsedSec = recording.elapsedSec,
        paused = recording.paused,
    )
    route is RouteUiState.Following -> ActiveJourney(
        mode = JourneyMode.Following,
        geometry = route.plan.geometry,
        distanceM = route.plan.distanceM,
        ascentM = route.plan.ascentM,
        durationS = route.plan.durationS,
    )
    route is RouteUiState.Solving -> ActiveJourney(JourneyMode.Planning, geometry = route.progress)
    route is RouteUiState.Done -> ActiveJourney(
        mode = JourneyMode.Planning,
        geometry = route.plan.geometry,
        distanceM = route.plan.distanceM,
        ascentM = route.plan.ascentM,
        durationS = route.plan.durationS,
    )
    else -> ActiveJourney(JourneyMode.Idle)
}
