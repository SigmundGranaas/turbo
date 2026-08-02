package com.sigmundgranaas.turbo.expressive.core.routing

import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.BeforeClass
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.turbo_route_ffi.GeoPoint
import uniffi.turbo_route_ffi.RouteEngine
import uniffi.turbo_route_ffi.RouteOptions
import uniffi.turbo_route_ffi.RouteProgress
import uniffi.turbo_route_ffi.TravelMode
import java.io.File

/**
 * The routing engine, on real silicon.
 *
 * This is measurement M1, as a command: `./gradlew
 * :core:routing-android:connectedAndroidTest`. It needs no server and no
 * downloaded pack — the committed 4.6 MB CI pack is copied in as a test
 * asset — so it works on a phone in aeroplane mode.
 *
 * # What only a device can answer
 *
 * Almost everything else has been settled without one.
 * `tools/bionic_parity.sh` proves the solver is bit-identical across
 * x86_64/glibc, aarch64/glibc and aarch64 + **bionic** — while all three
 * libms provably disagree — so libc is not the open question. What is
 * left is the parts that have no emulator:
 *
 * - **JNA on ART.** The `.so` loading, the uniffi scaffolding binding to
 *   it, and a foreign callback (`RouteProgress`) being invoked from
 *   Rust on a thread the JVM did not create.
 * - **Latency.** Every per-lane figure in this codebase is extrapolated
 *   from a desktop, and the whole routing policy — server-first, with
 *   the phone behind it — is waiting on a real one.
 *
 * The numbers are logged rather than asserted. A CI machine's phone and
 * a user's phone are different computers, and a threshold picked here
 * would either be so loose it never fires or so tight it fails on the
 * slow device that most needs measuring. Read them with
 * `adb logcat -s TurboRouting`.
 */
@RunWith(AndroidJUnit4::class)
class RoutingOnDeviceTest {

    companion object {
        private const val TAG = "TurboRouting"

        /** Two points on the Sjunkhatten trail network, inside the pack. */
        private val FROM = GeoPoint(lon = 15.04048, lat = 67.065016)
        private val TO = GeoPoint(lon = 15.0555, lat = 67.0685)

        private lateinit var packDir: File

        /**
         * Copy the pack out of the APK's assets onto the device.
         *
         * The engine mmaps files, so it needs a real path — an
         * `AssetManager` stream is not one. Copied once for the class:
         * `RouteEngine.open` builds the trail R-trees, and doing that per
         * test would measure the copy as much as the engine.
         */
        @JvmStatic
        @BeforeClass
        fun stagePack() {
            val ctx = InstrumentationRegistry.getInstrumentation().context
            packDir = File(ctx.cacheDir, "ci-pack").apply { mkdirs() }
            val assets = ctx.assets.list("ci-pack").orEmpty()
            assertTrue(
                "the CI pack must be packaged as a test asset — check the " +
                    "stageRoutingPack Gradle task",
                assets.isNotEmpty(),
            )
            for (name in assets) {
                val out = File(packDir, name)
                if (out.length() > 0L) continue
                ctx.assets.open("ci-pack/$name").use { input ->
                    out.outputStream().use { input.copyTo(it) }
                }
            }
        }
    }

    private fun engine(): RouteEngine = RouteEngine.open(packDir.absolutePath)

    @Test
    fun theEngineOpensAndRoutesOnThisDevice() {
        val openMs = measure { engine() }
        val e = engine()

        val cov = e.coverage()
        assertTrue("the pack must report coverage", cov.maxLat > cov.minLat)
        assertTrue("the endpoints must be inside it", e.hasCoverage(FROM))

        val route = e.plan(listOf(FROM, TO), RouteOptions(mode = TravelMode.FOOT))
        assertTrue("a route must have geometry", route.geometry.size > 2)
        assertTrue("a route must have length", route.lengthM > 0.0)

        Log.i(TAG, "open: $openMs ms")
        Log.i(TAG, "route: ${route.lengthM.toInt()} m, ${route.geometry.size} pts")
    }

    /**
     * The geometry a phone computes, against what the host computes.
     *
     * The hash is logged, not asserted against a constant. A committed
     * golden would be one more thing to rebaseline on every calibration
     * change, and the property that matters is **equivalence**, not
     * bit-identity — that was the explicit call. Compare it against
     * `tools/bionic_parity.sh` when a divergence is suspected; those two
     * print the same number in the same format on purpose.
     */
    @Test
    fun theRouteMatchesWhatTheHostComputes() {
        val route = engine().plan(listOf(FROM, TO), RouteOptions(mode = TravelMode.FOOT))
        var h = -0x340d631b7bdddcdbL // 0xcbf29ce484222325
        for (p in route.geometry) {
            for (v in listOf((p.lon * 1e7).toLong(), (p.lat * 1e7).toLong())) {
                for (i in 0 until 8) {
                    h = h xor ((v shr (i * 8)) and 0xff)
                    h *= 0x100000001b3L
                }
            }
        }
        Log.i(TAG, "geometry hash: %016x  (%.3f m)".format(h, route.lengthM))
        Log.i(TAG, "compare with: tools/bionic_parity.sh")
    }

    /**
     * Per-lane latency — the number the routing policy is waiting on.
     *
     * The unified lane should be roughly flat in distance (adaptive cell
     * sizing bounds it) and the cross-country lane should not. If that
     * shape does not hold on a phone, `maxOffTrailKm` is set wrong.
     */
    @Test
    fun latencyByLane() {
        val e = engine()
        for (offTrail in listOf(false, true)) {
            val lane = if (offTrail) "cross-country" else "unified     "
            // Three runs: the first pays for page-faulting the DEM in.
            val runs = (1..3).map {
                measure { e.plan(listOf(FROM, TO), RouteOptions(mode = TravelMode.FOOT, forceOffTrail = offTrail)) }
            }
            Log.i(TAG, "$lane  first ${runs.first()} ms, warm ${runs.drop(1).min()} ms")
        }
    }

    /**
     * A foreign callback, invoked from Rust, on a thread the JVM did not
     * create. The part of the streaming path no host test can reach.
     */
    @Test
    fun progressCallbacksCrossTheBoundary() {
        val seen = java.util.concurrent.atomic.AtomicInteger()
        val observer = object : RouteProgress {
            override fun onProgress(geometry: List<GeoPoint>) {
                if (geometry.isNotEmpty()) seen.incrementAndGet()
            }
        }
        val route = engine()
            .planWithProgress(listOf(FROM, TO), RouteOptions(mode = TravelMode.FOOT), observer)

        assertTrue("a route must still come back", route.geometry.isNotEmpty())
        assertTrue(
            "expected progress snapshots on the device, got ${seen.get()}",
            seen.get() > 1,
        )
        Log.i(TAG, "progress snapshots: ${seen.get()}")
    }

    /** Presets must all resolve on the device, or a dropdown entry throws. */
    @Test
    fun everyPresetResolves() {
        val e = engine()
        for (preset in listOf("balanced", "avoid_roads", "direct", "easy_grade", "trail_purist")) {
            val r = e.plan(
                listOf(FROM, TO),
                RouteOptions(mode = TravelMode.FOOT, preset = preset),
            )
            assertTrue("$preset produced no route", r.geometry.isNotEmpty())
        }
        assertEquals(Unit, Unit)
    }

    private inline fun measure(block: () -> Unit): Long {
        val t = System.nanoTime()
        block()
        return (System.nanoTime() - t) / 1_000_000
    }
}
