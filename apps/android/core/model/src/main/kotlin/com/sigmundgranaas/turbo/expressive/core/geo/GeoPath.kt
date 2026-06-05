package com.sigmundgranaas.turbo.expressive.core.geo

import com.sigmundgranaas.turbo.expressive.domain.LatLng

/** Where a [GeoPath] came from — drives styling and available verbs. */
enum class GeoPathSource { Route, Recording, Measure, Saved, Trail, Activity }

/**
 * The single canonical "a line on the map" value type that every feature
 * (routes, recordings, saved tracks, activities, measure tool) converts to and
 * from. Mirrors the Flutter app's `GeoPath` seam. Provide `toGeoPath()`
 * converters in each feature rather than duplicating geometry handling.
 */
data class GeoPath(
    val points: List<LatLng>,
    val source: GeoPathSource,
    val elevations: List<Double?>? = null,
    val distanceM: Double = GeoMetrics.pathLengthMeters(points),
    val ascentM: Double? = null,
    val descentM: Double? = null,
    val movingTimeSeconds: Int? = null,
    val recordedAtEpochMs: Long? = null,
) {
    val isEmpty: Boolean get() = points.isEmpty()

    /** Axis-aligned lat/lng bounds, or null when empty. */
    val bounds: Bounds?
        get() = if (points.isEmpty()) null else Bounds(
            minLat = points.minOf { it.lat },
            minLng = points.minOf { it.lng },
            maxLat = points.maxOf { it.lat },
            maxLng = points.maxOf { it.lng },
        )

    data class Bounds(val minLat: Double, val minLng: Double, val maxLat: Double, val maxLng: Double) {
        val center: LatLng get() = LatLng((minLat + maxLat) / 2, (minLng + maxLng) / 2)
    }

    companion object {
        fun fromPoints(
            points: List<LatLng>,
            source: GeoPathSource,
            elevations: List<Double?>? = null,
        ): GeoPath {
            val (asc, desc) = GeoMetrics.gainLoss(elevations)
            return GeoPath(
                points = points,
                source = source,
                elevations = elevations,
                distanceM = GeoMetrics.pathLengthMeters(points),
                ascentM = asc,
                descentM = desc,
            )
        }
    }
}
