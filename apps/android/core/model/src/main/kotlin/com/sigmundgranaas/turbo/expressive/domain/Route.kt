package com.sigmundgranaas.turbo.expressive.domain

/** A solved route from the Turbo pathfinder. Geometry is ordered WGS84 points. */
data class RoutePlan(
    val distanceM: Double,
    val durationS: Double,
    val ascentM: Double,
    val onTrailPct: Double,
    val surfaces: Map<String, Double>,
    val geometry: List<LatLng>,
)

/** Trip-style presets the pathfinder accepts (`preset` field). */
enum class RoutePreset(val key: String, val label: String, val description: String) {
    Balanced("balanced", "Balanced", "A sensible mix of trails, terrain and directness."),
    AvoidRoads("avoid_roads", "Avoid roads", "Stay off roads where a trail or terrain route exists."),
    Direct("direct", "Direct", "The shortest line, ignoring comfort and surface."),
    EasyGrade("easy_grade", "Easy grade", "Favours gentle slopes and switchbacks over steep climbs."),
    TrailPurist("trail_purist", "Trail purist", "Sticks to marked trails as much as possible."),
}

/** Streamed solver events: live best-path snapshots, then a terminal result/failure. */
sealed interface RouteStreamEvent {
    /** A best-path-so-far snapshot; the latest replaces the previous preview. */
    data class Progress(val coordinates: List<LatLng>) : RouteStreamEvent
    data class Result(val plan: RoutePlan) : RouteStreamEvent
    data class Failure(val message: String) : RouteStreamEvent
}
