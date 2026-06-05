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
enum class RoutePreset(val key: String, val label: String) {
    Balanced("balanced", "Balanced"),
    AvoidRoads("avoid_roads", "Avoid roads"),
    Direct("direct", "Direct"),
    EasyGrade("easy_grade", "Easy grade"),
    TrailPurist("trail_purist", "Trail purist"),
}

/** Streamed solver events: live best-path snapshots, then a terminal result/failure. */
sealed interface RouteStreamEvent {
    /** A best-path-so-far snapshot; the latest replaces the previous preview. */
    data class Progress(val coordinates: List<LatLng>) : RouteStreamEvent
    data class Result(val plan: RoutePlan) : RouteStreamEvent
    data class Failure(val message: String) : RouteStreamEvent
}
