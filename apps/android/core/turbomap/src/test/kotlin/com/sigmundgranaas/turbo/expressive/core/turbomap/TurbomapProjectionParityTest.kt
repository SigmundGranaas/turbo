package com.sigmundgranaas.turbo.expressive.core.turbomap

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import uniffi.turbomap_ffi.Camera
import uniffi.turbomap_ffi.FfiException
import uniffi.turbomap_ffi.GeoPoint
import uniffi.turbomap_ffi.TurboMap
import kotlin.math.PI
import kotlin.math.atan
import kotlin.math.cos
import kotlin.math.ln
import kotlin.math.sinh
import kotlin.math.tan

/**
 * Stage-D parity gate (projection): the wgpu engine's `project` must agree with the
 * **Web Mercator** projection MapLibre is defined by — that's what makes the Compose
 * overlays (markers/route/waypoints, drawn via `MapEngine.toScreen`) land in the same
 * place on either renderer. Rather than run MapLibre headless (a flaky GL snapshot on a
 * software emulator), this asserts the model-level invariants any correct web-mercator
 * renderer (incl. MapLibre) satisfies, host-side against the real engine: correct
 * centring, axis directions, longitude symmetry, and **conformality** (isotropic
 * pixels-per-mercator-unit). A live pixel-diff vs MapLibre `MapSnapshotter` is a
 * follow-up; this is the deterministic gate.
 */
class TurbomapProjectionParityTest {

    private val w = 512u
    private val h = 384u
    private val cx = 256.0
    private val cy = 192.0
    private val lat0 = 60.39
    private val lng0 = 5.32

    private fun newMap(): TurboMap =
        try {
            TurboMap.headless(w, h, Camera(lat0, lng0, 11.0, 0.0, 0.0))
        } catch (e: FfiException.NoAdapter) {
            if (System.getenv("REQUIRE_GPU") == "1") throw e
            assumeTrue("no usable GPU adapter: ${e.message}", false)
            error("unreachable")
        }

    // Normalised Web Mercator (the slippy-map convention MapLibre uses): world ∈ [0,1]².
    private fun worldY(latDeg: Double): Double {
        val r = Math.toRadians(latDeg)
        return (1.0 - ln(tan(r) + 1.0 / cos(r)) / PI) / 2.0
    }

    private fun latForWorldY(wy: Double): Double = Math.toDegrees(atan(sinh(PI * (1.0 - 2.0 * wy))))

    private fun project(map: TurboMap, lat: Double, lng: Double): Pair<Double, Double> {
        val p = map.project(GeoPoint(lat, lng)) ?: error("point ($lat,$lng) failed to project")
        return p.x to p.y
    }

    @Test
    fun `the camera centre projects to the viewport centre`() {
        newMap().use { map ->
            val (x, y) = project(map, lat0, lng0)
            assertEquals("centre x", cx, x, 1.0)
            assertEquals("centre y", cy, y, 1.0)
        }
    }

    @Test
    fun `axes point the web-mercator way (east right, north up)`() {
        newMap().use { map ->
            val (xe, ye) = project(map, lat0, lng0 + 0.05)
            val (xn, yn) = project(map, lat0 + 0.05, lng0)
            assertTrue("east is to the right: $xe", xe > cx + 1)
            assertEquals("east keeps the same screen Y", cy, ye, 2.0)
            assertTrue("north is up: $yn", yn < cy - 1)
            assertEquals("north keeps the same screen X", cx, xn, 2.0)
        }
    }

    @Test
    fun `longitude is symmetric about the centre`() {
        newMap().use { map ->
            val d = 0.05
            val (xe, _) = project(map, lat0, lng0 + d)
            val (xwest, _) = project(map, lat0, lng0 - d)
            assertEquals("east/west symmetric about centre", xe - cx, cx - xwest, 0.5)
        }
    }

    @Test
    fun `projection is conformal — isotropic pixels per mercator unit`() {
        newMap().use { map ->
            // Equal mercator-coordinate deltas in X (longitude) and Y (latitude) must map
            // to equal pixel deltas — the defining property of a conformal web-mercator
            // projection (and exactly what MapLibre does).
            val dm = 0.0008 // small mercator step, stays in the linear regime near centre
            val lngEast = lng0 + dm * 360.0 // dWorldX = dLng/360
            val latNorth = latForWorldY(worldY(lat0) - dm) // dWorldY = -dm (north)

            val (xe, _) = project(map, lat0, lngEast)
            val (_, yn) = project(map, latNorth, lng0)
            val dxEast = xe - cx
            val dyNorth = cy - yn
            assertTrue("east px delta positive: $dxEast", dxEast > 0)
            assertTrue("north px delta positive: $dyNorth", dyNorth > 0)
            // Isotropy: |Δpx_x| ≈ |Δpx_y| for equal mercator deltas (within renderer rounding).
            assertEquals("isotropic px/mercator (x vs y)", dxEast, dyNorth, dxEast * 0.05)
        }
    }
}
