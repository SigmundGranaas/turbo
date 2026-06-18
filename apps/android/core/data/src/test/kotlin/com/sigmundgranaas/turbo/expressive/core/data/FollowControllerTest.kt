package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class FollowControllerTest {

    private class FakeLocation(var permitted: Boolean = true) : LocationRepository {
        val feed = MutableSharedFlow<LocationSample>(extraBufferCapacity = 16)
        override fun hasPermission(): Boolean = permitted
        override fun samples(): Flow<LocationSample> = feed
        suspend fun emit(lat: Double, lng: Double, speed: Double? = null) =
            feed.emit(LocationSample(LatLng(lat, lng), altitude = null, speedMps = speed))
    }

    private val plan = RoutePlan(
        distanceM = 0.0, durationS = 0.0, ascentM = 100.0, onTrailPct = 90.0, surfaces = emptyMap(),
        // ~5.5 km straight north from 69.00 to 69.05.
        geometry = listOf(LatLng(69.00, 18.0), LatLng(69.05, 18.0)),
    )

    @Test
    fun `start projects live position onto the route as progress`() = runTest {
        val loc = FakeLocation()
        val controller = FollowController(loc, backgroundScope)
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
        val controller = FollowController(loc, backgroundScope)
        controller.start(plan)
        runCurrent()
        for (i in 0..16) { loc.emit(69.00 + i * 0.003, 18.0); runCurrent() } // ~69.048
        loc.emit(69.05, 18.0); runCurrent() // the end
        assertTrue(controller.session.value.arrived)
    }

    @Test
    fun `stop clears the session`() = runTest {
        val loc = FakeLocation()
        val controller = FollowController(loc, backgroundScope)
        controller.start(plan)
        runCurrent()
        controller.stop()
        assertFalse(controller.session.value.active)
        assertEquals(null, controller.session.value.plan)
    }

    @Test
    fun `holds the plan even without location permission`() = runTest {
        val loc = FakeLocation(permitted = false)
        val controller = FollowController(loc, backgroundScope)
        controller.start(plan)
        runCurrent()
        assertTrue(controller.session.value.active)
        assertEquals(null, controller.session.value.progress)
    }
}
