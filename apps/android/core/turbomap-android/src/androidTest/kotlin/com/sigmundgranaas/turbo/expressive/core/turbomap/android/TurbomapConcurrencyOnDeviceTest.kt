package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.Collections
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.concurrent.thread

/**
 * Stage-2 gate: the native engine is now reached from a dedicated render thread
 * (the frame loop) *and* the UI thread (gestures, projection, the reconciler)
 * concurrently, serialised only by the `Mutex<OnScreen>` inside the FFI. This
 * pounds that mutex from three threads at once — render, projection, and camera
 * mutation — for a sustained burst. A data race / unsynchronised `&mut` would
 * crash the process (SIGSEGV) or throw; the mutex must keep it safe, and
 * projection must still return valid results after the storm.
 */
@RunWith(AndroidJUnit4::class)
class TurbomapConcurrencyOnDeviceTest {

    private val dot = """
        { "sources": { "pts": { "type": "geo-json",
            "data": "{\"type\":\"Point\",\"coordinates\":[5.32,60.39]}" } },
          "layers": [ { "type": "circle", "id": "dot", "source": "pts",
            "color": { "const": { "r": 255, "g": 200, "b": 0, "a": 255 } },
            "radius": { "const": 24.0 } } ] }
    """.trimIndent()

    @Test
    fun concurrent_render_projection_and_camera_is_crash_free() {
        val reader = ImageReader.newInstance(
            256, 256, PixelFormat.RGBA_8888, 3,
            HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN,
        )
        var handle = 0L
        try {
            handle = NativeSurfaceMap.nativeCreate(reader.surface, 256, 256, 60.39, 5.32, 9.0)
            assertNotEquals("surface map should be created", 0L, handle)
            NativeSurfaceMap.nativeApplyScene(handle, dot)
            NativeSurfaceMap.nativePumpLocal(handle)

            val h = handle
            val stop = AtomicBoolean(false)
            val errors = Collections.synchronizedList(mutableListOf<Throwable>())
            fun loop(body: () -> Unit) = thread {
                while (!stop.get()) runCatching(body).onFailure { errors.add(it) }
            }

            val renderT = loop { NativeSurfaceMap.nativeRender(h) }
            val projT = loop {
                NativeSurfaceMap.nativeUnproject(h, 128.0, 128.0)
                NativeSurfaceMap.nativeProject(h, 60.39, 5.32)
                NativeSurfaceMap.nativeCamera(h)
            }
            var z = 9.0
            val camT = loop {
                NativeSurfaceMap.nativeSetCamera(h, 60.39, 5.32, z, 0.0)
                z = if (z > 15.0) 9.0 else z + 0.25
            }

            Thread.sleep(1500)
            stop.set(true)
            renderT.join(2000)
            projT.join(2000)
            camT.join(2000)

            assertTrue("no exceptions under concurrent access: $errors", errors.isEmpty())
            // Projection still works after the storm (and reports no engine error).
            val r = NativeSurfaceMap.nativeUnproject(handle, 128.0, 128.0)
            assertEquals("centre pixel unprojects validly", 1.0, r[2], 0.0)
            org.junit.Assert.assertNull(NativeSurfaceMap.nativeLastError())
        } finally {
            if (handle != 0L) NativeSurfaceMap.nativeDestroy(handle)
            reader.close()
        }
    }
}
