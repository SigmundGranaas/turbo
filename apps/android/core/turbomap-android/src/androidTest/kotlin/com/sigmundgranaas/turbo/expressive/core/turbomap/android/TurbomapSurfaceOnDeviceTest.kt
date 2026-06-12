package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Stage-C on-screen gate: drives the real wgpu **surface present** path on the
 * device — `Surface` → `ANativeWindow` → `wgpu::Surface` → render → present —
 * and reads the presented frame back through an `ImageReader` (a real producer
 * surface, so this is deterministic without an on-screen `SurfaceView`).
 *
 * Proves the JNI surface glue (`surface.rs` / [NativeSurfaceMap]) works on a
 * real device GPU: a central yellow disc is actually presented to the surface.
 */
@RunWith(AndroidJUnit4::class)
class TurbomapSurfaceOnDeviceTest {

    private val width = 256
    private val height = 256

    // A circle drawn in screen space at the camera centre — drains in-process
    // (no tile fetch), so it's a clean "did the surface present geometry" probe.
    private val circleScene = """
        {
          "sources": { "pts": { "type": "geo-json",
            "data": "{\"type\":\"Point\",\"coordinates\":[5.32,60.39]}" } },
          "layers": [ { "type": "circle", "id": "dot", "source": "pts",
            "color": { "const": { "r": 255, "g": 200, "b": 0, "a": 255 } },
            "radius": { "const": 48.0 } } ]
        }
    """.trimIndent()

    @Test
    fun engine_presents_to_an_android_surface() {
        val reader = ImageReader.newInstance(
            width, height, PixelFormat.RGBA_8888, 3,
            HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN,
        )
        var handle = 0L
        try {
            handle = NativeSurfaceMap.nativeCreate(reader.surface, width, height, 60.39, 5.32, 9.0)
            assertNotEquals("surface map should be created (0 = failure)", 0L, handle)
            assertNull("a successful create must not report an engine error", NativeSurfaceMap.nativeLastError())
            assertTrue("scene should apply", NativeSurfaceMap.nativeApplyScene(handle, circleScene))
            NativeSurfaceMap.nativePumpLocal(handle)

            // Present + read back, retrying a few frames for the buffer to land.
            val yellow = renderUntilPixels(handle, reader)
            assertTrue("a yellow disc should be presented to the surface (got $yellow px)", yellow > 500)

            // Resize must not crash and must still present.
            NativeSurfaceMap.nativeResize(handle, 320, 200)
            NativeSurfaceMap.nativeRender(handle)
        } finally {
            if (handle != 0L) NativeSurfaceMap.nativeDestroy(handle)
            reader.close()
        }
    }

    private fun renderUntilPixels(handle: Long, reader: ImageReader): Int {
        var best = 0
        repeat(30) {
            NativeSurfaceMap.nativeRender(handle)
            Thread.sleep(40)
            val image = reader.acquireLatestImage() ?: return@repeat
            try {
                best = maxOf(best, countYellow(image))
            } finally {
                image.close()
            }
            if (best > 500) return best
        }
        return best
    }

    private fun countYellow(image: android.media.Image): Int {
        val plane = image.planes[0]
        val buf = plane.buffer
        val rowStride = plane.rowStride
        val pixelStride = plane.pixelStride
        var yellow = 0
        for (y in 0 until image.height) {
            val rowStart = y * rowStride
            for (x in 0 until image.width) {
                val i = rowStart + x * pixelStride
                val r = buf.get(i).toInt() and 0xFF
                val g = buf.get(i + 1).toInt() and 0xFF
                val b = buf.get(i + 2).toInt() and 0xFF
                if (r > 180 && g > 120 && b < 120) yellow++
            }
        }
        return yellow
    }
}
