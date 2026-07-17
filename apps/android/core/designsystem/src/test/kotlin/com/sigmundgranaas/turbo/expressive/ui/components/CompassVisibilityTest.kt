package com.sigmundgranaas.turbo.expressive.ui.components

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The compass auto-hide rule (spec Phase 1): hidden when the map is within ~0.5° of north,
 * visible once it's rotated. A pure boundary test — no Compose, no pixels.
 */
class CompassVisibilityTest {

    @Test
    fun `near-north hides the compass`() {
        assertFalse(compassVisible(0f))
        assertFalse(compassVisible(0.3f))
        assertFalse(compassVisible(-0.3f))
    }

    @Test
    fun `a rotated map shows the compass`() {
        assertTrue(compassVisible(15f))
        assertTrue(compassVisible(-90f))
        assertTrue(compassVisible(179f))
    }

    /** Locked + north still hides — a locked map can't drift off north, so nothing to reset. */
    @Test
    fun `just past the half-degree threshold flips to visible`() {
        assertFalse(compassVisible(0.5f))
        assertTrue(compassVisible(0.6f))
    }
}
