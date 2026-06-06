package com.sigmundgranaas.turbo.expressive.core.data

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RecordingFilterTest {

    @Test
    fun `accepts fixes with unknown or good accuracy`() {
        assertTrue(RecordingFilter.acceptAccuracy(null))
        assertTrue(RecordingFilter.acceptAccuracy(5.0))
        assertTrue(RecordingFilter.acceptAccuracy(RecordingFilter.MAX_ACCURACY_M))
    }

    @Test
    fun `rejects fixes worse than the accuracy ceiling`() {
        assertFalse(RecordingFilter.acceptAccuracy(RecordingFilter.MAX_ACCURACY_M + 0.1))
        assertFalse(RecordingFilter.acceptAccuracy(500.0))
    }
}
