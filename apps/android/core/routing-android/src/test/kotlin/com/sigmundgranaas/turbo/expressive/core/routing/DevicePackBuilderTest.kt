package com.sigmundgranaas.turbo.expressive.core.routing

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.RoutingPack
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import uniffi.turbo_route_ffi.BuildBounds
import uniffi.turbo_route_ffi.HttpResponse
import uniffi.turbo_route_ffi.PackBuildProgress
import uniffi.turbo_route_ffi.PackBuildResult
import uniffi.turbo_route_ffi.PackHttp
import uniffi.turbo_route_ffi.RouteException
import java.io.File

/**
 * The builder around the FFI call — not the build itself, which is
 * minutes of live network and belongs on a device.
 *
 * What is worth pinning here is everything the Rust side cannot see:
 * where the pack lands, that a failed build leaves nothing behind, and
 * that the kommune list goes over empty so the region resolves itself.
 */
class DevicePackBuilderTest {

    @get:Rule
    val tmp = TemporaryFolder()

    private val bounds = GeoBounds(south = 67.45, west = 15.40, north = 67.56, east = 15.56)

    /** Never called — every test supplies its own build lambda. */
    private object NoHttp : PackHttp {
        override fun get(url: String): HttpResponse = error("not reached")
        override fun postJson(url: String, body: String): HttpResponse = error("not reached")
    }

    private fun result() = PackBuildResult(
        dir = "",
        demTiles = 24uL,
        nodes = 1100u,
        edges = 1898u,
        trails = 650u,
        roads = 300u,
        refusedCells = 5852uL,
        totalBytes = 3_400_000uL,
        seconds = 10.5,
    )

    /**
     * The pack has to land where [PackStore] looks, under the key the
     * downloader would have used. A build that succeeds into the wrong
     * directory is invisible, which is worse than failing.
     */
    @Test
    fun a_finished_pack_lands_under_the_downloader_key() = runTest {
        val root = tmp.newFolder()
        var gotDir: String? = null
        val b = DevicePackBuilder(
            root = root,
            http = NoHttp,
            io = StandardTestDispatcher(testScheduler),
            build = { dir, _, _, _, _ ->
                gotDir = dir
                // The real builder writes the manifest; stand in for it.
                File(dir).mkdirs()
                File(dir, RoutingPack.MANIFEST).writeText("[pack]\n")
                result()
            },
        )

        val out = b.build(bounds)

        val key = RoutingPack.keyFor(bounds)
        assertTrue("built into a .partial dir", gotDir!!.endsWith("$key.partial"))
        assertTrue(out is DevicePackBuilder.Outcome.Done)
        assertEquals(key, (out as DevicePackBuilder.Outcome.Done).key)
        assertTrue("renamed into place", File(root, "$key/${RoutingPack.MANIFEST}").isFile)
        assertFalse("no .partial left", File(root, "$key.partial").exists())
    }

    /**
     * An empty kommune list is the instruction to resolve them from the
     * bounds. Sending a non-empty one from here would silently pin every
     * device build to whatever list this file happened to hardcode.
     */
    @Test
    fun the_kommune_list_goes_over_empty() = runTest {
        var gotKommuner: List<String>? = null
        var gotBounds: BuildBounds? = null
        val b = DevicePackBuilder(
            root = tmp.newFolder(),
            http = NoHttp,
            io = StandardTestDispatcher(testScheduler),
            build = { dir, bb, k, _, _ ->
                gotKommuner = k
                gotBounds = bb
                File(dir).mkdirs()
                File(dir, RoutingPack.MANIFEST).writeText("[pack]\n")
                result()
            },
        )

        b.build(bounds)

        assertEquals(emptyList<String>(), gotKommuner)
        // west/south/east/north must not get transposed on the way in.
        assertEquals(15.40, gotBounds!!.minLon, 1e-9)
        assertEquals(67.45, gotBounds!!.minLat, 1e-9)
        assertEquals(15.56, gotBounds!!.maxLon, 1e-9)
        assertEquals(67.56, gotBounds!!.maxLat, 1e-9)
    }

    /**
     * A failed build must leave nothing. The half-written pack is the
     * dangerous artifact: it opens, claims coverage, and routes over
     * ground it has no trails for.
     */
    @Test
    fun a_failed_build_leaves_nothing_behind() = runTest {
        val root = tmp.newFolder()
        val b = DevicePackBuilder(
            root = root,
            http = NoHttp,
            io = StandardTestDispatcher(testScheduler),
            build = { dir, _, _, _, _ ->
                // Get far enough to have written something, then die.
                File(dir).mkdirs()
                File(dir, "norway.dem").writeText("half a terrain model")
                throw RouteException.Pack("WCS timed out")
            },
        )

        val out = b.build(bounds)

        assertTrue(out is DevicePackBuilder.Outcome.Failed)
        assertEquals("WCS timed out", (out as DevicePackBuilder.Outcome.Failed).reason)
        assertEquals("nothing left in the root", 0, root.listFiles()!!.size)
    }

    /** Cancelling is not failing, and it also leaves nothing. */
    @Test
    fun cancelling_is_reported_as_cancelled() = runTest {
        val root = tmp.newFolder()
        val b = DevicePackBuilder(
            root = root,
            http = NoHttp,
            io = StandardTestDispatcher(testScheduler),
            build = { dir, _, _, _, progress ->
                File(dir).mkdirs()
                // The host says stop by returning false; the Rust side
                // then unwinds as an error, which is what arrives here.
                progress.onProgress(uniffi.turbo_route_ffi.BuildPhase.TERRAIN, 3u, 24u)
                throw RouteException.Internal("cancelled")
            },
        )

        val out = b.build(bounds, isCancelled = { true })

        assertEquals(DevicePackBuilder.Outcome.Cancelled, out)
        assertEquals(0, root.listFiles()!!.size)
    }

    /** An existing pack is not rebuilt — minutes and megabytes saved. */
    @Test
    fun an_existing_pack_short_circuits() = runTest {
        val root = tmp.newFolder()
        val key = RoutingPack.keyFor(bounds)
        File(root, key).mkdirs()
        File(root, "$key/${RoutingPack.MANIFEST}").writeText("[pack]\n")
        var built = false
        val b = DevicePackBuilder(
            root = root,
            http = NoHttp,
            io = StandardTestDispatcher(testScheduler),
            build = { _, _, _, _, _ -> built = true; result() },
        )

        val out = b.build(bounds)

        assertTrue(out is DevicePackBuilder.Outcome.Done)
        assertFalse("must not rebuild", built)
    }

    /**
     * The size cap is checked before the network, not after. Learning
     * at the end of a multi-minute build that it was never going to fit
     * is the worst possible moment to find out.
     */
    @Test
    fun an_oversized_region_is_refused_before_building() = runTest {
        var built = false
        val huge = GeoBounds(south = 58.0, west = 5.0, north = 71.0, east = 31.0)
        val b = DevicePackBuilder(
            root = tmp.newFolder(),
            http = NoHttp,
            io = StandardTestDispatcher(testScheduler),
            build = { _, _, _, _, _ -> built = true; result() },
        )

        val out = b.build(huge)

        assertTrue("$out", out is DevicePackBuilder.Outcome.TooLarge)
        assertFalse("must not touch the network", built)
    }
}
