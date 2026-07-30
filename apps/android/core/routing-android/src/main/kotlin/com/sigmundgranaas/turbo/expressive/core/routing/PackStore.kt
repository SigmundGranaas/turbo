package com.sigmundgranaas.turbo.expressive.core.routing

import java.io.File

/**
 * Which downloaded region packs exist, and which one covers a request.
 *
 * A pack is a directory of `norway.{dem,mask,graph,graph_geom}` plus a
 * `pack.toml`, produced by `tileserver slice-pack`. One engine binds to exactly
 * one pack, so something has to choose — and the choice is coverage, not
 * recency or proximity: a route whose endpoints straddle two packs cannot be
 * solved by either, and answering with the nearer one would return a route that
 * stops at a border the user cannot see.
 */
class PackStore(private val root: File) {

    /** A pack on disk, identified by its directory name. */
    data class Pack(val id: String, val dir: File, val extent: Extent)

    /** WGS84 bounds, as the manifest records them. */
    data class Extent(
        val minLon: Double,
        val minLat: Double,
        val maxLon: Double,
        val maxLat: Double,
    ) {
        fun contains(lon: Double, lat: Double): Boolean =
            lon in minLon..maxLon && lat in minLat..maxLat
    }

    fun packs(): List<Pack> =
        (root.listFiles()?.filter { it.isDirectory } ?: emptyList())
            .mapNotNull { dir ->
                val extent = readExtent(File(dir, MANIFEST)) ?: return@mapNotNull null
                Pack(dir.name, dir, extent)
            }
            .sortedBy { it.id }

    /**
     * The pack covering every one of [points], or `null`.
     *
     * All points, not the first: a request the chosen pack only partly covers
     * fails at solve time with an error about terrain, which reads to the user
     * as "the router is broken" rather than "you have not downloaded that area".
     */
    fun covering(points: List<Pair<Double, Double>>): Pack? =
        packs().firstOrNull { p -> points.all { (lon, lat) -> p.extent.contains(lon, lat) } }

    private companion object {
        const val MANIFEST = "pack.toml"

        /**
         * Read `extent = [minLon, minLat, maxLon, maxLat]` out of the manifest.
         *
         * A four-line parser rather than a TOML dependency, because this reads
         * one array from a file this repo also writes. If the manifest ever
         * grows something structural, take the dependency then — but a parser
         * that quietly mis-reads a hand-edited file is worse than one that
         * returns null, so anything unexpected is null and the pack is skipped.
         */
        fun readExtent(file: File): Extent? {
            if (!file.isFile) return null
            val line = file.readLines()
                .map { it.substringBefore('#').trim() }
                .firstOrNull { it.startsWith("extent") }
                ?: return null
            val nums = line.substringAfter('[').substringBefore(']')
                .split(',')
                .mapNotNull { it.trim().toDoubleOrNull() }
            if (nums.size != 4) return null
            return Extent(nums[0], nums[1], nums[2], nums[3])
        }
    }
}
