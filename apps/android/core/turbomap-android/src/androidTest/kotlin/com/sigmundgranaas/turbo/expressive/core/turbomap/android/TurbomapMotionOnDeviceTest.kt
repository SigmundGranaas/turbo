package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Motion M1 gate: the on-screen path now drives the engine's camera physics.
 * `nativeRender` ticks the active animation, so a fling advances the camera over
 * frames and decelerates to a stop; `nativeCancelAnimation` freezes it where it
 * is (the finger-down catch). Drives the real engine on the device GPU.
 */
@RunWith(AndroidJUnit4::class)
class TurbomapMotionOnDeviceTest {

    private fun create(): Pair<Long, ImageReader> {
        val reader = ImageReader.newInstance(
            256, 256, PixelFormat.RGBA_8888, 3,
            HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN,
        )
        val handle = NativeSurfaceMap.nativeCreate(reader.surface, 256, 256, 60.39, 5.32, 9.0)
        assertNotEquals("surface map should be created", 0L, handle)
        return handle to reader
    }

    /** Render [frames] frames with a real delay so the time-based physics advances. */
    private fun pump(handle: Long, frames: Int, stepMs: Long = 20) {
        repeat(frames) {
            NativeSurfaceMap.nativeRender(handle)
            Thread.sleep(stepMs)
        }
    }

    @Test
    fun fling_advances_the_camera_then_decelerates_to_a_stop() {
        val (handle, reader) = create()
        try {
            val before = NativeSurfaceMap.nativeCamera(handle)
            NativeSurfaceMap.nativeFling(handle, 1200.0, 600.0)
            assertTrue("a fresh fling is animating", NativeSurfaceMap.nativeIsAnimating(handle))

            pump(handle, frames = 12) // ~240 ms of glide
            val mid = NativeSurfaceMap.nativeCamera(handle)
            assertTrue(
                "the fling moved the camera",
                before[0] != mid[0] || before[1] != mid[1],
            )

            // Pump until it settles (bounded). FlingAnimation converges in ~1 s.
            var settled = false
            for (i in 0 until 120) {
                NativeSurfaceMap.nativeRender(handle)
                Thread.sleep(20)
                if (!NativeSurfaceMap.nativeIsAnimating(handle)) {
                    settled = true
                    break
                }
            }
            assertTrue("the fling decelerates and stops", settled)

            // Once stopped, the camera holds still.
            val rest = NativeSurfaceMap.nativeCamera(handle)
            pump(handle, frames = 4)
            val later = NativeSurfaceMap.nativeCamera(handle)
            assertTrue("camera is at rest after settling", rest[0] == later[0] && rest[1] == later[1])
        } finally {
            NativeSurfaceMap.nativeDestroy(handle)
            reader.close()
        }
    }

    @Test
    fun cancel_freezes_an_in_flight_fling() {
        val (handle, reader) = create()
        try {
            NativeSurfaceMap.nativeFling(handle, 1500.0, 0.0)
            pump(handle, frames = 4)
            NativeSurfaceMap.nativeCancelAnimation(handle)
            assertTrue("cancel stops the animation", !NativeSurfaceMap.nativeIsAnimating(handle))

            val caught = NativeSurfaceMap.nativeCamera(handle)
            pump(handle, frames = 6)
            val after = NativeSurfaceMap.nativeCamera(handle)
            assertTrue("cancelled camera stays put", caught[0] == after[0] && caught[1] == after[1])
        } finally {
            NativeSurfaceMap.nativeDestroy(handle)
            reader.close()
        }
    }

    @Test
    fun ease_to_animates_to_the_target_then_settles() {
        val (handle, reader) = create()
        try {
            NativeSurfaceMap.nativeEaseTo(handle, 61.5, 6.5, 11.0, 0.0, 300)
            assertTrue("ease is animating", NativeSurfaceMap.nativeIsAnimating(handle))
            var settled = false
            for (i in 0 until 60) {
                NativeSurfaceMap.nativeRender(handle)
                Thread.sleep(20)
                if (!NativeSurfaceMap.nativeIsAnimating(handle)) {
                    settled = true
                    break
                }
            }
            assertTrue("ease settles", settled)
            val c = NativeSurfaceMap.nativeCamera(handle)
            assertTrue("eased to target lat", kotlin.math.abs(c[0] - 61.5) < 0.01)
            assertTrue("eased to target zoom", kotlin.math.abs(c[2] - 11.0) < 0.01)
        } finally {
            NativeSurfaceMap.nativeDestroy(handle)
            reader.close()
        }
    }
}
