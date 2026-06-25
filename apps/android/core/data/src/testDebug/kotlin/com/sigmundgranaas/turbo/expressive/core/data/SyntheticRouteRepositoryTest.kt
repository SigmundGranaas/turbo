package com.sigmundgranaas.turbo.expressive.core.data

import app.cash.turbine.test
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import com.sigmundgranaas.turbo.expressive.domain.RouteStreamEvent
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class SyntheticRouteRepositoryTest {

    private val repo = SyntheticRouteRepository()

    @Test
    fun `streams a progress snapshot then a densified result through the waypoints`() = runTest {
        val a = LatLng(69.60, 18.90)
        val b = LatLng(69.65, 18.90) // ~5.5 km north
        repo.planStream(listOf(a, b), RoutePreset.Balanced, "foot").test {
            val progress = awaitItem()
            assertTrue(progress is RouteStreamEvent.Progress)

            val result = awaitItem() as RouteStreamEvent.Result
            val geo = result.plan.geometry
            // Densified: many intermediate points, endpoints preserved, order kept.
            assertTrue("expected a densified line, got ${geo.size}", geo.size > 10)
            assertEquals(a, geo.first())
            assertEquals(b, geo.last())
            assertTrue(result.plan.distanceM > 4_000.0)
            awaitComplete()
        }
    }

    @Test
    fun `fails clearly with fewer than two points`() = runTest {
        repo.planStream(listOf(LatLng(69.0, 18.0)), RoutePreset.Balanced, "foot").test {
            assertTrue(awaitItem() is RouteStreamEvent.Failure)
            awaitComplete()
        }
    }
}
