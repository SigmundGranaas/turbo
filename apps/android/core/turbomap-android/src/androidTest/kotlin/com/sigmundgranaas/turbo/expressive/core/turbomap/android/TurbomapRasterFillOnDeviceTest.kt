package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.json.JSONArray
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

            // Drain pending → ingest synthetic tiles → repeat until the engine
            // stops asking (covers the visible set + prefetch ring).
            var ingested = 0
            repeat(6) {
                NativeSurfaceMap.nativePumpLocal(handle)
                val pending = JSONArray(NativeSurfaceMap.nativePendingTilesJson(handle))
                if (pending.length() == 0) return@repeat
                for (i in 0 until pending.length()) {
                    val o = pending.optJSONObject(i) ?: continue
                    if (o.optString("kind") != "raster") continue
                    NativeSurfaceMap.nativeIngestRaster(
                        handle, o.optString("layer"), o.optInt("z"), o.optInt("x"), o.optInt("y"), redTile,
                    )
                    ingested++
                }
                NativeSurfaceMap.nativeRender(handle)
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
