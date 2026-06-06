package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.max

/** Pure geometry for "download the area along this route" — isolated for testing. */
object RouteCorridor {
    private const val METERS_PER_DEG_LAT = 111_320.0

    /**
     * The axis-aligned lat/lng box enclosing [points], padded outward by
     * [paddingMeters] on every side (so the route isn't flush to the edge).
     * Null for fewer than two points.
     */
    fun bounds(points: List<LatLng>, paddingMeters: Double = 1500.0): GeoBounds? {
        if (points.size < 2) return null
        val minLat = points.minOf { it.lat }
        val maxLat = points.maxOf { it.lat }
        val minLng = points.minOf { it.lng }
        val maxLng = points.maxOf { it.lng }
        val padLat = paddingMeters / METERS_PER_DEG_LAT
        val midLat = (minLat + maxLat) / 2
        val padLng = paddingMeters / (METERS_PER_DEG_LAT * max(0.01, cos(Math.toRadians(midLat))))
        return GeoBounds(
            south = (minLat - padLat).coerceAtLeast(-85.0),
            west = (minLng - padLng).coerceAtLeast(-180.0),
            north = (maxLat + padLat).coerceAtMost(85.0),
            east = (maxLng + padLng).coerceAtMost(180.0),
        )
    }

    /** Rough span (deg) of a box — a cheap guard against absurd download areas. */
    fun spanDegrees(b: GeoBounds): Double = max(abs(b.north - b.south), abs(b.east - b.west))
}
