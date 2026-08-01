package com.sigmundgranaas.turbo.expressive.core.routing

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.RoutingPack
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlin.coroutines.coroutineContext
import uniffi.turbo_route_ffi.BuildBounds
import uniffi.turbo_route_ffi.BuildPhase
import uniffi.turbo_route_ffi.PackBuildProgress
import uniffi.turbo_route_ffi.PackBuildResult
import uniffi.turbo_route_ffi.PackHttp
import uniffi.turbo_route_ffi.RouteException
import uniffi.turbo_route_ffi.buildPack
import java.io.File
import kotlin.coroutines.cancellation.CancellationException

/**
 * Cuts a routing pack on the phone, from Kartverket directly.
 *
 * The counterpart to [com.sigmundgranaas.turbo.expressive.core.map.PackDownloader],
 * for the case it cannot serve: a region no server has prepared. The
 * downloader fetches a pack somebody else built; this builds one, out
 * of the same public services the tileserver uses.
 *
 * # It writes where the reader looks
 *
 * Into `filesDir/routing-packs/<key>/`, keyed by
 * [RoutingPack.keyFor] — the same root and the same key the downloader
 * uses, because [PackStore] scans exactly that directory. A pack that
 * landed anywhere else would build successfully and never be found.
 *
 * # Atomic, for the same reason as the downloader
 *
 * Built into `<key>.partial` and renamed. A half-built pack is the
 * dangerous artifact here: it has a DEM and no graph, so it opens,
 * reports coverage, and then routes cross-country over ground it has no
 * trails for. A reader sees a whole pack or none.
 */
class DevicePackBuilder(
    private val root: File,
    private val http: PackHttp,
    private val io: CoroutineDispatcher = Dispatchers.IO,
    /**
     * Seam for tests. The real one is minutes of network against live
     * public services, which is not something a unit test may do.
     */
    private val build: (String, BuildBounds, List<String>, PackHttp, PackBuildProgress) -> PackBuildResult =
        { dir, bounds, kommuner, client, progress ->
            buildPack(
                outDir = dir,
                bounds = bounds,
                haloM = HALO_M,
                kommuner = kommuner,
                http = client,
                progress = progress,
            )
        },
) {

    sealed interface Outcome {
        data class Done(val key: String, val dir: File, val bytes: Long) : Outcome

        /** The user backed out. Nothing was left on disk. */
        data object Cancelled : Outcome

        /**
         * Too big to build here.
         *
         * The same cap the downloader applies, checked before the
         * network rather than after: a device build is minutes long, and
         * discovering the region was never buildable at the end of it is
         * the worst possible time to say so.
         */
        data class TooLarge(val areaSqKm: Double) : Outcome

        data class Failed(val reason: String) : Outcome
    }

    /**
     * Build the pack covering [bounds].
     *
     * [onProgress] gets the phase and a 0..1 fraction. [isCancelled] is
     * polled between units of work — a build stops after the request in
     * flight, not in the middle of one, because a torn artifact is the
     * thing this is all arranged to avoid.
     */
    suspend fun build(
        bounds: GeoBounds,
        onProgress: (BuildPhase, Float) -> Unit = { _, _ -> },
        isCancelled: () -> Boolean = { false },
    ): Outcome = withContext(io) {
        // Cancelling the coroutine is not enough on its own. `buildPack`
        // is a blocking call into native code: cancellation cannot
        // interrupt it, it only takes effect when the call returns. A
        // build left to run would keep fetching from Kartverket for
        // minutes after the user hit pause — on mobile data, if that is
        // what they are on, and after they explicitly said stop.
        //
        // So the coroutine's liveness is folded into the same signal the
        // host already had: returning false from the progress callback.
        val job = coroutineContext[kotlinx.coroutines.Job]
        val stop = { job?.isActive == false || isCancelled() }
        val key = RoutingPack.keyFor(bounds)
        val done = File(root, key)
        if (File(done, RoutingPack.MANIFEST).isFile) {
            return@withContext Outcome.Done(key, done, done.sizeOnDisk())
        }
        if (!RoutingPack.fitsOnePack(bounds)) {
            return@withContext Outcome.TooLarge(RoutingPack.areaSqKm(bounds))
        }

        val partial = File(root, "$key.partial")
        partial.deleteRecursively()
        partial.mkdirs()

        val progress = object : PackBuildProgress {
            override fun onProgress(phase: BuildPhase, done: UInt, total: UInt): Boolean {
                if (stop()) return false
                // `total` is 0 at the start of a phase whose size is not
                // known yet. Reporting 0/0 as a fraction is a division
                // by zero; reporting it as "no movement" is honest.
                val f = if (total > 0u) done.toFloat() / total.toFloat() else 0f
                onProgress(phase, f.coerceIn(0f, 1f))
                return true
            }
        }

        try {
            build(
                partial.absolutePath,
                BuildBounds(
                    minLon = bounds.west,
                    minLat = bounds.south,
                    maxLon = bounds.east,
                    maxLat = bounds.north,
                ),
                // Empty means "work out which kommuner this region
                // covers". Nobody drawing a box on a map knows that N50
                // is ordered per kommune, and a list that is short by
                // one produces a pack whose water mask simply stops.
                emptyList(),
                http,
                progress,
            )
        } catch (e: CancellationException) {
            partial.deleteRecursively()
            throw e
        } catch (e: RouteException) {
            partial.deleteRecursively()
            // Cancellation arrives as an error across the FFI — the
            // build has no other way to report that it stopped early —
            // so it is separated back out here rather than shown to the
            // user as a failure they might retry.
            return@withContext if (stop()) {
                Outcome.Cancelled
            } else {
                Outcome.Failed(e.message ?: e::class.java.simpleName)
            }
        } catch (e: Exception) {
            partial.deleteRecursively()
            return@withContext Outcome.Failed(e.message ?: e::class.java.simpleName)
        }

        done.deleteRecursively()
        if (!partial.renameTo(done)) {
            partial.deleteRecursively()
            return@withContext Outcome.Failed("could not move the finished pack into place")
        }
        Outcome.Done(key, done, done.sizeOnDisk())
    }

    private fun File.sizeOnDisk(): Long =
        walkTopDown().filter { it.isFile }.sumOf { it.length() }

    private companion object {
        /**
         * Matches what the tileserver cuts.
         *
         * A pack with no halo cannot route near its own edge: the solver
         * needs terrain slightly outside the region to evaluate a step
         * that leaves it, and without it every route hugs the boundary.
         */
        const val HALO_M = 2000.0
    }
}
