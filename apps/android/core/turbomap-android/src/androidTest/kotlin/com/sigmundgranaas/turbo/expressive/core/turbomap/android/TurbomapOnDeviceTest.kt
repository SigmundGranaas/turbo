package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import android.graphics.BitmapFactory
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.turbomap_ffi.Camera
import uniffi.turbomap_ffi.GeoPoint
import uniffi.turbomap_ffi.TurboMap
import java.io.ByteArrayOutputStream

/**
 * Stage-C foundation gate — runs on the **device's real GPU** (Vulkan/GLES on
 * the emulator/phone), not a host software adapter. Green here proves the whole
 * native stack lines up on Android: the cargo-ndk `.so` is packaged in the APK,
 * JNA on ART loads it, the uniffi bindings marshal correctly, and wgpu finds an
 * adapter and renders the expected pixels. Mirrors the host round-trip
 * (`:core:turbomap`) and the Rust `roundtrip.rs`.
 */
@RunWith(AndroidJUnit4::class)
class TurbomapOnDeviceTest {

    private fun camera() = Camera(lat = 60.39, lng = 5.32, zoom = 9.0, pitchDeg = 0.0, bearingDeg = 0.0)

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

    private fun fakeTilePng(): ByteArray {
        val bmp = android.graphics.Bitmap.createBitmap(256, 256, android.graphics.Bitmap.Config.ARGB_8888)
        bmp.eraseColor(android.graphics.Color.argb(255, 90, 170, 140))
        val out = ByteArrayOutputStream()
        bmp.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, out)
        return out.toByteArray()
    }

    @Test
    fun engine_renders_on_the_device_gpu() {
        // No try/skip: on a device we *require* an adapter — a failure here is the
        // signal that wgpu can't run on this hardware, which is what we're testing.
        TurboMap.headless(512u, 384u, camera()).use { map ->
            val delta = map.applyScene(sceneJson())
            assertEquals("both layers added: $delta", 2u, delta.layersAdded)
            assertTrue(map.unsupportedLayers().isEmpty())

            val local = map.pumpLocalTiles()
            assertTrue("geojson should drain: $local", local.vectorTiles > 0u)

            // Drain the streaming plan (P5.1), delivering a PNG per start,
            // until the engine wants nothing more.
            val tile = fakeTilePng()
            var delivered = 0
            while (true) {
                val starts = planStarts(map.streamingPlanJson(64u))
                if (starts.isEmpty()) break
                assertTrue("only rasters start: $starts", starts.all { it.kind == "raster" })
                for (req in starts) {
                    assertTrue("tile $req should decode", map.ingestRasterTile(req.layer, req.z, req.x, req.y, tile))
                }
                delivered += starts.size
            }
            assertTrue("remote raster tiles should have been planned", delivered > 0)

            val png = map.renderPng()
            val bmp = BitmapFactory.decodeByteArray(png, 0, png.size)
            assertNotNull("renderPng should decode (${png.size} bytes)", bmp)
            assertEquals(512 to 384, bmp.width to bmp.height)
            val counts = countPixels(bmp)
            assertTrue("basemap should dominate: ${counts.greenish}/${counts.total}", counts.greenish * 2 > counts.total)
            assertTrue("route should be visible: ${counts.reddish}", counts.reddish > 50)
        }
    }

    @Test
    fun camera_projection_round_trip_on_device() {
        TurboMap.headless(256u, 256u, camera()).use { map ->
            val geo = GeoPoint(lat = 60.40, lng = 5.33)
            val screen = map.project(geo)
            assertNotNull("project should succeed", screen)
            val back = map.unproject(screen!!)
            assertNotNull("unproject should succeed", back)
            assertTrue(
                "round-trip drifted: $geo -> $screen -> $back",
                Math.abs(back!!.lat - geo.lat) < 1e-6 && Math.abs(back.lng - geo.lng) < 1e-6,
            )
        }
    }

    /** One `start` from a minted streaming-plan JSON (plan P5.1). */
    private data class PlanStart(val id: Long, val kind: String, val layer: String, val z: UByte, val x: UInt, val y: UInt)

    private fun planStarts(planJson: String): List<PlanStart> =
        START_RE.findAll(planJson).map { m ->
            PlanStart(
                id = m.groupValues[1].toLong(),
                kind = m.groupValues[2],
                layer = m.groupValues[3],
                z = m.groupValues[4].toUByte(),
                x = m.groupValues[5].toUInt(),
                y = m.groupValues[6].toUInt(),
            )
        }.toList()

    private data class PixelCounts(val greenish: Int, val reddish: Int, val total: Int)

    private companion object {
        // NOTE: the closing `\}` must stay escaped — Android's ICU regex
        // engine rejects a bare `}` that desktop java.util.regex accepts
        // (found the hard way: both on-device tests crashed at <clinit>).
        val START_RE =
            Regex("""\{"id":(\d+),"kind":"([^"]+)","layer":"([^"]+)","z":(\d+),"x":(\d+),"y":(\d+)\}""")
    }

    private fun countPixels(bmp: android.graphics.Bitmap): PixelCounts {
        var greenish = 0
        var reddish = 0
        for (y in 0 until bmp.height) {
            for (x in 0 until bmp.width) {
                val p = bmp.getPixel(x, y)
                val r = (p shr 16) and 0xFF
                val g = (p shr 8) and 0xFF
                val b = p and 0xFF
                if (g > r && g > 120 && b > 100) greenish++
                if (r > 180 && g < 100) reddish++
            }
        }
        return PixelCounts(greenish, reddish, bmp.width * bmp.height)
    }
}
