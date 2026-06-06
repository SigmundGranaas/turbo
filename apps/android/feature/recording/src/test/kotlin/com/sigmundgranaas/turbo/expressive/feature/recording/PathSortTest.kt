package com.sigmundgranaas.turbo.expressive.feature.recording

import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import org.junit.Assert.assertEquals
import org.junit.Test

class PathSortTest {

    private fun path(id: String, name: String, distance: Double, recordedAt: Long) = SavedPath(
        id = id,
        name = name,
        path = GeoPath(
            points = listOf(LatLng(0.0, 0.0), LatLng(0.0, 1.0)),
            source = GeoPathSource.Recording,
            distanceM = distance,
            recordedAtEpochMs = recordedAt,
        ),
    )

    private val paths = listOf(
        path("1", "Besseggen", 13000.0, 100L),
        path("2", "Aurlandsdalen", 5000.0, 300L),
        path("3", "Galdhøpiggen", 9000.0, 200L),
    )

    @Test
    fun `newest first by recorded timestamp`() {
        val order = sortAndFilterPaths(paths, "", PathSort.Newest).map { it.id }
        assertEquals(listOf("2", "3", "1"), order)
    }

    @Test
    fun `name sorts alphabetically case-insensitively`() {
        val order = sortAndFilterPaths(paths, "", PathSort.Name).map { it.name }
        assertEquals(listOf("Aurlandsdalen", "Besseggen", "Galdhøpiggen"), order)
    }

    @Test
    fun `longest first by distance`() {
        val order = sortAndFilterPaths(paths, "", PathSort.Longest).map { it.id }
        assertEquals(listOf("1", "3", "2"), order)
    }

    @Test
    fun `query filters by name substring`() {
        val result = sortAndFilterPaths(paths, "dal", PathSort.Newest)
        assertEquals(listOf("Aurlandsdalen"), result.map { it.name })
    }
}
