package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.RoutingPack
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.security.MessageDigest

/**
 * The pack download, judged on how it FAILS.
 *
 * Succeeding is the easy half. What makes a pack different from a tile is
 * that a partial one still looks like a working one: a truncated DEM
 * opens, reports no data for the ground it is missing, and the router
 * reads that as untraversable. Nothing throws, nothing logs, and the
 * route goes round an obstacle that is not there.
 */
class PackDownloaderTest {

    @get:Rule
    val tmp = TemporaryFolder()

    private val bounds = GeoBounds(south = 67.02, west = 14.95, north = 67.12, east = 15.20)

    private fun sha(bytes: ByteArray): String =
        MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }

    /** A server holding one pack: manifest plus the files it lists. */
    private class FakeServer(files: Map<String, ByteArray>) {
        val files: MutableMap<String, ByteArray> = files.toMutableMap()
        var requests = mutableListOf<String>()
        var buildOnce = false

        fun manifest(sha: (ByteArray) -> String): ByteArray = buildString {
            appendLine("[pack]")
            appendLine("format_version = 1")
            appendLine("""frame = "utm33n"""")
            appendLine("extent = [14.95, 67.02, 15.2, 67.12]")
            files.toSortedMap().forEach { (name, bytes) ->
                appendLine()
                appendLine("[[pack.files]]")
                appendLine("""name = "$name"""")
                appendLine("bytes = ${bytes.size}")
                appendLine("""sha256 = "${sha(bytes)}"""")
            }
        }.toByteArray()
    }

    private fun downloader(
        server: FakeServer,
        corrupt: Set<String> = emptySet(),
        truncate: Set<String> = emptySet(),
        missing: Set<String> = emptySet(),
    ) = PackDownloader(
        root = tmp.root,
        baseUrl = "https://example.test/v1/packs",
        fetch = { url, into ->
            val name = url.substringAfterLast('/')
            server.requests += name
            val body: ByteArray? = when {
                name in missing -> null
                name == RoutingPack.MANIFEST -> server.manifest(::sha)
                else -> server.files[name]
            }
            if (server.buildOnce && name == RoutingPack.MANIFEST) {
                server.buildOnce = false
                PackDownloader.FetchResult.Building(1)
            } else if (body == null) {
                PackDownloader.FetchResult.NotFound
            } else {
                val written = when {
                    name in corrupt -> ByteArray(body.size) { 0 }
                    name in truncate -> body.copyOfRange(0, body.size / 2)
                    else -> body
                }
                into.parentFile?.mkdirs()
                into.writeBytes(written)
                PackDownloader.FetchResult.Ok(written.size.toLong())
            }
        },
    )

    private fun server() = FakeServer(
        mapOf(
            "norway.dem" to ByteArray(4096) { (it % 251).toByte() },
            "norway.mask" to ByteArray(512) { 3 },
            "norway.graph" to ByteArray(256) { 7 },
        ),
    )

    @Test
    fun `a complete pack lands in a directory named for its key`() = runTest {
        val s = server()
        val out = downloader(s).download(bounds)
        assertTrue("expected Done, got $out", out is PackDownloader.Outcome.Done)
        val done = out as PackDownloader.Outcome.Done

        assertEquals(RoutingPack.keyFor(bounds), done.key)
        assertTrue(File(done.dir, RoutingPack.MANIFEST).isFile)
        assertTrue(File(done.dir, "norway.dem").isFile)
        assertEquals(4096L + 512L + 256L, done.bytes)

        // The manifest is fetched first: it triggers the server-side build
        // and it is what the rest of the download is driven from.
        assertEquals(RoutingPack.MANIFEST, s.requests.first())
        // And the file list comes FROM it, rather than being hard-coded
        // here — a pack format that gains a file must not silently skip it.
        assertTrue(s.requests.containsAll(listOf("norway.dem", "norway.mask", "norway.graph")))
    }

    @Test
    fun `a truncated file fails the download instead of being kept`() = runTest {
        val out = downloader(server(), truncate = setOf("norway.dem")).download(bounds)
        assertTrue(out is PackDownloader.Outcome.Failed)
        assertTrue(
            "the message should name the file and the shortfall: $out",
            (out as PackDownloader.Outcome.Failed).reason.contains("truncated"),
        )
        // Nothing left behind. A half-pack on disk is worse than none: a
        // truncated DEM opens and answers "no data" for the ground it is
        // missing, which the router reads as untraversable.
        assertNoLeftovers()
    }

    @Test
    fun `a corrupt file fails even when the length is right`() = runTest {
        // What a length check cannot see, and what a CDN in the path makes
        // possible. Same size, different bytes.
        val out = downloader(server(), corrupt = setOf("norway.mask")).download(bounds)
        assertTrue(out is PackDownloader.Outcome.Failed)
        assertTrue((out as PackDownloader.Outcome.Failed).reason.contains("corrupt"))
        assertNoLeftovers()
    }

    @Test
    fun `a missing file is an error, never an empty tile`() = runTest {
        // The tile loop treats absence as legitimate — an ocean DEM tile
        // genuinely has no data. For a pack it never is, and treating a
        // 404 the tile way would mark a broken pack complete.
        val out = downloader(server(), missing = setOf("norway.graph")).download(bounds)
        assertTrue("a missing pack file must fail the download: $out", out is PackDownloader.Outcome.Failed)
        assertNoLeftovers()
    }

    @Test
    fun `a 202 is waited out, not treated as a failure`() = runTest {
        // The server cuts a region on first request. "Come back" is the
        // normal path for a region nobody has asked for before.
        val s = server().apply { buildOnce = true }
        var waited = 0
        val out = downloader(s).download(bounds, waitForBuild = { waited++ })
        assertTrue("expected Done after the build, got $out", out is PackDownloader.Outcome.Done)
        assertEquals(1, waited)
    }

    @Test
    fun `an already-downloaded pack is not fetched again`() = runTest {
        val s = server()
        downloader(s).download(bounds)
        s.requests.clear()
        val out = downloader(s).download(bounds)
        assertTrue(out is PackDownloader.Outcome.Done)
        assertTrue("a present pack must not be re-fetched: ${s.requests}", s.requests.isEmpty())
    }

    @Test
    fun `a server with no pack endpoint is not a failure`() = runTest {
        // The merge-order landmine. The app and the tileserver ship
        // separately, so whichever lands first there is a window where an
        // app that asks for packs meets a server that has never heard of
        // them. Failing here would break offline map downloads — which
        // work today — for everyone in that window.
        val out = downloader(server(), missing = setOf(RoutingPack.MANIFEST)).download(bounds)
        assertTrue(
            "a missing manifest means this server serves no packs, which is a " +
                "deployment state rather than a broken download: got $out",
            out is PackDownloader.Outcome.Unsupported,
        )
        assertNoLeftovers()
    }

    @Test
    fun `a file missing AFTER the manifest listed it is still a failure`() = runTest {
        // The distinction the case above rests on. This server does serve
        // packs — it answered with a manifest — so a file it named and
        // cannot deliver is broken, not absent.
        val out = downloader(server(), missing = setOf("norway.dem")).download(bounds)
        assertTrue(
            "a server that lists a file and then 404s it is broken: got $out",
            out is PackDownloader.Outcome.Failed,
        )
        assertNoLeftovers()
    }

    @Test
    fun `deleting a region deletes its pack`() = runTest {
        val d = downloader(server())
        d.download(bounds)
        assertTrue(File(tmp.root, RoutingPack.keyFor(bounds)).isDirectory)
        d.delete(bounds)
        assertFalse(
            "a deleted region must not leave megabytes the user cannot see",
            File(tmp.root, RoutingPack.keyFor(bounds)).exists(),
        )
    }

    private fun assertNoLeftovers() {
        val stray = tmp.root.listFiles()?.map { it.name }.orEmpty()
        assertTrue("a failed download must leave nothing behind: $stray", stray.isEmpty())
    }
}
