package com.sigmundgranaas.turbo.expressive.domain

import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.atan
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.ln
import kotlin.math.sinh
import kotlin.math.tan

/**
 * A routing pack: the terrain, trails and water for one region, so routes
 * can be planned on the phone.
 *
 * This is the *domain* half — what a pack is called, where it lives, and
 * how big one will be. Downloading them is `:core:map`'s job and routing
 * on them is `:core:routing-android`'s; both agree here rather than
 * depending on each other, the same way the tile store and the map
 * engine already agree on a directory.
 */
object RoutingPack {

    /** Directory under `filesDir` holding one subdirectory per pack. */
    const val DIR: String = "routing-packs"

    /** The manifest, fetched first: it triggers the server-side build and
     *  carries the sizes and digests the rest of the download needs. */
    const val MANIFEST: String = "pack.toml"

    /**
     * Bytes per square kilometre, for sizing a download before one exists.
     *
     * Measured across two real packs at 16–21 KB/km²; 20 is the top of
     * that range on purpose. An estimate that undershoots turns into a
     * progress bar that stalls near the end and a user who thinks the
     * download hung — overshooting just makes it finish early.
     */
    const val BYTES_PER_SQ_KM: Long = 20_000L

    /**
     * Zoom the pack grid quantises to. Must match the server's
     * `PACK_GRID_Z`: it is half of the shared name for a pack, and a
     * mismatch means the client asks for regions the cache has never
     * seen — every download a cold build, and no two users sharing one.
     */
    const val GRID_Z: Int = 12

    /**
     * The pack key covering [bounds] — the name the server knows a
     * region by, and the directory it lands in.
     *
     * Snapping is **outward**, to a grid, and both halves matter. Outward
     * because a pack that covers less than the viewport has a hole at
     * the edge, which is exactly where someone who framed their screen
     * deliberately is going to tap. To a grid because the CDN in front of
     * the server keys on the URL: a free-form bounding box would give two
     * people framing the same valley two cache entries and two builds of
     * a multi-megabyte object.
     */
    fun keyFor(bounds: GeoBounds, z: Int = GRID_Z): String {
        val n = 1 shl z
        val x0 = lonToX(bounds.west, n)
        val x1 = lonToX(bounds.east, n)
        // Slippy `y` grows southward, so the NORTH edge is the smaller y.
        val y0 = latToY(bounds.north, n)
        val y1 = latToY(bounds.south, n)
        return "z${z}_${minOf(x0, x1)}_${minOf(y0, y1)}_${maxOf(x0, x1)}_${maxOf(y0, y1)}"
    }

    /** The ground [keyFor] actually covers — always ⊇ the request. */
    fun extentOf(key: String): GeoBounds? {
        val parts = key.removePrefix("z").split("_")
        if (parts.size != 5) return null
        val z = parts[0].toIntOrNull() ?: return null
        val v = parts.drop(1).map { it.toIntOrNull() ?: return null }
        val n = 1 shl z
        return GeoBounds(
            west = xToLon(v[0], n),
            north = yToLat(v[1], n),
            east = xToLon(v[2] + 1, n),
            south = yToLat(v[3] + 1, n),
        )
    }

    /**
     * Largest region one pack may cover, in km². **Must match the
     * server's `MAX_AREA_SQ_KM`.**
     *
     * The server refuses a bigger region with a 400, and a 400 arriving
     * mid-download is a bad way to learn this: the request has already
     * been made, and there is nothing useful to do with the answer. So
     * the client applies the same rule first, with the same formula, and
     * simply does not ask.
     *
     * The number is not arbitrary and is not about the phone. It is what
     * keeps a server-side cut inside the window a request waits inline
     * (measured: ~11 s at the cap) and the pack inside what is sane to
     * hand a walker on a mountain connection (~80 MB).
     *
     * # Why an area and not a span or a cell count
     *
     * This cap used to be expressed server-side in grid cells, and that
     * was wrong in a way worth remembering here: a z12 cell is square on
     * the ground but *shrinks* as latitude rises, so 400 cells is
     * 5 842 km² at 67°N and 11 017 km² at 58°N. A cap in cells is a cap
     * that is nearly twice as loose in southern Norway as in the north.
     */
    const val MAX_AREA_SQ_KM: Double = 5_500.0

    /**
     * Ground area of the pack covering [bounds], in km².
     *
     * The *covered* area, not the requested one: [keyFor] snaps outward,
     * so the server always builds something bigger than the viewport and
     * it is the bigger figure both sides must judge.
     */
    fun areaSqKm(bounds: GeoBounds): Double {
        val covered = extentOf(keyFor(bounds)) ?: bounds
        val midLat = (covered.north + covered.south) / 2.0
        val kmNorthSouth = abs(covered.north - covered.south) * DEG_TO_KM
        val kmEastWest = abs(covered.east - covered.west) * DEG_TO_KM * cos(midLat * PI / 180.0)
        return kmNorthSouth * kmEastWest
    }

    /** Can one pack cover [bounds], or is the region past [MAX_AREA_SQ_KM]? */
    fun fitsOnePack(bounds: GeoBounds): Boolean = areaSqKm(bounds) <= MAX_AREA_SQ_KM

    /** Rough size of the pack covering [bounds], for a pre-download estimate. */
    fun estimatedBytes(bounds: GeoBounds): Long =
        (areaSqKm(bounds) * BYTES_PER_SQ_KM).toLong()

    private const val DEG_TO_KM = 111.320

    private fun lonToX(lon: Double, n: Int): Int =
        floor((lon + 180.0) / 360.0 * n).toInt().coerceIn(0, n - 1)

    private fun latToY(lat: Double, n: Int): Int {
        val r = lat.coerceIn(-85.05112878, 85.05112878) * PI / 180.0
        return floor((1.0 - ln(tan(r) + 1.0 / cos(r)) / PI) / 2.0 * n).toInt().coerceIn(0, n - 1)
    }

    private fun xToLon(x: Int, n: Int): Double = x.toDouble() / n * 360.0 - 180.0

    private fun yToLat(y: Int, n: Int): Double =
        atan(sinh(PI * (1.0 - 2.0 * y.toDouble() / n))) * 180.0 / PI
}
