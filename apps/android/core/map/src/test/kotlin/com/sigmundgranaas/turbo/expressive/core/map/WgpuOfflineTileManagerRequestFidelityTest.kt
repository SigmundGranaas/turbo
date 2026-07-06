package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.core.turbomap.android.TileStore
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import okio.Buffer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.util.Collections
import java.util.concurrent.TimeUnit

/**
 * Request-fidelity gate for the offline downloader, against a tile server WE
 * fully control (OkHttp [MockWebServer]) — hermetic, deterministic, and safe
 * to run on every push (no live third-party dependency, unlike the opt-in
 * [WgpuOfflineTileManagerRealNetworkTest] that hits Kartverket).
 *
 * The point is exactness: the local server records every path it is asked for
 * and serves a KNOWN tile body only for the tiles the region should contain
 * (404 for anything else). The test then asserts the manager requested **the
 * exact set** `TileMath.tilesFor(bounds, min, max)` maps to — no missing tile,
 * no stray request, no duplicate — stored our known bytes byte-for-byte, and
 * reported honest size accounting. Then it evicts and re-downloads and asserts
 * the same controlled set is fetched again.
 *
 * This exercises the real production path end to end: real `WgpuOfflineTile
 * Manager` orchestration (parallelism, retry, progress, region persistence,
 * delete/prune) + a real OkHttp client over a real socket — only the *origin*
 * is ours.
 */
class WgpuOfflineTileManagerRequestFidelityTest {

    @get:Rule val tmp = TemporaryFolder()

    private lateinit var server: MockWebServer

    // Every path the server was asked for, in order (to catch duplicates too).
    private val requested = Collections.synchronizedList(mutableListOf<String>())

    // A tiny but valid 1x1 PNG — the known tile body every served tile carries,
    // so "stored == served" is a byte-for-byte assertion.
    private val tileBody: ByteArray = byteArrayOf(
        0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00,
        0x1F, 0x15, 0xC4.toByte(), 0x89.toByte(),
        0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, // IDAT
        0x78, 0x9C.toByte(), 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
        0x0D, 0x0A, 0x2D, 0xB4.toByte(),
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE.toByte(), 0x42, 0x60, 0x82.toByte(), // IEND
    )

    private val bounds = GeoBounds(south = 67.25, west = 15.00, north = 67.27, east = 15.04)
    private fun spec() = DownloadSpec(
        name = "Sjunkhatten",
        base = BaseLayer.Norgeskart,
        bounds = bounds,
        minZoom = 8.0,
        maxZoom = 9.0,
    )

    /** The exact set of tiles a Norgeskart-only download of this region covers. */
    private val expectedTiles by lazy { TileMath.tilesFor(bounds, 8.0, 9.0) }
    private val expectedPaths by lazy { expectedTiles.map { "/${it.z}/${it.x}/${it.y}.png" }.toSet() }

    // A single Norgeskart raster lane pointed at OUR server — the DEM/vector
    // lanes are deliberately omitted so the requested set is fully controlled.
    private fun oneLane(@Suppress("UNUSED_PARAMETER") s: DownloadSpec): List<WgpuOfflineTileManager.Lane> =
        listOf(WgpuOfflineTileManager.Lane("norgeskart", "${server.url("/")}{z}/{x}/{y}.png", maxZoom = 18))

    private val http = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(10, TimeUnit.SECONDS)
        .build()

    // A real fetcher (same shape as production's okHttpFetcher) over the socket.
    private val fetcher: suspend (String) -> WgpuOfflineTileManager.FetchOutcome = { url ->
        http.newCall(Request.Builder().url(url).build()).execute().use { r ->
            when {
                r.code == 404 -> WgpuOfflineTileManager.FetchOutcome.Absent
                !r.isSuccessful -> WgpuOfflineTileManager.FetchOutcome.Error
                else -> {
                    val b = r.body?.bytes()
                    if (b == null || b.isEmpty()) WgpuOfflineTileManager.FetchOutcome.Absent
                    else WgpuOfflineTileManager.FetchOutcome.Data(b)
                }
            }
        }
    }

    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())

    @Before fun startServer() {
        server = MockWebServer()
        server.dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse {
                val path = request.path ?: ""
                requested.add(path)
                // Serve the known tile ONLY for in-region paths; 404 otherwise so a
                // stray request is observable both here and as a region failure.
                return if (path in expectedPaths) {
                    MockResponse().setResponseCode(200)
                        .addHeader("Content-Type", "image/png")
                        .setBody(Buffer().write(tileBody))
                } else {
                    MockResponse().setResponseCode(404)
                }
            }
        }
        server.start()
    }

    @After fun tearDown() {
        scope.cancel()
        server.shutdown()
    }

    private fun manager(store: TileStore, metaDir: java.io.File) = WgpuOfflineTileManager(
        tileStore = store,
        store = OfflineRegionStore(metaDir),
        serviceLauncher = OfflineServiceLauncher {},
        fetcher = fetcher,
        laneProvider = ::oneLane,
        scope = scope,
        now = System::currentTimeMillis,
    )

    private fun awaitTerminal(mgr: WgpuOfflineTileManager) = runBlocking {
        withTimeout(60_000) {
            mgr.regions.first { list ->
                list.singleOrNull()?.status.let { it == OfflineStatus.Complete || it == OfflineStatus.Failed }
            }.single()
        }
    }

    @Test
    fun requests_exactly_the_region_tiles_and_stores_the_served_bytes() {
        assertTrue("fixture must enumerate a non-trivial tile set", expectedPaths.size >= 2)
        val store = TileStore(tmp.newFolder("cache"))
        val mgr = manager(store, tmp.newFolder("meta"))

        mgr.download(spec())
        val region = awaitTerminal(mgr)

        assertEquals("region should complete", OfflineStatus.Complete, region.status)
        // THE fidelity assertion: exactly the region's tiles were requested —
        // every one, once, and nothing else.
        assertEquals("no duplicate requests", expectedPaths.size, requested.size)
        assertEquals("requested set == region tile set", expectedPaths, requested.toSet())
        // Each tile is stored as the exact bytes the server served.
        for (t in expectedTiles) {
            val stored = store.get("norgeskart", t.z, t.x, t.y)
            assertTrue("tile ${t.z}/${t.x}/${t.y} stored", stored != null)
            assertTrue("stored bytes == served bytes", stored!!.contentEquals(tileBody))
        }
        // Honest size accounting: count + total bytes reflect the known tiles.
        assertEquals(expectedTiles.size.toLong(), region.tileCount)
        assertEquals(expectedTiles.size.toLong() * tileBody.size, region.sizeBytes)
    }

    @Test
    fun evict_then_redownload_requests_the_same_controlled_set_again() {
        val store = TileStore(tmp.newFolder("cache"))
        val mgr = manager(store, tmp.newFolder("meta"))

        mgr.download(spec())
        val first = awaitTerminal(mgr)
        assertEquals(OfflineStatus.Complete, first.status)
        assertEquals("first download requested the exact set", expectedPaths, requested.toSet())

        // Evict: the region's tiles leave disk.
        mgr.delete(first.id)
        assertTrue("tiles gone after evict", expectedTiles.none { store.exists("norgeskart", it.z, it.x, it.y) })

        // Re-download from a clean request log: the same controlled set is fetched.
        requested.clear()
        mgr.download(spec())
        val second = awaitTerminal(mgr)
        assertEquals(OfflineStatus.Complete, second.status)
        assertEquals("re-download requested the same exact set", expectedPaths, requested.toSet())
        assertEquals("re-download re-fetched every tile (no stale skips)", expectedPaths.size, requested.size)
    }
}
