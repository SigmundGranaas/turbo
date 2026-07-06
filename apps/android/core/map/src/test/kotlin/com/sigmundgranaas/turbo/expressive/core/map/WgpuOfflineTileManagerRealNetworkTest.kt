package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.core.turbomap.android.TileStore
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import com.sigmundgranaas.turbo.expressive.ui.map.MapStyles
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import okhttp3.OkHttpClient
import okhttp3.Request
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.util.concurrent.TimeUnit

/**
 * REAL-NETWORK integration test: downloads actual Kartverket topo tiles for the
 * **Sjunkhatten** region through the production [WgpuOfflineTileManager] path
 * and validates the download UX end to end — real bytes on disk, real size
 * accounting, region persistence, and download → evict → re-download.
 *
 * This is the counterpart to the synthetic-tile gates (the JVM
 * [WgpuOfflineTileManagerTest] with a fake fetcher, and the on-device
 * airplane-mode render gate): those prove the *mechanism* with fabricated
 * bytes; this proves the manager fetches and stores the *real* national map.
 *
 * OPT-IN so it never flakes the per-push/PR gate on Kartverket's uptime:
 * skipped unless `TURBO_REAL_TILES=1` is in the environment. Run it via the
 * `android_offline_realdata` dispatch workflow, or locally:
 *
 *     TURBO_REAL_TILES=1 ./gradlew :core:map:testDebugUnitTest \
 *         --tests '*WgpuOfflineTileManagerRealNetworkTest*' --rerun-tasks
 *
 * It uses the exact production URL (`MapStyles.turbomapRasterSpecs`) and a real
 * OkHttp client — no test doubles on the fetch path. Only the base raster lane
 * is exercised (the DEM/vector lanes point at a self-hosted origin that may not
 * be provisioned); a tiny z8–9 Sjunkhatten box keeps it to a handful of tiles.
 */
class WgpuOfflineTileManagerRealNetworkTest {

    @get:Rule val tmp = TemporaryFolder()

    // Sjunkhatten National Park (near Bodø) — the same small box the synthetic
    // WgpuOfflineTileManagerTest fixture uses, so the two tell one story.
    private val sjunkhatten = GeoBounds(south = 67.25, west = 15.00, north = 67.27, east = 15.04)

    private fun spec() = DownloadSpec(
        name = "Sjunkhatten",
        base = BaseLayer.Norgeskart,
        bounds = sjunkhatten,
        minZoom = 8.0,
        maxZoom = 9.0,
    )

    // The REAL production base-raster lane: exact Kartverket topo URL + native
    // max zoom, straight from MapStyles (the same knowledge the live map uses).
    private fun kartverketBaseLane(spec: DownloadSpec): List<WgpuOfflineTileManager.Lane> =
        MapStyles.turbomapRasterSpecs(spec.base)
            .filter { it.id == "norgeskart" }
            .map { WgpuOfflineTileManager.Lane(it.id, it.tileUrlTemplate, it.maxZoom) }

    // A real OkHttp fetcher (mirrors the manager's private okHttpFetcher): GET
    // the tile, map to a FetchOutcome. No mocking — this hits the network.
    private val http = OkHttpClient.Builder()
        .connectTimeout(20, TimeUnit.SECONDS)
        .readTimeout(20, TimeUnit.SECONDS)
        .build()

    private val realFetcher: suspend (String) -> WgpuOfflineTileManager.FetchOutcome = { url ->
        http.newCall(Request.Builder().url(url).header("User-Agent", "turbomap-realnet-test/1.0").build())
            .execute()
            .use { r ->
                when {
                    r.code == 404 || r.code == 204 || r.code == 410 -> WgpuOfflineTileManager.FetchOutcome.Absent
                    !r.isSuccessful -> WgpuOfflineTileManager.FetchOutcome.Error
                    else -> {
                        val bytes = r.body?.bytes()
                        when {
                            bytes == null || bytes.isEmpty() -> WgpuOfflineTileManager.FetchOutcome.Absent
                            else -> WgpuOfflineTileManager.FetchOutcome.Data(bytes)
                        }
                    }
                }
            }
    }

    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())

    @After fun tearDown() = scope.cancel()

    private fun manager(store: TileStore, metaDir: java.io.File) = WgpuOfflineTileManager(
        tileStore = store,
        store = OfflineRegionStore(metaDir),
        serviceLauncher = OfflineServiceLauncher {},
        fetcher = realFetcher,
        laneProvider = ::kartverketBaseLane,
        scope = scope,
        now = System::currentTimeMillis,
    )

    /** Block until the (single) region reaches a terminal status, or time out. */
    private fun awaitTerminal(mgr: WgpuOfflineTileManager) = runBlocking {
        withTimeout(120_000) {
            mgr.regions.first { list ->
                list.singleOrNull()?.status.let { it == OfflineStatus.Complete || it == OfflineStatus.Failed }
            }.single()
        }
    }

    @Test
    fun downloads_real_kartverket_sjunkhatten_tiles_into_the_store() {
        assumeTrue(
            "real-network test — set TURBO_REAL_TILES=1 to hit cache.kartverket.no",
            System.getenv("TURBO_REAL_TILES") == "1",
        )
        val cache = tmp.newFolder("cache")
        val store = TileStore(cache)
        val mgr = manager(store, tmp.newFolder("meta"))
        val tiles = TileMath.tilesFor(sjunkhatten, 8.0, 9.0)
        assertTrue("fixture should enumerate at least one tile", tiles.isNotEmpty())

        mgr.download(spec())
        val region = awaitTerminal(mgr)

        assertEquals("Kartverket download should complete", OfflineStatus.Complete, region.status)
        // Every enumerated tile is on disk as a REAL PNG (magic bytes), not a
        // fabricated blob — this is what distinguishes it from the synthetic gate.
        var onDiskBytes = 0L
        for (t in tiles) {
            val bytes = store.get("norgeskart", t.z, t.x, t.y)
            assertTrue("tile ${t.z}/${t.x}/${t.y} must be stored", bytes != null)
            requireNotNull(bytes)
            assertTrue("tile must be a real PNG (got ${bytes.size} B)", bytes.size > 200 && isPng(bytes))
            onDiskBytes += bytes.size
        }
        // Real size accounting: the region's reported size reflects real bytes.
        assertEquals("tileCount matches the enumerated set", tiles.size.toLong(), region.tileCount)
        assertTrue("sizeBytes reflects real tiles (${region.sizeBytes} B)", region.sizeBytes > 0L)
    }

    @Test
    fun download_then_evict_then_redownload_restores_the_region() {
        assumeTrue(
            "real-network test — set TURBO_REAL_TILES=1 to hit cache.kartverket.no",
            System.getenv("TURBO_REAL_TILES") == "1",
        )
        val cache = tmp.newFolder("cache")
        val store = TileStore(cache)
        val mgr = manager(store, tmp.newFolder("meta"))
        val tiles = TileMath.tilesFor(sjunkhatten, 8.0, 9.0)

        // Download.
        mgr.download(spec())
        val first = awaitTerminal(mgr)
        assertEquals(OfflineStatus.Complete, first.status)
        assertTrue("tiles present after download", tiles.all { store.exists("norgeskart", it.z, it.x, it.y) })

        // Evict: delete the region → its (unshared) tiles leave disk.
        mgr.delete(first.id)
        assertTrue("region removed", mgr.regions.value.none { it.id == first.id })
        assertTrue("tiles gone after evict", tiles.none { store.exists("norgeskart", it.z, it.x, it.y) })

        // Re-download: a fresh real fetch restores them.
        mgr.download(spec())
        val second = awaitTerminal(mgr)
        assertEquals(OfflineStatus.Complete, second.status)
        assertTrue("tiles restored after re-download", tiles.all { store.exists("norgeskart", it.z, it.x, it.y) })
    }

    private fun isPng(b: ByteArray): Boolean =
        b.size >= 8 && b[0] == 0x89.toByte() && b[1] == 0x50.toByte() &&
            b[2] == 0x4E.toByte() && b[3] == 0x47.toByte()
}
