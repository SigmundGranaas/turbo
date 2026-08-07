package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.core.map.WgpuOfflineTileManager.FetchOutcome
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
        fetcher: suspend (String) -> FetchOutcome = { FetchOutcome.Data(ByteArray(64) { 7 }) },
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
    fun `a region where every tile errors is a failed region`() = runTest(UnconfinedTestDispatcher()) {
        val cache = tmp.newFolder("cache")
        val meta = tmp.newFolder("meta")
        val mgr = manager(this, cache, meta, fetcher = { FetchOutcome.Error })

        mgr.download(spec())
        advanceUntilIdle()

        assertEquals(OfflineStatus.Failed, mgr.regions.value.single().status)
    }

    /** The tile pyramid for [wide], and the URLs the manager will ask for. */
    private val wide = GeoBounds(south = 67.20, west = 15.00, north = 67.40, east = 15.40)
    private fun wideUrls(): List<String> =
        TileMath.tilesFor(wide, 10.0, 14.0).map { "https://example/${it.z}/${it.x}/${it.y}.png" }

    /**
     * A handful of tiles lost to a busy server is not a failed download.
     *
     * This is the one that shipped wrong: a single errored tile failed the
     * whole region, so a 2000-tile area over a rate-limiting public WMTS
     * got to ninety-odd percent and then presented every tile it *had*
     * fetched as a failure. The tiles are on disk and the area is usable.
     */
    @Test
    fun `a few failed tiles still complete the region, with a note`() = runTest(UnconfinedTestDispatcher()) {
        val cache = tmp.newFolder("cache")
        val meta = tmp.newFolder("meta")
        val urls = wideUrls()
        assertTrue("need a pyramid big enough for 2% to be >0 tiles", urls.size >= 100)
        val doomed = urls.take(urls.size / 50).toSet() // 2%, under the 5% bar
        assertTrue(doomed.isNotEmpty())
        val mgr = manager(this, cache, meta, fetcher = { url ->
            if (url in doomed) FetchOutcome.Error else FetchOutcome.Data(ByteArray(64) { 7 })
        })

        mgr.download(spec(b = wide, min = 10.0, max = 14.0))
        advanceUntilIdle()

        val r = mgr.regions.value.single()
        assertEquals(OfflineStatus.Complete, r.status)
        assertEquals(1f, r.progress)
        assertEquals((urls.size - doomed.size).toLong(), r.tileCount)
        // Complete, but it must not pretend to be whole — the note is what
        // the screen turns into "tap to fill the gaps".
        assertTrue("says what is missing: ${r.errorReason}", r.errorReason!!.contains("${doomed.size} of"))
    }

    /** Past the bar it is a failure again — an area this patchy is not covered. */
    @Test
    fun `losing a large share of the tiles still fails the region`() = runTest(UnconfinedTestDispatcher()) {
        val cache = tmp.newFolder("cache")
        val meta = tmp.newFolder("meta")
        val urls = wideUrls()
        val doomed = urls.take(urls.size / 5).toSet() // 20%, well past the bar
        val mgr = manager(this, cache, meta, fetcher = { url ->
            if (url in doomed) FetchOutcome.Error else FetchOutcome.Data(ByteArray(64) { 7 })
        })

        mgr.download(spec(b = wide, min = 10.0, max = 14.0))
        advanceUntilIdle()

        assertEquals(OfflineStatus.Failed, mgr.regions.value.single().status)
    }

    /** A clean download carries no note — the gap line must not always show. */
    @Test
    fun `a complete region has no missing-tile note`() = runTest(UnconfinedTestDispatcher()) {
        val cache = tmp.newFolder("cache")
        val meta = tmp.newFolder("meta")
        val mgr = manager(this, cache, meta)

        mgr.download(spec())
        advanceUntilIdle()

        assertEquals(null, mgr.regions.value.single().errorReason)
    }

    @Test
    fun `absent tiles (no data here) still complete the region`() = runTest(UnconfinedTestDispatcher()) {
        val cache = tmp.newFolder("cache")
        val meta = tmp.newFolder("meta")
        val store = TileStore(cache)
        // A water lane over land / DEM over sea: the server has no tile here. That's
        // not a failure — the region completes with zero stored tiles.
        val mgr = manager(this, cache, meta, fetcher = { FetchOutcome.Absent })

        mgr.download(spec())
        advanceUntilIdle()

        val r = mgr.regions.value.single()
        assertEquals(OfflineStatus.Complete, r.status)
        assertEquals(0L, r.tileCount)
        val tiles = TileMath.tilesFor(bounds, 12.0, 13.0)
        assertTrue("nothing stored for absent tiles", tiles.none { store.exists("norgeskart", it.z, it.x, it.y) })
    }

    @Test
    fun `an over-large area fails immediately without touching the network`() = runTest(UnconfinedTestDispatcher()) {
        var fetched = false
        val mgr = manager(
            this,
            tmp.newFolder("cache"),
            tmp.newFolder("meta"),
            fetcher = { fetched = true; FetchOutcome.Error },
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

    /**
     * The one that would have caught three separate bugs.
     *
     * `Job.cancel()` returns before the coroutine stops, and a device
     * pack build has no suspension point for minutes — it is a blocking
     * call into native code. So `pause` followed by `resume` (or a retry,
     * or Wi-Fi returning) used to launch a second job for the region
     * while the first was still running: two builds on one directory, the
     * second deleting the first's working files, the first dying on a
     * bare ENOENT.
     *
     * The build seam here parks under [NonCancellable] precisely because
     * that is what the real one does — cancelling it changes nothing
     * until it returns on its own. A test whose fake build stops politely
     * on cancellation would pass against the bug.
     */
    @Test
    fun `restarting a region waits for the job it replaces instead of running two`() =
        runTest(UnconfinedTestDispatcher()) {
            val cache = tmp.newFolder("cache")
            val meta = tmp.newFolder("meta")

            val inside = java.util.concurrent.atomic.AtomicInteger(0)
            val mostAtOnce = java.util.concurrent.atomic.AtomicInteger(0)
            val firstIsIn = kotlinx.coroutines.CompletableDeferred<Unit>()
            val release = kotlinx.coroutines.CompletableDeferred<Unit>()

            val mgr = WgpuOfflineTileManager(
                tileStore = TileStore(cache),
                store = OfflineRegionStore(meta),
                serviceLauncher = OfflineServiceLauncher {},
                fetcher = { FetchOutcome.Data(ByteArray(8)) },
                laneProvider = oneLane,
                scope = this,
                now = { 1_000L },
                // 404 on the manifest is what "this server has no pack
                // for that region" looks like, and it is the branch that
                // hands over to the device build.
                packs = PackDownloader(
                    root = tmp.newFolder("packs"),
                    source = { "https://example/packs" },
                    fetch = { _, _ -> PackDownloader.FetchResult.NotFound },
                ),
                deviceBuild = { _, _ ->
                    val n = inside.incrementAndGet()
                    mostAtOnce.updateAndGet { maxOf(it, n) }
                    firstIsIn.complete(Unit)
                    try {
                        // Blocking native code: cancellation does not
                        // reach in here, it only takes effect on return.
                        kotlinx.coroutines.withContext(kotlinx.coroutines.NonCancellable) {
                            release.await()
                        }
                        WgpuOfflineTileManager.DeviceBuildResult.Done(1L)
                    } finally {
                        inside.decrementAndGet()
                    }
                },
            )

            // Released in a finally, and asserted only afterwards. A
            // build parked under NonCancellable keeps `runTest` from
            // completing, so asserting while one is held turns a failure
            // into a hang — which is what this test did on its first
            // draft, and a guard that hangs instead of failing tells
            // whoever hits it nothing.
            val peakWhileFirstWasHeld: Int
            try {
                mgr.download(spec())
                firstIsIn.await()
                val id = mgr.regions.value.single().id

                // The user pauses and immediately resumes, while the
                // first build is still inside the FFI and unstoppable.
                mgr.pause(id)
                mgr.resume(id)
                advanceUntilIdle()
                peakWhileFirstWasHeld = mostAtOnce.get()
            } finally {
                release.complete(Unit)
                advanceUntilIdle()
            }

            assertEquals(
                "a second build ran while the first was still inside the FFI",
                1,
                peakWhileFirstWasHeld,
            )
            assertEquals("builds overlapped at some point", 1, mostAtOnce.get())
        }
}
