package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.shouldShowAvalanche
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SyntheticConditionsRepositoryTest {

    private val repo = SyntheticConditionsRepository()
    private val tromso = LatLng(69.65, 18.96)

    @Test
    fun `forPoint returns weather, a showable avalanche card and marine`() = runTest {
        val c = (repo.forPoint(tromso) as Outcome.Success).value
        assertNotNull(c.weather)
        assertNotNull(c.marine)
        assertNotNull(c.avalanche)
        // Level is high enough that the UI's suppression heuristic keeps it.
        assertTrue(shouldShowAvalanche(c.avalanche!!.dangerLevel, c.weather?.temperatureC))
        assertTrue(c.avalanche!!.problems.isNotEmpty())
    }

    @Test
    fun `forecast yields a multi-day hourly series with daily rollups`() = runTest {
        val f = (repo.forecast(tromso) as Outcome.Success).value
        assertEquals(72, f.points.size)
        assertTrue("expected ≥3 days, got ${f.days.size}", f.days.size >= 3)
        // Points are time-ordered and carry a symbol the forecast UI can render.
        assertTrue(f.points.zipWithNext().all { (a, b) -> a.timeIso <= b.timeIso })
        assertTrue(f.points.all { it.symbol1h != null })
    }

    @Test
    fun `colder the further north`() = runTest {
        val north = (repo.forPoint(LatLng(70.0, 18.0)) as Outcome.Success).value.weather!!.temperatureC!!
        val south = (repo.forPoint(LatLng(60.0, 10.0)) as Outcome.Success).value.weather!!.temperatureC!!
        assertTrue("north ($north) should be colder than south ($south)", north < south)
    }
}
