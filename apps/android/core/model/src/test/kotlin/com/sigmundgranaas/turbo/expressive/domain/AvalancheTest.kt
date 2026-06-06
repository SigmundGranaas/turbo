package com.sigmundgranaas.turbo.expressive.domain

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AvalancheTest {

    @Test
    fun `level 1 is always suppressed`() {
        assertFalse(shouldShowAvalanche(1, airTempC = -10.0))
        assertFalse(shouldShowAvalanche(0, airTempC = null))
    }

    @Test
    fun `level 2 shows only when cold`() {
        assertTrue(shouldShowAvalanche(2, airTempC = -2.0))
        assertTrue(shouldShowAvalanche(2, airTempC = 5.0)) // boundary inclusive
        assertFalse(shouldShowAvalanche(2, airTempC = 12.0))
        assertTrue(shouldShowAvalanche(2, airTempC = null)) // unknown temp → don't hide
    }

    @Test
    fun `level 3 and above always show`() {
        assertTrue(shouldShowAvalanche(3, airTempC = 20.0))
        assertTrue(shouldShowAvalanche(5, airTempC = 30.0))
    }
}
