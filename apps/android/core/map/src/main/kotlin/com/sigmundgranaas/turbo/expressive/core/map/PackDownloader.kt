package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.RoutingPack
import java.io.File
import java.security.MessageDigest

/**
 * Fetches one region's routing pack into `filesDir/routing-packs/<key>/`.
 *
 * Deliberately separate from the tile loop rather than another lane in
 * it. A tile is 20 KB, is either on disk or not, and may legitimately be
 * absent — an ocean DEM tile has no data, and `FetchOutcome.Absent`
 * exists to say so. Every one of those is false for a pack file:
 *
 * - **Size.** The DEM is megabytes and can be *half*-written. Resume by
 *   "skip what exists" would then leave a truncated file — and a
 *   truncated DEM does not fail to open. It opens, answers "no data" for
 *   the ground that is missing, and the router reads that as untraversable
 *   and goes around. Silent, wrong, and shaped exactly like a coastline.
 *   So each file is checked against the manifest's size and digest.
 *
 * - **Absence.** A missing pack file is never legitimate. Treating a 404
 *   as "no data here" would mark a broken pack complete.
 *
 * - **Atomicity.** A pack is only usable whole, so it is assembled in a
 *   `.partial` directory and renamed. A reader sees a complete pack or
 *   no pack.
 */
class PackDownloader(
    private val root: File,
    private val baseUrl: String,
    private val fetch: suspend (url: String, into: File) -> FetchResult,
) {

    sealed interface FetchResult {
        data class Ok(val bytes: Long) : FetchResult
        /** The server is still cutting this region — retry the same URL. */
        data class Building(val retryAfterSeconds: Int) : FetchResult
        data class Failed(val reason: String) : FetchResult
    }

    sealed interface Outcome {
        data class Done(val key: String, val dir: File, val bytes: Long) : Outcome
        data class Failed(val reason: String) : Outcome
    }

    /** One file's expected size and digest, from the manifest. */
    private data class Expected(val name: String, val bytes: Long, val sha256: String)

    /**
     * Download the pack covering [bounds], reporting bytes as they land.
     *
     * [onProgress] receives (bytesSoFar, bytesTotal); the total is only
     * known after the manifest, which is why the manifest is fetched
     * first and separately.
     */
    suspend fun download(
        bounds: GeoBounds,
        onProgress: (Long, Long) -> Unit = { _, _ -> },
        waitForBuild: suspend (seconds: Int) -> Unit = {},
    ): Outcome {
        val key = RoutingPack.keyFor(bounds)
        val done = File(root, key)
        if (File(done, RoutingPack.MANIFEST).isFile) {
            return Outcome.Done(key, done, done.walkTopDown().filter { it.isFile }.sumOf { it.length() })
        }

        val partial = File(root, "$key.partial")
        partial.deleteRecursively()
        if (!partial.mkdirs()) return Outcome.Failed("Couldn't create $partial")

        // The manifest first: it is what triggers the server-side build,
        // and it carries the file list this download is driven from. A
        // hard-coded list here would silently skip a file a future pack
        // format adds.
        val manifestFile = File(partial, RoutingPack.MANIFEST)
        when (val r = fetchWithBuildWait(url(key, RoutingPack.MANIFEST), manifestFile, waitForBuild)) {
            is FetchResult.Ok -> Unit
            is FetchResult.Building -> return Outcome.Failed("The map server is still preparing this area.")
            is FetchResult.Failed -> return Outcome.Failed(r.reason)
        }

        val expected = parseManifest(manifestFile.readText())
        if (expected.isEmpty()) {
            partial.deleteRecursively()
            return Outcome.Failed("The routing pack's manifest listed no files.")
        }

        val total = expected.sumOf { it.bytes }
        var soFar = 0L
        for (f in expected) {
            val target = File(partial, f.name)
            when (val r = fetchWithBuildWait(url(key, f.name), target, waitForBuild)) {
                is FetchResult.Ok -> Unit
                is FetchResult.Building -> {
                    partial.deleteRecursively()
                    return Outcome.Failed("The map server is still preparing this area.")
                }
                is FetchResult.Failed -> {
                    partial.deleteRecursively()
                    return Outcome.Failed(r.reason)
                }
            }
            verify(target, f)?.let {
                partial.deleteRecursively()
                return Outcome.Failed(it)
            }
            soFar += f.bytes
            onProgress(soFar, total)
        }

        done.deleteRecursively()
        if (!partial.renameTo(done)) {
            partial.deleteRecursively()
            return Outcome.Failed("Couldn't finish the routing pack.")
        }
        return Outcome.Done(key, done, total)
    }

    /** Delete the pack covering [bounds], if this app downloaded one. */
    fun delete(bounds: GeoBounds) {
        File(root, RoutingPack.keyFor(bounds)).deleteRecursively()
    }

    /**
     * Retry through a `202 Building`.
     *
     * The server cuts a region on first request; for a large one that
     * outlives the request window and it answers "come back". Bounded
     * retries rather than a loop: a server that is always building is a
     * server that is broken, and a client that waits forever hides it.
     */
    private suspend fun fetchWithBuildWait(
        url: String,
        into: File,
        waitForBuild: suspend (Int) -> Unit,
    ): FetchResult {
        var result = fetch(url, into)
        var attempts = 0
        while (result is FetchResult.Building && attempts < BUILD_RETRIES) {
            waitForBuild(result.retryAfterSeconds)
            attempts++
            result = fetch(url, into)
        }
        return result
    }

    private fun url(key: String, file: String) = "$baseUrl/$key/$file"

    /**
     * Size and digest, in that order.
     *
     * Size first because it is free and catches the common case — a
     * truncated download — with a message naming the number that is
     * wrong. The digest then catches corruption a length check cannot
     * see, which is what a CDN in the path makes possible.
     */
    private fun verify(file: File, expected: Expected): String? {
        if (!file.isFile) return "${expected.name} is missing from the routing pack."
        if (file.length() != expected.bytes) {
            return "${expected.name} arrived truncated (${file.length()} of ${expected.bytes} bytes)."
        }
        if (expected.sha256.isNotEmpty() && sha256(file) != expected.sha256) {
            return "${expected.name} arrived corrupt."
        }
        return null
    }

    private fun sha256(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buf = ByteArray(1 shl 16)
            while (true) {
                val n = input.read(buf)
                if (n <= 0) break
                digest.update(buf, 0, n)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    /**
     * Pull `[[pack.files]]` entries out of the manifest.
     *
     * A hand-rolled reader rather than a TOML dependency, because this
     * reads three keys from a file this repo also writes, in a format
     * the writer pins with a round-trip test. Anything unexpected yields
     * no entries, which fails the download loudly — the failure mode a
     * lenient parser would turn into a pack with a file missing.
     */
    private fun parseManifest(text: String): List<Expected> {
        val out = mutableListOf<Expected>()
        var name: String? = null
        var bytes: Long? = null
        var sha: String? = null

        fun flush() {
            val n = name
            val b = bytes
            if (n != null && b != null) out += Expected(n, b, sha.orEmpty())
            name = null; bytes = null; sha = null
        }

        for (raw in text.lineSequence()) {
            val line = raw.substringBefore('#').trim()
            when {
                line == "[[pack.files]]" -> flush()
                line.startsWith("name") -> name = line.substringAfter('=').trim().trim('"')
                line.startsWith("bytes") -> bytes = line.substringAfter('=').trim().toLongOrNull()
                line.startsWith("sha256") -> sha = line.substringAfter('=').trim().trim('"')
            }
        }
        flush()
        return out
    }

    private companion object {
        const val BUILD_RETRIES = 12
    }
}
