package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import com.sigmundgranaas.turbo.expressive.domain.RouteStreamEvent
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/**
 * Pure (network-free) encoding/decoding for the Turbo routing API: builds the
 * request body and turns one SSE frame `(event, data)` into a domain
 * [RouteStreamEvent]. Kept separate from the HTTP client so it is unit-testable.
 * Wire coordinates are GeoJSON `[lon, lat]`; domain [LatLng] is `(lat, lng)`.
 */
object RouteSse {
    private val json = Json { ignoreUnknownKeys = true }

    fun encodeRequest(
        points: List<LatLng>,
        preset: RoutePreset,
        profile: String,
        roundTrip: Boolean = false,
    ): String =
        json.encodeToString(
            RouteReqDto(
                points = points.map { listOf(it.lng, it.lat) },
                preset = preset.key,
                profile = profile,
                // Only serialize when set — keeps the default request body unchanged and
                // lets the solver loop origin→vias→far→origin with return-leg self-avoidance.
                roundTrip = roundTrip.takeIf { it },
            ),
        )

    /** Parse one SSE frame; returns null for keep-alives/unknown events. */
    fun parse(event: String?, data: String): RouteStreamEvent? = when (event) {
        "progress" -> RouteStreamEvent.Progress(json.decodeFromString<CoordsDto>(data).coordinates.toLatLngs())
        "result" -> RouteStreamEvent.Result(json.decodeFromString<RoutePlanDto>(data).toDomain())
        "error" -> RouteStreamEvent.Failure(json.decodeFromString<ErrorDto>(data).error ?: DEFAULT_ERROR)
        else -> null
    }

    const val DEFAULT_ERROR = "The route could not be solved."
}

private fun List<List<Double>>.toLatLngs(): List<LatLng> =
    mapNotNull { pair -> if (pair.size >= 2) LatLng(pair[1], pair[0]) else null }

@Serializable
private data class RouteReqDto(
    val points: List<List<Double>>,
    val preset: String? = null,
    val profile: String? = null,
    @SerialName("round_trip") val roundTrip: Boolean? = null,
)

@Serializable
private data class CoordsDto(val coordinates: List<List<Double>> = emptyList())

@Serializable
private data class ErrorDto(val error: String? = null)

@Serializable
private data class GeometryDto(val coordinates: List<List<Double>> = emptyList())

@Serializable
private data class RoutePlanDto(
    @SerialName("distance_m") val distanceM: Double = 0.0,
    @SerialName("duration_s") val durationS: Double = 0.0,
    @SerialName("ascent_m") val ascentM: Double = 0.0,
    @SerialName("on_trail_pct") val onTrailPct: Double = 0.0,
    val surfaces: Map<String, Double> = emptyMap(),
    val geometry: GeometryDto = GeometryDto(),
) {
    fun toDomain(): RoutePlan = RoutePlan(
        distanceM = distanceM,
        durationS = durationS,
        ascentM = ascentM,
        onTrailPct = onTrailPct,
        surfaces = surfaces,
        geometry = geometry.coordinates.toLatLngs(),
    )
}
