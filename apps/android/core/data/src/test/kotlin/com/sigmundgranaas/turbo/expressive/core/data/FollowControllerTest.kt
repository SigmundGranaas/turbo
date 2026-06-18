package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class FollowControllerTest {

    private class FakeLocation(var permitted: Boolean = true) : LocationRepository {
        val feed = MutableSharedFlow<LocationSample>(extraBufferCapacity = 64)
        override fun hasPermission(): Boolean = permitted
        override fun samples(): Flow<LocationSample> = feed
        suspend fun emit(lat: Double, lng: Double, speed: Double? = null, altitude: Double? = null) =
            feed.emit(LocationSample(LatLng(lat, lng), altitude = altitude, speedMps = speed))
    }

    /** Captures auto-saved tracks so the Follow = Record + D1 auto-save can be asserted. */
    private class FakePaths : PathRepository {
        val saved = mutableListOf<SavedPath>()
        override fun observeAll() = flowOf(emptyList<SavedPath>())
        override suspend fun byId(id: String): SavedPath? = saved.firstOrNull { it.id == id }
        override suspend fun save(path: SavedPath) { saved += path }
        override suspend fun delete(id: String) { saved.removeAll { it.id == id } }
        override suspend fun remoteId(id: String): String? = null
    }

    private val plan = RoutePlan(
        distanceM = 0.0, durationS = 0.0, ascentM = 100.0, onTrailPct = 90.0, surfaces = emptyMap(),
        // ~5.5 km straight north from 69.00 to 69.05.
        geometry = listOf(LatLng(69.00, 18.0), LatLng(69.05, 18.0)),
    )

    @Test
    fun `start projects live position onto the route as progress`() = runTest {
        val loc = FakeLocation()
        val controller = FollowController(loc, FakePaths(), backgroundScope)
        controller.start(plan, name = "Skåla Loop")
        runCurrent()
        assertTrue(controller.session.value.active)
        assertEquals("Skåla Loop", controller.session.value.name)

        // The cursor must be WALKED — feed fixes in ~333 m steps to the midpoint.
        for (i in 0..8) { loc.emit(69.00 + i * 0.003, 18.0, speed = 2.5); runCurrent() }
        val p = controller.session.value.progress!!
        assertTrue("fraction ~0.5 but was ${p.fraction}", p.fraction in 0.4..0.6)
        assertEquals(2.5, controller.session.value.speedMps!!, 1e-6)
    }

    @Test
    fun `arrived flips true after walking to the end`() = runTest {
        val loc = FakeLocation()
        val controller = FollowController(loc, FakePaths(), backgroundScope)
        controller.start(plan)
        runCurrent()
        for (i in 0..16) { loc.emit(69.00 + i * 0.003, 18.0); runCurrent() } // ~69.048
        loc.emit(69.05, 18.0); runCurrent() // the end
        assertTrue(controller.session.value.arrived)
    }

    @Test
    fun `stop clears the session`() = runTest {
        val loc = FakeLocation()
        val controller = FollowController(loc, FakePaths(), backgroundScope)
        controller.start(plan)
        runCurrent()
        controller.stop()
        assertFalse(controller.session.value.active)
        assertEquals(null, controller.session.value.plan)
    }

    @Test
    fun `holds the plan even without location permission`() = runTest {
        val loc = FakeLocation(permitted = false)
        val controller = FollowController(loc, FakePaths(), backgroundScope)
        controller.start(plan)
        runCurrent()
        assertTrue(controller.session.value.active)
        assertEquals(null, controller.session.value.progress)
    }

    @Test
    fun `following captures the real travelled track (Follow = Record)`() = runTest {
        val loc = FakeLocation()
        val controller = FollowController(loc, FakePaths(), backgroundScope)
        controller.start(plan)
        runCurrent()
        for (i in 0..8) { loc.emit(69.00 + i * 0.003, 18.0, altitude = 100.0 + i * 10); runCurrent() }
        val s = controller.session.value
        // The travelled polyline + cumulative distance were captured, not just projected.
        assertTrue("captured points but was ${s.points.size}", s.points.size >= 8)
        assertTrue("captured distance but was ${s.capturedDistanceM}", s.capturedDistanceM > 2_000.0)
    }

    @Test
    fun `finishing a real follow auto-saves the travelled track`() = runTest {
        val loc = FakeLocation()
        val paths = FakePaths()
        val controller = FollowController(loc, paths, backgroundScope)
        controller.start(plan, name = "Skåla Loop")
        runCurrent()
        for (i in 0..8) { loc.emit(69.00 + i * 0.003, 18.0, altitude = 100.0 + i * 10); runCurrent() }
        controller.stop()
        runCurrent()
        assertEquals(1, paths.saved.size)
        val track = paths.saved.single()
        assertTrue("name carries the route", track.name.contains("Skåla Loop"))
        assertTrue("a real polyline was saved", track.path.points.size >= 8)
    }

    @Test
    fun `crossing a checkpoint logs a split (US-3)`() = runTest {
        val loc = FakeLocation()
        val controller = FollowController(loc, FakePaths(), backgroundScope)
        // A checkpoint at the route midpoint, plus the end.
        controller.start(
            plan, name = "Skåla Loop",
            phasePoints = listOf(LatLng(69.025, 18.0), LatLng(69.05, 18.0)),
            phaseNames = listOf("B", "C"),
        )
        runCurrent()
        // Walk past the midpoint checkpoint.
        for (i in 0..10) { loc.emit(69.00 + i * 0.003, 18.0); runCurrent() } // 69.00 … 69.03
        val s = controller.session.value
        assertTrue("crossed at least the midpoint checkpoint", s.phaseSplits.isNotEmpty())
        assertEquals("B", s.phaseSplits.first().name)
        assertTrue("split has a distance", s.phaseSplits.first().splitDistanceM > 0.0)
        assertEquals("next checkpoint is C", "C", s.nextPhaseName)
    }

    @Test
    fun `pausing a follow buffers the walk instead of advancing the track (US-4)`() = runTest {
        val loc = FakeLocation()
        val controller = FollowController(loc, FakePaths(), backgroundScope)
        controller.start(plan)
        runCurrent()
        loc.emit(69.00, 18.0); runCurrent()
        controller.pause()
        loc.emit(69.003, 18.0); runCurrent() // ~333 m walked while paused
        val s = controller.session.value
        assertTrue(s.paused)
        assertEquals("track frozen while paused", 1, s.points.size)
        assertTrue("buffered the paused walk: ${s.bufferedDistanceM}", s.bufferedDistanceM > 90.0)
        assertTrue(s.hasBufferedMovement)
    }

    @Test
    fun `resuming a follow with include stitches the paused walk onto the track`() = runTest {
        val loc = FakeLocation()
        val controller = FollowController(loc, FakePaths(), backgroundScope)
        controller.start(plan)
        runCurrent()
        loc.emit(69.00, 18.0); runCurrent()
        controller.pause()
        loc.emit(69.003, 18.0); runCurrent()
        loc.emit(69.006, 18.0); runCurrent()
        controller.resume(includeBuffered = true)
        runCurrent()
        val s = controller.session.value
        assertFalse(s.paused)
        assertEquals(3, s.points.size)
        assertEquals(0.0, s.bufferedDistanceM, 1e-9)
        assertTrue("counts the paused walk: ${s.capturedDistanceM}", s.capturedDistanceM > 500.0)
    }

    @Test
    fun `a trivially short follow is not auto-saved`() = runTest {
        val loc = FakeLocation()
        val paths = FakePaths()
        val controller = FollowController(loc, paths, backgroundScope)
        controller.start(plan)
        runCurrent()
        // Two fixes ~33 m apart — under the 50 m save floor.
        loc.emit(69.00, 18.0); runCurrent()
        loc.emit(69.0003, 18.0); runCurrent()
        controller.stop()
        runCurrent()
        assertTrue("nothing saved for a trivial follow", paths.saved.isEmpty())
    }
}
