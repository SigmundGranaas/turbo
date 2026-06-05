package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

private class FakeLocationRepository(var permitted: Boolean = true) : LocationRepository {
    val fixes = MutableSharedFlow<LatLng>(extraBufferCapacity = 32)
    override fun hasPermission(): Boolean = permitted
    override fun locationUpdates(): Flow<LatLng> = fixes
}

@OptIn(ExperimentalCoroutinesApi::class)
class RecordingControllerTest {

    @Test
    fun `accumulates distance across fixes beyond the min step`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, backgroundScope)
        controller.start()
        runCurrent()

        loc.fixes.emit(LatLng(69.0000, 18.0)); runCurrent()
        loc.fixes.emit(LatLng(69.0010, 18.0)); runCurrent() // ~111 m north

        val session = controller.session.value
        assertEquals(2, session.points.size)
        assertTrue("distance ~111m but was ${session.distanceM}", session.distanceM in 100.0..125.0)
    }

    @Test
    fun `ignores jitter below the min step`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, backgroundScope)
        controller.start()
        runCurrent()

        loc.fixes.emit(LatLng(69.0, 18.0)); runCurrent()
        loc.fixes.emit(LatLng(69.000005, 18.0)); runCurrent() // < 1 m

        val session = controller.session.value
        assertEquals(1, session.points.size)
        assertEquals(0.0, session.distanceM, 1e-9)
    }

    @Test
    fun `paused recording drops new fixes`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, backgroundScope)
        controller.start()
        runCurrent()
        loc.fixes.emit(LatLng(69.0, 18.0)); runCurrent()
        controller.togglePause()
        loc.fixes.emit(LatLng(69.001, 18.0)); runCurrent()

        assertEquals(1, controller.session.value.points.size)
        assertTrue(controller.session.value.paused)
    }

    @Test
    fun `timer advances elapsed seconds while active`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, backgroundScope)
        controller.start()
        advanceTimeBy(3_100)
        runCurrent()
        assertEquals(3, controller.session.value.elapsedSec)
    }

    @Test
    fun `reset clears the session`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, backgroundScope)
        controller.start()
        runCurrent()
        loc.fixes.emit(LatLng(69.0, 18.0)); runCurrent()
        controller.reset()
        assertEquals(0, controller.session.value.points.size)
        assertFalse(controller.session.value.active)
    }

    @Test
    fun `start is a no-op without permission`() = runTest {
        val loc = FakeLocationRepository(permitted = false)
        val controller = RecordingController(loc, backgroundScope)
        controller.start()
        runCurrent()
        assertFalse(controller.session.value.active)
    }
}
