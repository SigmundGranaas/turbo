package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.OfflineEstimate
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.asinh
import kotlin.math.floor
import kotlin.math.tan

/**
 * Pure slippy-map (Web Mercator XYZ) math for estimating how big an offline
 * download will be, plus a guard against absurdly large areas. No MapLibre — kept
 * here so it is unit-testable and reusable by the pre-flight estimate UI.
 */
object TileMath {

    /** Average bytes for a 256px raster tile — empirical, tunable. */
    private const val AVG_RASTER_TILE_BYTES = 20_000L

    /** Hard ceiling mirroring MapLibre's tile-count limit; downloads above this fail. */
    const val MAX_TILES = 100_000L

    /** Reject single-shot downloads whose bounding box spans more than this (degrees). */
    const val MAX_SPAN_DEGREES = 6.0

    private fun lonToTileX(lon: Double, z: Int): Int {
        val n = 1 shl z
        return floor((lon + 180.0) / 360.0 * n).toInt().coerceIn(0, n - 1)
    }

    private fun latToTileY(lat: Double, z: Int): Int {
        val n = 1 shl z
        val clamped = lat.coerceIn(-85.05112878, 85.05112878)
        val rad = Math.toRadians(clamped)
        // y = (1 - asinh(tan φ)/π) / 2 · n  — asinh(tan φ) == ln(tan φ + sec φ).
        val y = (1.0 - asinh(tan(rad)) / PI) / 2.0 * n
        return floor(y).toInt().coerceIn(0, n - 1)
    }

    /** A single slippy-map tile coordinate. */
    data class TileXyz(val z: Int, val x: Int, val y: Int)

    /**
     * Enumerate every `(z, x, y)` one source covers over [bounds] across integer
     * zooms `floor(min)..floor(max)` — the exact tiles the offline downloader must
     * fetch (and the map will request) for a region. By construction
     * `tilesFor(...).size.toLong() == tileCount(...)`.
     */
    fun tilesFor(bounds: GeoBounds, minZoom: Double, maxZoom: Double): List<TileXyz> {
        val lo = floor(minZoom).toInt().coerceAtLeast(0)
        val hi = floor(maxZoom).toInt().coerceAtLeast(lo)
        val out = ArrayList<TileXyz>()
        for (z in lo..hi) {
            val x0 = lonToTileX(bounds.west, z)
            val x1 = lonToTileX(bounds.east, z)
            // Mercator Y grows southward, so north maps to the smaller index.
            val y0 = latToTileY(bounds.north, z)
            val y1 = latToTileY(bounds.south, z)
            for (x in x0..x1) for (y in y0..y1) out.add(TileXyz(z, x, y))
        }
        return out
    }

    /** Tiles for one source over [bounds] across integer zooms `floor(min)..floor(max)`. */
    fun tileCount(bounds: GeoBounds, minZoom: Double, maxZoom: Double): Long {
        val lo = floor(minZoom).toInt().coerceAtLeast(0)
        val hi = floor(maxZoom).toInt().coerceAtLeast(lo)
        var total = 0L
        for (z in lo..hi) {
            val x0 = lonToTileX(bounds.west, z)
            val x1 = lonToTileX(bounds.east, z)
            // Mercator Y grows southward, so north maps to the smaller index.
            val y0 = latToTileY(bounds.north, z)
            val y1 = latToTileY(bounds.south, z)
            val nx = (x1 - x0).toLong() + 1
            val ny = (y1 - y0).toLong() + 1
            total += nx * ny
        }
        return total
    }

    /**
     * Estimate for a [spec]: the base map plus each overlay is its own tile pyramid,
     * so the tile count is multiplied by the number of sources.
     */
    fun estimate(spec: DownloadSpec): OfflineEstimate {
        val sources = 1 + spec.overlays.size
        val tiles = tileCount(spec.bounds, spec.minZoom, spec.maxZoom) * sources
        val span = maxOf(
            abs(spec.bounds.north - spec.bounds.south),
            abs(spec.bounds.east - spec.bounds.west),
        )
        return OfflineEstimate(
            tiles = tiles,
            bytes = tiles * AVG_RASTER_TILE_BYTES,
            withinLimits = span <= MAX_SPAN_DEGREES && tiles <= MAX_TILES,
        )
    }

    /** Cheap guard: too many tiles, or a box that's simply too wide to be sensible. */
    fun isWithinLimits(spec: DownloadSpec): Boolean = estimate(spec).withinLimits
}
