package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.ByteArrayOutputStream
import java.io.File

/**
 * The AIRPLANE-MODE gate (plan P6.6): a region whose tiles were pre-populated
 * into the shared [TileStore] — exactly what the offline downloader does —
 * renders to full basemap coverage with the network never consulted.
 *
 * Two-phase, which also pins invariant 5 (deterministic selection) across
 * surface sessions:
 *  1. a first surface session drains its streaming plan and "downloads" every
 *     requested tile into the store (the downloader's write path: the same
 *     `TileStore.put(layer, z, x, y)` keys);
 *  2. a FRESH surface over the same scene + camera drains its own plan and
 *     serves ONLY `TileStore.get` — a store miss fails the test (the second
 *     session asked for a tile the first session's download didn't cover),
 *     and the frame must end overwhelmingly basemap-coloured.
 */
@RunWith(AndroidJUnit4::class)
class TurbomapOfflineOnDeviceTest {

    private val width = 360
    private val height = 640

    // `offline.invalid` can never resolve — any accidental network dependence
    // in this path would surface as a coverage failure, not a silent pass.
    private val scene = """
        { "sources": { "base": { "type": "raster-xyz",
            "tiles": ["https://offline.invalid/{z}/{x}/{y}.png"] } },
          "layers": [ { "type": "raster", "id": "base", "source": "base" } ] }
    """.trimIndent()

    @Test
    fun downloaded_region_renders_fully_without_network() {
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val storeDir = File(ctx.cacheDir, "offline-gate-test").apply { deleteRecursively() }
        val store = TileStore(storeDir)
        val redTile = solidPng(Color.rgb(255, 0, 0))

        // Phase 1 — the "download": a session over the scene populates the
        // store for every raster tile its plan requests.
        var stored = 0
        withSurface { handle, _ ->
            assertTrue(NativeSurfaceMap.nativeApplyScene(handle, scene))
            repeat(12) {
                NativeSurfaceMap.nativeRender(handle)
                NativeSurfaceMap.nativePumpLocal(handle)
                NativeSurfaceMap.nativeRender(handle)
                for (o in planStarts(handle)) {
                    if (o.optString("kind") != "raster") continue
                    store.put(o.optString("layer"), o.optInt("z"), o.optInt("x"), o.optInt("y"), redTile)
                    // Deliver too, so this session settles and stops re-planning.
                    NativeSurfaceMap.nativeIngestRaster(
                        handle, o.optString("layer"), o.optInt("z"), o.optInt("x"), o.optInt("y"), redTile,
                    )
                    stored++
                }
            }
        }
        assertTrue("phase 1 should have downloaded tiles (got $stored)", stored > 0)

        // Phase 2 — airplane mode: a FRESH session serves only the store.
        withSurface { handle, reader ->
            assertTrue(NativeSurfaceMap.nativeApplyScene(handle, scene))
            var misses = 0
            var served = 0
            repeat(12) {
                NativeSurfaceMap.nativeRender(handle)
                NativeSurfaceMap.nativePumpLocal(handle)
                NativeSurfaceMap.nativeRender(handle)
                for (o in planStarts(handle)) {
                    if (o.optString("kind") != "raster") continue
                    val bytes = store.get(o.optString("layer"), o.optInt("z"), o.optInt("x"), o.optInt("y"))
                    if (bytes == null) {
                        misses++
                        NativeSurfaceMap.nativeReportFetchFailed(handle, o.optLong("id"))
                    } else {
                        NativeSurfaceMap.nativeIngestRaster(
                            handle, o.optString("layer"), o.optInt("z"), o.optInt("x"), o.optInt("y"), bytes,
                        )
                        served++
                    }
                }
            }
            assertTrue("phase 2 should serve tiles from the store (got $served)", served > 0)
            assertEquals(
                "deterministic selection: the offline session asked for tiles the download never stored",
                0,
                misses,
            )
            val red = bestRedCount(handle, reader)
            val total = width * height
            assertTrue(
                "offline basemap must blanket the surface (red ${red * 100 / total}%)",
                red > total * 85 / 100,
            )
        }
        storeDir.deleteRecursively()
    }

    // ---- harness (mirrors TurbomapRasterFillOnDeviceTest) ----------------

    private inline fun withSurface(block: (handle: Long, reader: ImageReader) -> Unit) {
        val reader = ImageReader.newInstance(
            width, height, PixelFormat.RGBA_8888, 3,
            HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN,
        )
        var handle = 0L
        try {
            handle = NativeSurfaceMap.nativeCreate(reader.surface, width, height, 60.39, 5.32, 9.0)
            assertNotEquals("surface map should be created", 0L, handle)
            block(handle, reader)
        } finally {
            if (handle != 0L) NativeSurfaceMap.nativeDestroy(handle)
            reader.close()
        }
    }

    private fun planStarts(handle: Long): List<JSONObject> {
        val plans = JSONArray(NativeSurfaceMap.nativeTakeStreamingPlanJson(handle, 64))
        val out = mutableListOf<JSONObject>()
        for (i in 0 until plans.length()) {
            val starts = plans.getJSONObject(i).optJSONArray("start") ?: continue
            for (j in 0 until starts.length()) out.add(starts.getJSONObject(j))
        }
        return out
    }

    /** Render + read back a few frames (rate-limited uploads land over several
     *  frames) and return the best red-pixel count observed. */
    private fun bestRedCount(handle: Long, reader: ImageReader): Int {
        var best = 0
        repeat(20) {
            NativeSurfaceMap.nativeRender(handle)
            Thread.sleep(30)
            val image = reader.acquireLatestImage() ?: return@repeat
            try {
                val plane = image.planes[0]
                val buf = plane.buffer
                val rowStride = plane.rowStride
                val pixelStride = plane.pixelStride
                var red = 0
                for (y in 0 until image.height) {
                    val rowStart = y * rowStride
                    for (x in 0 until image.width) {
                        val i = rowStart + x * pixelStride
                        val r = buf.get(i).toInt() and 0xFF
                        val g = buf.get(i + 1).toInt() and 0xFF
                        val b = buf.get(i + 2).toInt() and 0xFF
                        if (r > 180 && g < 80 && b < 80) red++
                    }
                }
                if (red > best) best = red
            } finally {
                image.close()
            }
        }
        return best
    }

    private fun solidPng(color: Int): ByteArray {
        val bmp = Bitmap.createBitmap(256, 256, Bitmap.Config.ARGB_8888)
        bmp.eraseColor(color)
        val out = ByteArrayOutputStream()
        bmp.compress(Bitmap.CompressFormat.PNG, 100, out)
        return out.toByteArray()
    }
}
