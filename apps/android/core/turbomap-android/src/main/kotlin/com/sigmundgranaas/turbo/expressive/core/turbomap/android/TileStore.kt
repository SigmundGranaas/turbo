package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import java.io.File

/**
 * Subdirectory under `context.cacheDir` where the wgpu map's tiles live. The map
 * reads tiles from here (read-through); the offline downloader pre-populates it
 * for downloaded regions. Shared so the map host and the offline manager agree
 * on one location — bump the suffix to invalidate every cached tile at once.
 */
const val TURBOMAP_TILE_DIR: String = "turbomap-tiles-v2"

/**
 * A tiny read-through disk store for the turbomap host's tiles, and the same
 * store the offline manager pre-populates for downloaded regions.
 *
 * The host fetches tiles itself (it owns the URL templates + auth) and serves
 * them from here, so a previously-viewed (or pre-downloaded) area renders from
 * disk with no network. Tiles are keyed by `layer/z/x/y` and stored as opaque
 * bytes under [dir] (`<layer>/<z>/<x>_<y>.tile`). Pure file IO; safe to call from
 * a worker thread (the host already fetches off the main thread).
 *
 * This is the non-MapLibre offline foundation: download = `put` a region's
 * tiles here; render = `get`; delete = `remove`.
 */
class TileStore(private val dir: File) {

    /** Cached bytes for `layer/z/x/y`, or null on a miss / unreadable entry. */
    fun get(layer: String, z: Int, x: Int, y: Int): ByteArray? {
        val f = fileFor(layer, z, x, y)
        return if (f.isFile && f.length() > 0) f.runCatching { readBytes() }.getOrNull() else null
    }

    /** True when `layer/z/x/y` is present on disk (cheap existence check, no read). */
    fun exists(layer: String, z: Int, x: Int, y: Int): Boolean {
        val f = fileFor(layer, z, x, y)
        return f.isFile && f.length() > 0
    }

    /** Store fetched bytes; writes atomically (temp + rename) so a crash can't leave a partial tile. */
    fun put(layer: String, z: Int, x: Int, y: Int, bytes: ByteArray) {
        if (bytes.isEmpty()) return
        val f = fileFor(layer, z, x, y)
        runCatching {
            f.parentFile?.mkdirs()
            val tmp = File(f.parentFile, "${f.name}.tmp")
            tmp.writeBytes(bytes)
            if (!tmp.renameTo(f)) {
                tmp.copyTo(f, overwrite = true)
                tmp.delete()
            }
        }
    }

    /** Drop a cached entry — a poisoned tile, or a deleted offline region's tiles. */
    fun remove(layer: String, z: Int, x: Int, y: Int) {
        runCatching { fileFor(layer, z, x, y).delete() }
    }

    /** On-disk size of `layer/z/x/y` in bytes, or 0 on a miss — for honest size totals. */
    fun size(layer: String, z: Int, x: Int, y: Int): Long {
        val f = fileFor(layer, z, x, y)
        return if (f.isFile) f.length() else 0L
    }

    /** The backing file for `layer/z/x/y` — lets the offline manager build the
     *  keep-set of region tiles for [pruneExcept] without reimplementing the layout. */
    fun fileOf(layer: String, z: Int, x: Int, y: Int): File = fileFor(layer, z, x, y)

    /**
     * Delete every stored tile whose file is NOT in [keep] — used to drop the
     * ambient (browse) cache while preserving downloaded regions, whose tiles the
     * caller passes via [fileOf]. Walks [dir]; only `*.tile` files are touched.
     */
    fun pruneExcept(keep: Set<File>) {
        val roots = keep.map { it.canonicalFile }.toHashSet()
        runCatching {
            dir.walkTopDown()
                .filter { it.isFile && it.name.endsWith(".tile") }
                .filter { it.canonicalFile !in roots }
                .forEach { it.delete() }
        }
    }

    private fun fileFor(layer: String, z: Int, x: Int, y: Int): File {
        // Sanitise the layer id (it comes from app-authored scene ids) to a flat filename.
        val safeLayer = layer.map { if (it.isLetterOrDigit() || it == '-' || it == '_') it else '_' }.joinToString("")
        return File(dir, "$safeLayer/$z/${x}_$y.tile")
    }
}
