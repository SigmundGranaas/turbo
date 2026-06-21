package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.MapEngine
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Stage-E keystone: the wgpu engine satisfies the **same `MapEngine` contract**
 * feature code uses — the whole point of the renderer-agnostic seam. Runs the
 * contract against [TurbomapMapEngine] on the device (camera, projection,
 * visible box), so a second engine is proven to slot behind the seam.
 */
@RunWith(AndroidJUnit4::class)
class TurbomapMapEngineContractTest {

    private val size = 256

    private inline fun withEngine(block: (engine: MapEngine, pump: () -> Unit) -> Unit) {
        val reader = ImageReader.newInstance(
            size, size, PixelFormat.RGBA_8888, 2,
            HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN,
        )
        var handle = 0L
        try {
            handle = NativeSurfaceMap.nativeCreate(reader.surface, size, size, 60.39, 5.32, 9.0)
            assertNotEquals("surface map should be created", 0L, handle)
            // Programmatic moves (flyTo/zoom/...) now EASE, and the FFI applies
            // every mutation on the NEXT render (wait-free command queue, not a
            // synchronous lock). So render once unconditionally to DRAIN the
            // queued command (start the ease / apply an instant move), then keep
            // rendering until the animation settles — only then do the contract's
            // final-state assertions hold.
            val pump: () -> Unit = {
                NativeSurfaceMap.nativeRender(handle)
                var i = 0
                while (i < 80 && NativeSurfaceMap.nativeIsAnimating(handle)) {
                    NativeSurfaceMap.nativeRender(handle)
                    Thread.sleep(16)
                    i++
                }
            }
            block(TurbomapMapEngine(handle, size, size), pump)
        } finally {
            if (handle != 0L) NativeSurfaceMap.nativeDestroy(handle)
            reader.close()
        }
    }

    @Test
    fun camera_moves_are_observable() = withEngine { engine, pump ->
        engine.flyTo(LatLng(61.0, 6.0), 11.0)
        pump()
        assertEquals(61.0, engine.center().lat, 1e-3)
        assertEquals(6.0, engine.center().lng, 1e-3)
        assertEquals(11.0, engine.zoom(), 1e-3)
        assertEquals(0.0, engine.bearing(), 1e-9)

        engine.zoomIn()
        pump()
        assertEquals(12.0, engine.zoom(), 1e-3)
        engine.zoomOut()
        pump()
        assertEquals(11.0, engine.zoom(), 1e-3)
    }

    @Test
    fun project_then_unproject_is_identity() = withEngine { engine, pump ->
        engine.flyTo(LatLng(61.0, 6.0), 11.0)
        pump()
        val geo = LatLng(61.01, 6.02)
        val (x, y) = engine.toScreen(geo)
        val back = engine.fromScreen(x, y)
        assertEquals("lat round-trip drifted", geo.lat, back.lat, 1e-4)
        assertEquals("lng round-trip drifted", geo.lng, back.lng, 1e-4)
    }

    @Test
    fun bottom_inset_lifts_the_centred_target_into_the_visible_band() = withEngine { engine, pump ->
        val target = LatLng(61.0, 6.0)
        engine.flyTo(target, 11.0)
        pump()
        val (_, yNoInset) = engine.toScreen(target)
        // Reserve the bottom half for a sheet. This is engine-side padding: the
        // projection itself shifts the principal point up, so the *already-centred*
        // target lifts into the visible band with no re-centring (overlays follow).
        engine.setBottomInset(size / 2)
        pump() // drain the queued inset (applied on the next frame) before projecting
        val (_, yInset) = engine.toScreen(target)
        assertTrue("inset should move the centred target up the screen ($yInset !< $yNoInset)", yInset < yNoInset)
        // The lift is ~half the reserved band, and projection stays invertible.
        val back = engine.fromScreen(size / 2f, yInset)
        assertEquals("lat round-trip under inset", target.lat, back.lat, 1e-3)
        assertEquals("lng round-trip under inset", target.lng, back.lng, 1e-3)
    }

    @Test
    fun visible_bounds_contain_the_centre() = withEngine { engine, pump ->
        engine.flyTo(LatLng(61.0, 6.0), 11.0)
        pump()
        val b = engine.visibleBounds()
        assertTrue("south<north: $b", b.south < b.north)
        assertTrue("west<east: $b", b.west < b.east)
        assertTrue("centre lat inside: $b", b.south < 61.0 && 61.0 < b.north)
        assertTrue("centre lng inside: $b", b.west < 6.0 && 6.0 < b.east)
    }
}
