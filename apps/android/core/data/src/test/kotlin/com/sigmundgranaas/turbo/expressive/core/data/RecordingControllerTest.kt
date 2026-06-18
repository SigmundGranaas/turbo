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
    val feed = MutableSharedFlow<LocationSample>(extraBufferCapacity = 32)
    override fun hasPermission(): Boolean = permitted
    override fun samples(): Flow<LocationSample> = feed
    suspend fun emit(lat: Double, lng: Double, alt: Double? = null, speed: Double? = null) =
        feed.emit(LocationSample(LatLng(lat, lng), alt, speedMps = speed))
}

private class FakeDraftStore(var draft: RecordingDraft? = null) : RecordingDraftStore {
    var cleared = false
    override suspend fun load(): RecordingDraft? = draft
    override suspend fun save(points: List<LatLng>, elevations: List<Double?>, elapsedSec: Int, pausedFromIndex: Int) {
        draft = RecordingDraft(points, elevations, elapsedSec, pausedFromIndex)
    }
    override suspend fun clear() { draft = null; cleared = true }
}

@OptIn(ExperimentalCoroutinesApi::class)
class RecordingControllerTest {

    @Test
    fun `accumulates distance across fixes beyond the min step`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, FakeDraftStore(), backgroundScope)
        controller.start()
        runCurrent()

        loc.emit(69.0000, 18.0); runCurrent()
        loc.emit(69.0010, 18.0); runCurrent() // ~111 m north

        val session = controller.session.value
        assertEquals(2, session.points.size)
        assertTrue("distance ~111m but was ${session.distanceM}", session.distanceM in 100.0..125.0)
    }

    @Test
    fun `captures altitude into the session elevation track`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, FakeDraftStore(), backgroundScope)
        controller.start()
        runCurrent()

        loc.emit(69.0000, 18.0, alt = 12.0); runCurrent()
        loc.emit(69.0010, 18.0, alt = 40.0); runCurrent()

        val session = controller.session.value
        assertEquals(listOf(12.0, 40.0), session.elevations)
    }

    @Test
    fun `ignores jitter below the min step`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, FakeDraftStore(), backgroundScope)
        controller.start()
        runCurrent()

        loc.emit(69.0, 18.0); runCurrent()
        loc.emit(69.000005, 18.0); runCurrent() // < 1 m

        val session = controller.session.value
        assertEquals(1, session.points.size)
        assertEquals(0.0, session.distanceM, 1e-9)
    }

    @Test
    fun `paused recording buffers new fixes instead of dropping them`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, FakeDraftStore(), backgroundScope)
        controller.start()
        runCurrent()
        loc.emit(69.0, 18.0); runCurrent()
        controller.pause()
        loc.emit(69.001, 18.0); runCurrent() // ~111 m walked while paused

        val s = controller.session.value
        assertEquals("track untouched while paused", 1, s.points.size)
        assertTrue(s.paused)
        assertTrue("buffered the paused walk but was ${s.bufferedDistanceM}", s.bufferedDistanceM > 90.0)
        assertTrue(s.hasBufferedMovement)
    }

    @Test
    fun `resume including the buffer stitches the paused walk onto the track`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, FakeDraftStore(), backgroundScope)
        controller.start()
        runCurrent()
        loc.emit(69.0, 18.0); runCurrent()
        controller.pause()
        loc.emit(69.001, 18.0); runCurrent() // ~111 m walked while paused
        loc.emit(69.002, 18.0); runCurrent() // ~222 m total walked while paused
        controller.resume(includeBuffered = true)
        runCurrent()

        val s = controller.session.value
        assertFalse(s.paused)
        assertEquals("the paused walk is now part of the track", 3, s.points.size)
        assertEquals(0.0, s.bufferedDistanceM, 1e-9)
        assertTrue("distance counts the full paused walk but was ${s.distanceM}", s.distanceM > 200.0)
    }

    @Test
    fun `resume discarding the buffer drops the walk and does not count the gap`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, FakeDraftStore(), backgroundScope)
        controller.start()
        runCurrent()
        loc.emit(69.0, 18.0); runCurrent()
        controller.pause()
        loc.emit(69.001, 18.0); runCurrent() // ~111 m walked while paused (to be discarded)
        controller.resume(includeBuffered = false)
        runCurrent()

        // The discarded walk is gone, and resuming hasn't yet added anything.
        assertEquals(1, controller.session.value.points.size)
        assertEquals(0.0, controller.session.value.distanceM, 1e-9)

        // The first fix after a discard is detached: the gap back to the old point isn't counted.
        loc.emit(69.001, 18.0); runCurrent()
        assertEquals(2, controller.session.value.points.size)
        assertEquals("no gap distance counted", 0.0, controller.session.value.distanceM, 1e-9)
        // …but normal accumulation resumes from there.
        loc.emit(69.002, 18.0); runCurrent()
        assertTrue("walks from the new point count", controller.session.value.distanceM > 90.0)
    }

    @Test
    fun `a kill while paused restores the track and the held buffer`() = runTest {
        val loc = FakeLocationRepository()
        val draft = FakeDraftStore()
        val c1 = RecordingController(loc, draft, backgroundScope)
        c1.start(); runCurrent()
        loc.emit(69.0, 18.0); runCurrent()
        loc.emit(69.001, 18.0); runCurrent() // ~111 m track, 2 points
        c1.pause()
        loc.emit(69.002, 18.0); runCurrent() // ~111 m walked while paused → buffer persisted
        c1.stop() // session goes away, but the draft survives

        // A fresh controller resumes from the same persisted draft.
        val c2 = RecordingController(loc, draft, backgroundScope)
        c2.start(); runCurrent()
        val s = c2.session.value
        assertTrue("restored paused", s.paused)
        assertEquals("track restored without the buffer", 2, s.points.size)
        assertTrue("buffer restored: ${s.bufferedDistanceM}", s.bufferedDistanceM > 90.0)
    }

    @Test
    fun `timer advances elapsed seconds while active`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, FakeDraftStore(), backgroundScope)
        controller.start()
        advanceTimeBy(3_100)
        runCurrent()
        assertEquals(3, controller.session.value.elapsedSec)
    }

    @Test
    fun `tracks latest and peak speed across fixes, even sub-min-step ones`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, FakeDraftStore(), backgroundScope)
        controller.start()
        runCurrent()

        loc.emit(69.0, 18.0, speed = 1.2); runCurrent()
        loc.emit(69.001, 18.0, speed = 3.9); runCurrent() // moved, faster
        loc.emit(69.001002, 18.0, speed = 2.0); runCurrent() // jitter (< min step) but updates speed

        val s = controller.session.value
        assertEquals(2.0, s.speedMps!!, 1e-6)
        assertEquals(3.9, s.maxSpeedMps, 1e-6)
        assertEquals(2, s.points.size) // jitter point dropped from the track
    }

    @Test
    fun `reset clears the session`() = runTest {
        val loc = FakeLocationRepository()
        val controller = RecordingController(loc, FakeDraftStore(), backgroundScope)
        controller.start()
        runCurrent()
        loc.emit(69.0, 18.0); runCurrent()
        controller.reset()
        assertEquals(0, controller.session.value.points.size)
        assertFalse(controller.session.value.active)
    }

    @Test
    fun `start is a no-op without permission`() = runTest {
        val loc = FakeLocationRepository(permitted = false)
        val controller = RecordingController(loc, FakeDraftStore(), backgroundScope)
        controller.start()
        runCurrent()
        assertFalse(controller.session.value.active)
    }

    @Test
    fun `start resumes a persisted draft (process-death recovery)`() = runTest {
        val loc = FakeLocationRepository()
        val draft = FakeDraftStore(
            RecordingDraft(listOf(LatLng(69.0, 18.0), LatLng(69.001, 18.0)), listOf(10.0, 25.0), elapsedSec = 42),
        )
        val controller = RecordingController(loc, draft, backgroundScope)
        controller.start()
        runCurrent()

        val s = controller.session.value
        assertEquals(2, s.points.size)
        assertEquals(listOf(10.0, 25.0), s.elevations)
        assertEquals(42, s.elapsedSec)
        assertTrue(s.distanceM > 90.0)
    }

    @Test
    fun `persists the track as points accumulate and clears on reset`() = runTest {
        val loc = FakeLocationRepository()
        val draft = FakeDraftStore()
        val controller = RecordingController(loc, draft, backgroundScope)
        controller.start()
        runCurrent()
        loc.emit(69.0, 18.0); runCurrent()
        loc.emit(69.001, 18.0); runCurrent()

        assertEquals(2, draft.draft?.points?.size)

        controller.reset()
        runCurrent()
        assertTrue(draft.cleared)
    }
}
