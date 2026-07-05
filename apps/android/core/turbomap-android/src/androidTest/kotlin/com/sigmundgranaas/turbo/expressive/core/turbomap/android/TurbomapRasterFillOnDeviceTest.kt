package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.ByteArrayOutputStream

/**
 * Regression for the "map renders into a small grey-bordered island" report.
 *
 * The grey is the engine's empty-tile clear colour (`BACKGROUND_CLEAR` ≈
 * sRGB 170,170,165); the island was just the handful of tiles that loaded
 * before host tile-fetching stalled. This test rules out a *projection* cause:
 * at a tall phone aspect, when **every** pending raster tile is ingested, the
 * basemap must cover the whole framebuffer — not a centred sub-rectangle.
 *
 * It feeds synthetic solid-red tiles (no network) for exactly the z/x/y the
 * engine asks for, then asserts the presented frame is overwhelmingly red and
 * carries almost no leftover clear-colour grey.
 */
@RunWith(AndroidJUnit4::class)
class TurbomapRasterFillOnDeviceTest {

    // A tall, phone-like surface — the aspect at which the bug was reported.
    private val width = 360
    private val height = 780

    private val rasterScene = """
        { "sources": { "base": { "type": "raster-xyz",
            "tiles": ["https://example.test/{z}/{x}/{y}.png"] } },
          "layers": [ { "type": "raster", "id": "base", "source": "base" } ] }
    """.trimIndent()

    @Test
    fun ingested_basemap_fills_the_whole_surface() {
        val reader = ImageReader.newInstance(
            width, height, PixelFormat.RGBA_8888, 3,
            HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN,
        )
        val redTile = solidPng(Color.rgb(255, 0, 0))
        var handle = 0L
        try {
            handle = NativeSurfaceMap.nativeCreate(reader.surface, width, height, 60.39, 5.32, 9.0)
            assertNotEquals("surface map should be created", 0L, handle)
            assertTrue("raster scene should apply", NativeSurfaceMap.nativeApplyScene(handle, rasterScene))

            // Async FFI: applyScene / pumpLocal / ingest all apply on the NEXT
            // render (wait-free command queue), and tile uploads are rate-limited
            // per frame. So each round renders to apply the prior commands + the
            // scene and publish a fresh pending list, pumps, renders again so
            // `pendingTilesJson` reflects the desired set, then ingests it. A dozen
            // rounds + the readback renders below drain the visible + prefetch set.
            var ingested = 0
            repeat(12) {
                NativeSurfaceMap.nativeRender(handle)
                NativeSurfaceMap.nativePumpLocal(handle)
                NativeSurfaceMap.nativeRender(handle)
                // Streaming plan (P5.1): drain the minted starts and deliver.
                for (o in planStarts(handle)) {
                    if (o.optString("kind") != "raster") continue
                    NativeSurfaceMap.nativeIngestRaster(
                        handle, o.optString("layer"), o.optInt("z"), o.optInt("x"), o.optInt("y"), redTile,
                    )
                    ingested++
                }
            }
            assertTrue("the engine should have requested basemap tiles (got $ingested)", ingested > 0)

            val (red, grey) = renderAndCount(handle, reader)
            val total = width * height
            // The basemap must blanket the surface: red dominates, the empty-tile
            // grey is negligible (a thin seam at most, never a full border).
            assertTrue("basemap should fill the surface — only $red/$total px red", red > total * 0.9)
            assertTrue("empty-tile grey should be gone — $grey/$total px still grey", grey < total * 0.02)
        } finally {
            if (handle != 0L) NativeSurfaceMap.nativeDestroy(handle)
            reader.close()
        }
    }

    @Test
    fun ingested_tile_fades_in_then_settles() {
        val reader = ImageReader.newInstance(
            256, 256, PixelFormat.RGBA_8888, 3,
            HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN,
        )
        val redTile = solidPng(Color.rgb(255, 0, 0))
        var handle = 0L
        try {
            handle = NativeSurfaceMap.nativeCreate(reader.surface, 256, 256, 60.39, 5.32, 9.0)
            assertTrue(handle != 0L)
            assertTrue(NativeSurfaceMap.nativeApplyScene(handle, rasterScene))
            // Async FFI: applyScene / pumpLocal / ingest apply on the next render,
            // and the fade clock only advances when render() ticks it.
            NativeSurfaceMap.nativeRender(handle) // apply the scene
            assertTrue("no fade running before any tile arrives", !NativeSurfaceMap.nativeIsAnimating(handle))

            NativeSurfaceMap.nativePumpLocal(handle)
            NativeSurfaceMap.nativeRender(handle) // apply pumpLocal → the plan mints
            val pending = JSONArray()
            planStarts(handle).forEach { pending.put(it) }

            // Ingest the VISIBLE viewport tile — the sharpest (highest-z) raster the
            // engine wants, the one that fades in over the coarse base. Don't assume
            // pending[0]: pending is streaming-priority ordered (Overview floor first,
            // then Visible, then Prefetch — see turbomap TileTier), so index 0 is the
            // coarse floor, not the tile whose fade-in this test is about.
            var target: JSONObject? = null
            for (i in 0 until pending.length()) {
                val o = pending.optJSONObject(i) ?: continue
                if (o.optString("kind") != "raster") continue
                if (target == null || o.optInt("z") > target!!.optInt("z")) target = o
            }
            assertTrue("engine should request a viewport raster tile", target != null)
            val t = target!!
            NativeSurfaceMap.nativeIngestRaster(handle, t.optString("layer"), t.optInt("z"), t.optInt("x"), t.optInt("y"), redTile)

            // The ingest + GPU upload that starts the fade applies on a later frame
            // (async command queue + per-frame upload budget), so pump a bounded
            // number of renders until the fade is observed rather than assuming
            // exactly one render starts it. If it never fades, that's a real signal.
            var fading = false
            for (frame in 0 until 8) {
                NativeSurfaceMap.nativeRender(handle)
                if (NativeSurfaceMap.nativeIsAnimating(handle)) { fading = true; break }
            }
            assertTrue("a freshly ingested viewport tile fades in", fading)

            // Past the ~0.3 s fade window → the animation settles → render-on-demand
            // can park (this is the property the host relies on to stop drawing).
            Thread.sleep(450)
            NativeSurfaceMap.nativeRender(handle)
            assertTrue("fade completes and animation settles", !NativeSurfaceMap.nativeIsAnimating(handle))
        } finally {
            if (handle != 0L) NativeSurfaceMap.nativeDestroy(handle)
            reader.close()
        }
    }

    private fun solidPng(color: Int): ByteArray {
        val bmp = Bitmap.createBitmap(256, 256, Bitmap.Config.ARGB_8888)
        bmp.eraseColor(color)
        return ByteArrayOutputStream().use { out ->
            bmp.compress(Bitmap.CompressFormat.PNG, 100, out)
            out.toByteArray()
        }
    }

    private fun renderAndCount(handle: Long, reader: ImageReader): Pair<Int, Int> {
        var best = 0 to 0
        repeat(20) {
            NativeSurfaceMap.nativeRender(handle)
            Thread.sleep(30)
            val image = reader.acquireLatestImage() ?: return@repeat
            try {
                val counts = countRedAndGrey(image)
                if (counts.first > best.first) best = counts
            } finally {
                image.close()
            }
        }
        return best
    }

    private fun countRedAndGrey(image: android.media.Image): Pair<Int, Int> {
        val plane = image.planes[0]
        val buf = plane.buffer
        val rowStride = plane.rowStride
        val pixelStride = plane.pixelStride
        var red = 0
        var grey = 0
        for (y in 0 until image.height) {
            val rowStart = y * rowStride
            for (x in 0 until image.width) {
                val i = rowStart + x * pixelStride
                val r = buf.get(i).toInt() and 0xFF
                val g = buf.get(i + 1).toInt() and 0xFF
                val b = buf.get(i + 2).toInt() and 0xFF
                if (r > 180 && g < 80 && b < 80) red++
                // BACKGROUND_CLEAR ≈ sRGB (170,170,165): near-neutral mid-grey.
                else if (r in 150..190 && g in 150..190 && b in 145..185 &&
                    kotlin.math.abs(r - g) < 12 && kotlin.math.abs(g - b) < 16
                ) {
                    grey++
                }
            }
        }
        return red to grey
    }
}


/** Grant generous lanes, render once so the plan mints, and return every
 *  `start` entry across the drained plans (P5.1 transport). */
private fun planStarts(handle: Long): List<JSONObject> {
    // Grant lanes first (a take is also a grant), render so the plan mints
    // under that grant, then drain what was minted.
    NativeSurfaceMap.nativeTakeStreamingPlanJson(handle, 256)
    NativeSurfaceMap.nativeRender(handle)
    val plans = JSONArray(NativeSurfaceMap.nativeTakeStreamingPlanJson(handle, 256))
    val out = mutableListOf<JSONObject>()
    for (p in 0 until plans.length()) {
        val starts = plans.optJSONObject(p)?.optJSONArray("start") ?: continue
        for (i in 0 until starts.length()) starts.optJSONObject(i)?.let { out.add(it) }
    }
    return out
}
