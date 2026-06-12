package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import java.io.File

/**
 * A tiny read-through disk cache for the turbomap host's raster tiles.
 *
 * The host fetches tiles itself (it owns the URL templates + auth); this is the
 * "host owns caching/offline" half of the pull/push contract. Tiles are keyed by
 * `layer/z/x/y` and stored as opaque bytes under [dir], so a previously-viewed
 * area renders from disk with no network — the foundation the richer offline
 * (download-region) work builds on. Pure file IO; safe to call from a worker
 * thread (the host already fetches off the main thread).
 */
internal class TurbomapTileCache(private val dir: File) {

    /** Cached bytes for `layer/z/x/y`, or null on a miss / unreadable entry. */
    fun get(layer: String, z: Int, x: Int, y: Int): ByteArray? {
        val f = fileFor(layer, z, x, y)
        return if (f.isFile && f.length() > 0) f.runCatching { readBytes() }.getOrNull() else null
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

    /** Drop a cached entry — used when its bytes turn out not to decode (a poisoned tile). */
    fun remove(layer: String, z: Int, x: Int, y: Int) {
        runCatching { fileFor(layer, z, x, y).delete() }
    }

    private fun fileFor(layer: String, z: Int, x: Int, y: Int): File {
        // Sanitise the layer id (it comes from app-authored scene ids) to a flat filename.
        val safeLayer = layer.map { if (it.isLetterOrDigit() || it == '-' || it == '_') it else '_' }.joinToString("")
        return File(dir, "$safeLayer/$z/${x}_$y.tile")
    }
}
