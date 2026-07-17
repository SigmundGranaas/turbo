package com.sigmundgranaas.turbo.expressive.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Phase 1 "scene-IR" tests: the layers-sheet 3D + Sun sliders drive the map's
 * environment through a pure reducer, so we can assert the user-facing rules
 * (relief appears, tilt unlocks, the sun moves) without a device or a GPU.
 */
class MapEnvironmentTest {

    @Test fun `3D at zero is flat 2D — no DEM, tilt locked`() {
        val env = mapEnvironment(threeDLevel = 0f, sunLevel = 0f)
        assertFalse("no terrain loaded when flat", env.demPresent)
        assertEquals("no exaggeration when flat", 0f, env.exaggeration, 0f)
        assertFalse("pitch gestures rejected in 2D", env.tiltEnabled)
    }

    @Test fun `3D above zero shows terrain and unlocks tilt`() {
        val env = mapEnvironment(threeDLevel = DEFAULT_3D_DETENT, sunLevel = 0f)
        assertTrue("DEM present in 3D", env.demPresent)
        assertEquals("exaggeration carries the slider value", DEFAULT_3D_DETENT, env.exaggeration, 0f)
        assertTrue("pitch gestures accepted in 3D", env.tiltEnabled)
    }

    @Test fun `sun in 2D lights relief top-down — DEM, relief, and sun, but tilt stays locked`() {
        val env = mapEnvironment(threeDLevel = 0f, sunLevel = 0.5f)
        assertTrue("sun needs the DEM to light relief", env.demPresent)
        assertNotNull_("sun vector present", env.sunHour)
        // "3D seen from the top": the mesh has relief for the sun to shade, but the
        // camera never tilts — that's what keeps it a 2D map.
        assertTrue("relief present so the sun has slopes to light", env.exaggeration > 0f)
        assertFalse("the sun slider must NOT unlock tilt", env.tiltEnabled)
    }

    @Test fun `moving the sun slider moves the sun — decoupled from any camera field`() {
        val a = mapEnvironment(threeDLevel = 0f, sunLevel = 0.25f)
        val b = mapEnvironment(threeDLevel = 0f, sunLevel = 0.75f)
        assertNotEquals("sun position changes with the slider", a.sunHour, b.sunHour)
        // Decoupling is structural: neither environment carries a camera/pitch field to
        // change, and tilt stays locked in both (2D) — the slider can't tilt the view.
        assertFalse(a.tiltEnabled)
        assertFalse(b.tiltEnabled)
    }

    @Test fun `sun off yields no sun vector`() {
        assertNull("no sun vector when the slider is at 0", mapEnvironment(0f, 0f).sunHour)
    }

    @Test fun `3D and sun together — terrain, exaggeration, sun, and tilt`() {
        val env = mapEnvironment(threeDLevel = 4f, sunLevel = 1f)
        assertTrue(env.demPresent)
        assertEquals(4f, env.exaggeration, 0f)
        assertNotNull_("sun on", env.sunHour)
        assertTrue(env.tiltEnabled)
    }

    @Test fun `slider values are clamped to their ranges`() {
        val hi = mapEnvironment(threeDLevel = 99f, sunLevel = 9f)
        assertEquals("3D clamps to the max exaggeration", MAX_3D_EXAGGERATION, hi.exaggeration, 0f)
        assertEquals("sun clamps to 1", 1f, hi.sunLevel, 0f)
        val lo = mapEnvironment(threeDLevel = -5f, sunLevel = -5f)
        assertFalse("negative 3D is flat", lo.demPresent)
        assertEquals(0f, lo.sunLevel, 0f)
    }

    private fun assertNotNull_(msg: String, v: Any?) = assertTrue(msg, v != null)
}
