package com.sigmundgranaas.turbo.expressive.core.routing

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.RoutingPack
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.Job
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import uniffi.turbo_route_ffi.BuildBounds
import uniffi.turbo_route_ffi.BuildPhase
import uniffi.turbo_route_ffi.HttpResponse
import uniffi.turbo_route_ffi.PackBuildProgress
import uniffi.turbo_route_ffi.PackBuildResult
import uniffi.turbo_route_ffi.PackHttp
import uniffi.turbo_route_ffi.RouteException
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

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
            newHttp = { NoHttp },
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
            newHttp = { NoHttp },
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
            newHttp = { NoHttp },
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
            newHttp = { NoHttp },
            io = StandardTestDispatcher(testScheduler),
            build = { dir, _, _, _, progress ->
                File(dir).mkdirs()
                // The host says stop by returning false; the Rust side
                // then unwinds as an error, which is what arrives here.
                progress.onProgress(BuildPhase.TERRAIN, 3u, 24u)
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
            newHttp = { NoHttp },
            io = StandardTestDispatcher(testScheduler),
            build = { _, _, _, _, _ -> built = true; result() },
        )

        val out = b.build(bounds)

        assertTrue(out is DevicePackBuilder.Outcome.Done)
        assertFalse("must not rebuild", built)
    }

    /**
     * Cancelling the coroutine must stop the build.
     *
     * `buildPack` is a blocking native call, so cancellation cannot
     * interrupt it — the only lever is returning false from the progress
     * callback. Without that wiring a paused download keeps fetching
     * from Kartverket for minutes, on whatever connection the user is
     * on, after they explicitly said stop.
     */
    @Test
    fun a_cancelled_coroutine_stops_the_build() = runTest {
        var keptGoing: Boolean? = null
        var job: Job? = null
        val b = DevicePackBuilder(
            root = tmp.newFolder(),
            newHttp = { NoHttp },
            io = StandardTestDispatcher(testScheduler),
            build = { dir, _, _, _, progress ->
                File(dir).mkdirs()
                // Cancel mid-build, exactly as pause() does: it cancels
                // the job while the native call is already running.
                job!!.cancel()
                keptGoing = progress.onProgress(BuildPhase.TERRAIN, 1u, 24u)
                throw RouteException.Internal("cancelled")
            },
        )

        job = launch { b.build(bounds) }
        testScheduler.advanceUntilIdle()

        assertEquals("the callback must say stop", false, keptGoing)
    }

    /**
     * Two builds of the same region must not overlap.
     *
     * `buildPack` is blocking native code, so cancelling a download does
     * not stop a build already inside it. A retry, a resume, or the
     * network gate flipping back on therefore starts a second build
     * while the first is still running — and both derive the same key,
     * so the second one's `deleteRecursively()` removes the directory
     * the first is writing into. The first then dies on its next file
     * operation with a bare "No such file or directory (os error 2)".
     *
     * Real threads and latches, not a test dispatcher: the thing under
     * test is mutual exclusion between two blocking calls, and a
     * single-threaded scheduler cannot interleave them at all — a
     * version of this test written that way passed against the bug.
     */
    @Test
    fun a_second_build_of_the_same_region_waits_instead_of_deleting_the_first() = runBlocking {
        val root = tmp.newFolder()
        val inside = CountDownLatch(1)
        val release = CountDownLatch(1)
        val concurrent = AtomicInteger(0)
        val overlapped = AtomicBoolean(false)
        val vanished = AtomicBoolean(false)
        val builds = AtomicInteger(0)
        val b = DevicePackBuilder(
            root = root,
            newHttp = { NoHttp },
            io = Dispatchers.IO,
            build = { dir, _, _, _, _ ->
                builds.incrementAndGet()
                if (concurrent.incrementAndGet() > 1) overlapped.set(true)
                File(dir).mkdirs()
                File(dir, "norway.dem").writeText("terrain")
                inside.countDown()
                release.await(10, TimeUnit.SECONDS)
                // Whoever else ran would have wiped this out from under us.
                if (!File(dir, "norway.dem").isFile) vanished.set(true)
                File(dir, RoutingPack.MANIFEST).writeText("[pack]\n")
                concurrent.decrementAndGet()
                result()
            },
        )

        val first = launch(Dispatchers.IO) { b.build(bounds) }
        assertTrue("the first build must get going", inside.await(10, TimeUnit.SECONDS))
        val second = launch(Dispatchers.IO) { b.build(bounds) }
        // Long enough for an unguarded second build to reach its
        // deleteRecursively, which is the very first thing it does.
        Thread.sleep(300)
        release.countDown()
        first.join()
        second.join()

        assertFalse("the two builds must not overlap", overlapped.get())
        assertFalse("neither build may lose its working directory", vanished.get())
        // The loser finds the pack already there and returns it.
        assertEquals("built once, not twice", 1, builds.get())
        val key = RoutingPack.keyFor(bounds)
        assertTrue("the pack is in place", File(root, "$key/${RoutingPack.MANIFEST}").isFile)
        assertFalse("no .partial left", File(root, "$key.partial").exists())
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
            newHttp = { NoHttp },
            io = StandardTestDispatcher(testScheduler),
            build = { _, _, _, _, _ -> built = true; result() },
        )

        val out = b.build(huge)

        assertTrue("$out", out is DevicePackBuilder.Outcome.TooLarge)
        assertFalse("must not touch the network", built)
    }

    /**
     * A [PackHttp] that blocks the way a socket read does, and stops
     * only when something aborts it.
     *
     * The real client waits five minutes for a response, because a cold
     * WCS coverage genuinely takes that long. A fake that returns
     * promptly would make the test pass whether or not cancellation
     * reaches the request — which is the whole question.
     */
    private class BlockingHttp : PackHttp, AbortableHttp {
        val entered = CountDownLatch(1)
        val abortSeen = CountDownLatch(1)
        private val released = CountDownLatch(1)
        private val cancelled = AtomicBoolean(false)

        override fun get(url: String): HttpResponse {
            entered.countDown()
            released.await()
            if (cancelled.get()) throw RouteException.Pack("$url: cancelled")
            return HttpResponse(status = 200u, body = ByteArray(0))
        }

        override fun postJson(url: String, body: String): HttpResponse = get(url)

        override fun abortInFlight() {
            forceRelease()
            abortSeen.countDown()
        }

        /**
         * Unpark the blocked thread no matter what the test found.
         *
         * A thread sitting in `await()` is not reachable by coroutine
         * cancellation, so without this a failing run hangs instead of
         * failing — which is exactly what the first draft did, and what
         * makes a red test useless to whoever hits it.
         */
        fun forceRelease() {
            cancelled.set(true)
            released.countDown()
        }
    }

    /**
     * Cancellation is checked between units of work, and a unit of work
     * is an HTTP request this client waits minutes for. Blocked on a
     * socket — which is most of a build's life — "stops at the next
     * boundary" meant "stops in up to five minutes". Cancelling the
     * request makes the boundary arrive now.
     *
     * Real dispatchers and real threads on purpose: the thing under test
     * is one thread parked in a blocking call while another cancels it,
     * which a test dispatcher cannot represent.
     */
    @Test
    fun cancelling_a_build_aborts_the_request_it_is_blocked_on() = runBlocking {
        val http = BlockingHttp()
        val builder = DevicePackBuilder(
            root = tmp.newFolder(),
            newHttp = { http },
            io = Dispatchers.IO,
            build = { _, _, _, client, _ ->
                // Stands in for the Rust side fetching: blocks until the
                // request is aborted, then surfaces the failure.
                client.get("https://example/wcs")
                error("the aborted request should not have returned")
            },
        )

        var reachedRequest = false
        var aborted = false
        val job = launch(Dispatchers.Default) { builder.build(bounds) }
        try {
            reachedRequest = http.entered.await(10, TimeUnit.SECONDS)
            if (reachedRequest) {
                job.cancel()
                aborted = http.abortSeen.await(10, TimeUnit.SECONDS)
            }
        } finally {
            // Before any assertion, so a failure unwinds instead of
            // parking the test on a latch forever.
            http.forceRelease()
            job.cancel()
            job.join()
        }

        assertTrue("the build never reached its first request", reachedRequest)
        assertTrue("cancelling the build left the request in flight", aborted)
    }

    /**
     * The FFI reports what it built; the builder used to discard it and
     * re-walk the directory for a byte count the report already carried.
     */
    @Test
    fun a_finished_build_carries_the_reports_figures() = runTest {
        val builder = DevicePackBuilder(
            root = tmp.newFolder(),
            newHttp = { NoHttp },
            io = StandardTestDispatcher(testScheduler),
            build = { dir, _, _, _, _ ->
                File(dir, RoutingPack.MANIFEST).writeText("built = true")
                result()
            },
        )

        val outcome = builder.build(bounds)

        val done = outcome as DevicePackBuilder.Outcome.Done
        assertEquals(3_400_000L, done.bytes)
        val stats = requireNotNull(done.stats) { "a fresh build must carry its stats" }
        assertEquals(1100u, stats.nodes)
        assertEquals(1898u, stats.edges)
        assertEquals(650u, stats.trails)
        assertEquals(24uL, stats.demTiles)
        assertEquals(10.5, stats.seconds, 0.0001)
    }
}
