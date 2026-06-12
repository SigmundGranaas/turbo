package com.sigmundgranaas.turbo.expressive.feature.map.live

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class WaveAmplitudeTest {

    @Test
    fun `at rest the wave has a small non-zero floor`() {
        assertEquals(0.1f, waveAmplitudeForSpeed(0.0), 1e-4f)
        assertEquals(0.1f, waveAmplitudeForSpeed(null), 1e-4f)
    }

    @Test
    fun `amplitude grows with speed but stays calm (well under 1f)`() {
        val slow = waveAmplitudeForSpeed(1.0)
        val brisk = waveAmplitudeForSpeed(6.0)
        assertTrue("faster should wave more", brisk > slow)
        assertTrue("never thrashes", brisk <= 0.55f)
    }

    @Test
    fun `very fast clamps to the calm cap`() {
        assertEquals(0.55f, waveAmplitudeForSpeed(50.0), 1e-4f)
    }
}
