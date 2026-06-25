package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.PlaceQualifier
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SyntheticSearchTest {

    @Test
    fun `place search echoes the query into named hits and ignores too-short input`() = runTest {
        val repo = SyntheticSearchRepository()
        assertTrue((repo.search("a") as Outcome.Success).value.isEmpty())
        val hits = (repo.search("stor") as Outcome.Success).value
        assertTrue(hits.size >= 3)
        assertTrue(hits.all { it.name.startsWith("Stor", ignoreCase = true) })
        // Distinct, real coordinates so each can be focused on the map.
        assertEquals(hits.size, hits.map { it.position }.toSet().size)
    }

    @Test
    fun `trail search returns named routes for the query`() = runTest {
        val repo = SyntheticTrailSearchRepository()
        val hits = (repo.search("skåla") as Outcome.Success).value
        assertTrue(hits.isNotEmpty())
        assertTrue(hits.first().name.contains("Skåla", ignoreCase = true))
        assertTrue(hits.all { it.description.contains("km") })
    }

    @Test
    fun `reverse-geocode names a coordinate deterministically`() = runTest {
        val repo = SyntheticReverseGeocodeRepository()
        val a = (repo.describe(LatLng(69.65, 18.95)) as Outcome.Success).value
        val b = (repo.describe(LatLng(69.65, 18.95)) as Outcome.Success).value
        assertEquals(a, b) // same coordinate → same description (cache-friendly)
        assertNotNull(a.elevationM)
        assertTrue(a.qualifier == PlaceQualifier.On || a.qualifier == PlaceQualifier.In)
        assertTrue(a.label.isNotBlank())
    }
}
