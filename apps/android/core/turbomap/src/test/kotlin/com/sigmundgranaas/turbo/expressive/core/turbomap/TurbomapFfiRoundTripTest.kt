package com.sigmundgranaas.turbo.expressive.core.turbomap

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import uniffi.turbomap_ffi.Camera
import uniffi.turbomap_ffi.FfiException
import uniffi.turbomap_ffi.GeoPoint
import uniffi.turbomap_ffi.TurboMap
import java.awt.image.BufferedImage
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import javax.imageio.ImageIO

/**
 * Stage-B FFI binding gate: the *generated Kotlin* bindings + JNA + the freshly
 * built cdylib drive the wgpu engine end-to-end on the host GPU, mirroring the
 * Rust `roundtrip.rs`. If this is green, the control plane the Android host will
 * talk to actually works in Kotlin — not just in Rust.
 *
 * The engine renders offscreen here; the on-screen surface path is Stage C.
 */
class TurbomapFfiRoundTripTest {

    private fun camera() = Camera(lat = 60.39, lng = 5.32, zoom = 9.0, pitchDeg = 0.0, bearingDeg = 0.0)

    /**
     * A map on the host GPU, or skip — unless `REQUIRE_GPU=1` (CI Lane D), where a
     * missing adapter is a hard failure so a broken software-Vulkan install can't
     * silently pass the suite.
     */
    private fun newMap(width: UInt = 512u, height: UInt = 384u): TurboMap =
        try {
            TurboMap.headless(width, height, camera())
        } catch (e: FfiException.NoAdapter) {
            if (System.getenv("REQUIRE_GPU") == "1") throw e
            assumeTrue("no usable GPU adapter: ${e.message}", false)
            error("unreachable")
        }

    private fun sceneJson() = """
        {
          "sources": {
            "base": { "type": "raster-xyz", "tiles": ["https://example.test/{z}/{x}/{y}.png"] },
            "route": { "type": "geo-json",
              "data": "{\"type\":\"LineString\",\"coordinates\":[[5.10,60.30],[5.32,60.39],[5.55,60.48]]}" }
          },
          "layers": [
            { "type": "raster", "id": "basemap", "source": "base" },
            { "type": "line", "id": "route", "source": "route",
              "color": { "const": { "r": 220, "g": 30, "b": 60, "a": 255 } },
              "width": { "const": 5.0 } }
          ]
        }
    """.trimIndent()

    private data class PixelCounts(val greenish: Int, val reddish: Int, val total: Int)

    /** Classify every pixel as sea-green basemap / red route, by the same thresholds as `roundtrip.rs`. */
    private fun countPixels(img: BufferedImage): PixelCounts {
        var greenish = 0
        var reddish = 0
        for (y in 0 until img.height) {
            for (x in 0 until img.width) {
                val p = img.getRGB(x, y)
                val r = (p shr 16) and 0xFF
                val g = (p shr 8) and 0xFF
                val b = p and 0xFF
                if (g > r && g > 120 && b > 100) greenish++
                if (r > 180 && g < 100) reddish++
            }
        }
        return PixelCounts(greenish, reddish, img.width * img.height)
    }

    /** A solid sea-green 256×256 "fetched tile", PNG-encoded like a server would serve. */
    private fun fakeTilePng(): ByteArray {
        val img = BufferedImage(256, 256, BufferedImage.TYPE_INT_ARGB)
        val argb = (0xFF shl 24) or (90 shl 16) or (170 shl 8) or 140
        for (y in 0 until 256) for (x in 0 until 256) img.setRGB(x, y, argb)
        val out = ByteArrayOutputStream()
        ImageIO.write(img, "png", out)
        return out.toByteArray()
    }

    @Test
    fun `full host round-trip through the FFI surface`() {
        newMap().use { map ->
            // 1. Apply the scene; both layers + sources arrive, none unsupported.
            val delta = map.applyScene(sceneJson())
            assertEquals("both layers added: $delta", 2u, delta.layersAdded)
            assertEquals("both sources changed: $delta", 2u, delta.sourcesChanged)
            assertTrue(map.unsupportedLayers().isEmpty())

            // 2. GeoJSON drains in-process; remote raster tiles surface via the
            //    streaming plan (P5.1 — the engine plans, the host executes).
            val local = map.pumpLocalTiles()
            assertTrue("geojson should drain: $local", local.vectorTiles > 0u)

            // 3. Host fetch loop: drain plans, pushing an encoded PNG for each
            //    start, until the engine wants nothing more.
            val tile = fakeTilePng()
            var delivered = 0
            while (true) {
                val starts = planStarts(map.streamingPlanJson(64u))
                if (starts.isEmpty()) break
                assertTrue("only the basemap raster starts: $starts", starts.all { it.kind == "raster" && it.layer == "basemap" })
                for (req in starts) {
                    assertTrue("tile $req should decode", map.ingestRasterTile(req.layer, req.z, req.x, req.y, tile))
                }
                delivered += starts.size
            }
            assertTrue("remote raster tiles should have been planned", delivered > 0)

            // 4. Snapshot through the FFI and check actual pixels: sea-green basemap
            //    dominates, the red route is visible.
            val img = ImageIO.read(ByteArrayInputStream(map.renderPng()))
            assertEquals(512 to 384, img.width to img.height)
            val px = countPixels(img)
            assertTrue("basemap should dominate: greenish=${px.greenish}/${px.total}", px.greenish * 2 > px.total)
            assertTrue("route should be visible: reddish=${px.reddish}", px.reddish > 50)
        }
    }

    @Test
    fun `camera projection and hit-test through the FFI surface`() {
        newMap().use { map ->
            // Camera round-trip.
            val cam = map.camera()
            assertTrue(Math.abs(cam.lat - 60.39) < 1e-9 && Math.abs(cam.zoom - 9.0) < 1e-9)
            map.setCamera(cam.copy(zoom = 11.0))
            assertTrue(Math.abs(map.camera().zoom - 11.0) < 1e-9)

            // project ∘ unproject ≈ identity at pitch 0.
            val geo = GeoPoint(lat = 60.40, lng = 5.33)
            val screen = map.project(geo)
            assertNotNull("project should succeed", screen)
            val back = map.unproject(screen!!)
            assertNotNull("unproject should succeed", back)
            assertTrue(
                "round-trip drifted: $geo -> $screen -> $back",
                Math.abs(back!!.lat - geo.lat) < 1e-6 && Math.abs(back.lng - geo.lng) < 1e-6,
            )

            // Hit-testing a circle layer through the FFI.
            val circleScene = """
                {
                  "sources": { "pts": { "type": "geo-json",
                    "data": "{\"type\":\"Point\",\"coordinates\":[5.33,60.40]}" } },
                  "layers": [ { "type": "circle", "id": "dot", "source": "pts",
                    "color": { "const": { "r": 255, "g": 200, "b": 0, "a": 255 } },
                    "radius": { "const": 10.0 } } ]
                }
            """.trimIndent()
            map.applyScene(circleScene)
            val dot = map.project(geo)!!
            val hits = map.hitTest(dot, 8.0)
            assertTrue("circle should be hit at its own position: $hits", hits.any { it.layerId == "dot" })
        }
    }

    @Test
    fun `bad scenes are marshalled as structured errors, never panics`() {
        newMap().use { map ->
            // Invalid JSON.
            try {
                map.applyScene("not json")
                error("expected InvalidScene")
            } catch (e: FfiException) {
                assertTrue(e is FfiException.InvalidScene)
            }
            // A layer referencing a missing source — validation names the problem.
            val dangling = """{ "layers": [ { "type": "raster", "id": "x", "source": "missing" } ] }"""
            try {
                map.applyScene(dangling)
                error("expected validation error")
            } catch (e: FfiException) {
                assertTrue("should name the problem: ${e.message}", e.message?.contains("unknown source") == true)
            }
        }
    }

    @Test
    fun `repeated attach-detach cycles do not leak the engine`() {
        // Construct + close many maps; the AutoCloseable handle frees the Rust
        // object each time. A leak here (handle map growth / native OOM) surfaces
        // as a failure or crash rather than slipping past the functional tests.
        repeat(25) {
            newMap(256u, 256u).use { map ->
                map.applyScene(sceneJson())
                map.pumpLocalTiles()
                assertNotNull(map.project(GeoPoint(60.40, 5.33))) // exercise the engine each cycle
            }
        }
    }
}
