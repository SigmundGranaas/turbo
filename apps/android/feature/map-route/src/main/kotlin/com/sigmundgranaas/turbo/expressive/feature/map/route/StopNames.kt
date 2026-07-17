package com.sigmundgranaas.turbo.expressive.feature.map.route

import com.sigmundgranaas.turbo.expressive.core.data.ReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import java.util.concurrent.ConcurrentHashMap
import kotlin.math.roundToInt

/**
 * Pure display rules for a route stop's label + its stable palette colour. Both are
 * keyed on a coarse (~11 m) grid cell so they follow the *coordinate*, not the list
 * position — that is what makes a name/colour survive a reorder or a re-solve.
 */
object StopLabels {

    // ~11 m at Nordic latitudes: 0.0001° lat ≈ 11.1 m. Quantize and pack into a stable key.
    private const val GRID = 10_000.0

    /** Stable ~11 m grid-cell key for a point — the cache key + colour seed. */
    fun gridKey(point: LatLng): Long {
        val qLat = (point.lat * GRID).roundToInt()
        val qLng = (point.lng * GRID).roundToInt()
        return (qLat.toLong() shl 32) xor (qLng.toLong() and 0xFFFFFFFFL)
    }

    /** Trimmed plain-decimal coordinates, e.g. `69.9607, 23.2715` — the always-available
     *  fallback shown in the row's single-line slot until (if ever) a name resolves. */
    fun trimmedCoords(point: LatLng): String =
        "%.4f, %.4f".format(point.lat, point.lng)

    /**
     * What the row renders in its single fixed-height line: the cached name when there is
     * one, otherwise the trimmed coordinates. Because both occupy the same slot, resolving
     * a name is an in-place text swap — no reflow.
     */
    fun label(cachedName: String?, point: LatLng): String =
        cachedName?.takeIf { it.isNotBlank() } ?: trimmedCoords(point)
}

/**
 * A small categorical palette for intermediate stops (the vias). A stop's colour is chosen
 * by its stable grid key, so the same place always draws the same colour in the list and on
 * the map, regardless of where it sits in the order. Start/end keep their role colours (see
 * [stopColor]); this covers the vias between them.
 */
object StopPalette {
    // Distinct, map-legible hues (ARGB). Deliberately excludes the start-green / end-red roles.
    private val colors = listOf(
        0xFF1E88E5, // blue
        0xFF8E24AA, // purple
        0xFFF9A825, // amber
        0xFF00897B, // teal
        0xFFD81B60, // pink
        0xFF6D4C41, // brown
    )

    /** The via colour (ARGB) for a stop, stable across reorder because it is keyed on the
     *  coordinate's grid cell, not its index. */
    fun colorOf(point: LatLng): Long {
        val key = StopLabels.gridKey(point)
        val idx = ((key xor (key ushr 32)).toInt() and Int.MAX_VALUE) % colors.size
        return colors[idx]
    }
}

/**
 * Lazily reverse-geocodes route stops and caches the resulting name per ~11 m grid cell, so a
 * stop's name is fetched at most once and then re-used across re-renders, re-solves and
 * reorders. Never throws and never blocks the solve: an offline / failed lookup simply yields
 * `null`, and the row falls back to [StopLabels.trimmedCoords].
 */
class StopNames(private val geocoder: ReverseGeocodeRepository) {

    private val cache = ConcurrentHashMap<Long, String>()

    /** The already-resolved name for a stop, or null if it hasn't been fetched yet. */
    fun cached(point: LatLng): String? = cache[StopLabels.gridKey(point)]

    /**
     * Resolve a stop's name. A cache hit returns immediately (no network); otherwise it
     * reverse-geocodes once, caches the title, and returns it. Any failure (offline, no
     * match) returns null and caches nothing, so a later online attempt can still succeed.
     */
    suspend fun resolve(point: LatLng): String? {
        val key = StopLabels.gridKey(point)
        cache[key]?.let { return it }
        val name = runCatching { geocoder.describe(point).getOrNull()?.title }
            .getOrNull()
            ?.takeIf { it.isNotBlank() }
        if (name != null) cache[key] = name
        return name
    }
}
