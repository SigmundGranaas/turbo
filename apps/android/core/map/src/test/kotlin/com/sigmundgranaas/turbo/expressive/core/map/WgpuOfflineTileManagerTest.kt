package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.core.turbomap.android.TileStore
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

@OptIn(ExperimentalCoroutinesApi::class)
class WgpuOfflineTileManagerTest {

    @get:Rule val tmp = TemporaryFolder()

    private val bounds = GeoBounds(south = 67.25, west = 15.00, north = 67.27, east = 15.04)
    private fun spec(name: String = "Sjunkhatten", b: GeoBounds = bounds, min: Double = 12.0, max: Double = 13.0) =
        DownloadSpec(name = name, base = BaseLayer.Norgeskart, bounds = b, minZoom = min, maxZoom = max)

    private val oneLane: (DownloadSpec) -> List<WgpuOfflineTileManager.Lane> =
        { listOf(WgpuOfflineTileManager.Lane("norgeskart", "https://example/{z}/{x}/{y}.png", 18)) }

    private fun manager(
        scope: CoroutineScope,
        cacheDir: File,
        metaDir: File,
        fetcher: suspend (String) -> ByteArray? = { ByteArray(64) { 7 } },
        launcher: OfflineServiceLauncher = OfflineServiceLauncher {},
    ) = WgpuOfflineTileManager(
        tileStore = TileStore(cacheDir),
        store = OfflineRegionStore(metaDir),
        serviceLauncher = launcher,
        fetcher = fetcher,
        laneProvider = oneLane,
        scope = scope,
        now = { 1_000L },
    )

    @Test
    fun `download fetches every tile into the shared store and persists the region`() = runTest(UnconfinedTestDispatcher()) {
        val cache = tmp.newFolder("cache")
        val meta = tmp.newFolder("meta")
        val store = TileStore(cache)
        val mgr = manager(this, cache, meta)

        mgr.download(spec())
        advanceUntilIdle()

        val r = mgr.regions.value.single()
        assertEquals(OfflineStatus.Complete, r.status)
        assertEquals(1f, r.progress)
        val tiles = TileMath.tilesFor(bounds, 12.0, 13.0)
        assertTrue("every region tile is on disk", tiles.all { store.exists("norgeskart", it.z, it.x, it.y) })
        assertEquals(tiles.size.toLong(), r.tileCount)

        // Survives relaunch: a fresh manager reads the persisted region back.
        assertEquals(OfflineStatus.Complete, OfflineRegionStore(meta).loadAll().single().status)
    }

    @Test
    fun `a tile that never fetches marks the region failed`() = runTest(UnconfinedTestDispatcher()) {
        val cache = tmp.newFolder("cache")
        val meta = tmp.newFolder("meta")
        val mgr = manager(this, cache, meta, fetcher = { null })

        mgr.download(spec())
        advanceUntilIdle()

        assertEquals(OfflineStatus.Failed, mgr.regions.value.single().status)
    }

    @Test
    fun `an over-large area fails immediately without touching the network`() = runTest(UnconfinedTestDispatcher()) {
        var fetched = false
        val mgr = manager(
            this,
            tmp.newFolder("cache"),
            tmp.newFolder("meta"),
            fetcher = { fetched = true; null },
        )
        val huge = GeoBounds(south = 0.0, west = 0.0, north = 50.0, east = 50.0)
        mgr.download(spec(b = huge, min = 8.0, max = 14.0))
        advanceUntilIdle()

        val r = mgr.regions.value.single()
        assertEquals(OfflineStatus.Failed, r.status)
        assertEquals("Area too large", r.errorReason)
        assertFalse("no tiles should be fetched for a rejected area", fetched)
    }

    @Test
    fun `delete removes the region and frees its tiles`() = runTest(UnconfinedTestDispatcher()) {
        val cache = tmp.newFolder("cache")
        val meta = tmp.newFolder("meta")
        val store = TileStore(cache)
        val mgr = manager(this, cache, meta)

        mgr.download(spec())
        advanceUntilIdle()
        val id = mgr.regions.value.single().id

        mgr.delete(id)
        advanceUntilIdle()

        assertTrue("region list is empty", mgr.regions.value.isEmpty())
        assertTrue("metadata is gone", OfflineRegionStore(meta).loadAll().isEmpty())
        val tiles = TileMath.tilesFor(bounds, 12.0, 13.0)
        assertTrue("tiles are freed", tiles.none { store.exists("norgeskart", it.z, it.x, it.y) })
    }
}
