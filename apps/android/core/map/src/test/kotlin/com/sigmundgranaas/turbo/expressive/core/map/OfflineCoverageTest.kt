package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class OfflineCoverageTest {

    private fun region(status: OfflineStatus, bounds: GeoBounds?) = OfflineRegionInfo(
        id = 1, name = "r", status = status, progress = 1f, sizeBytes = 1, bounds = bounds,
    )

    private val tromso = GeoBounds(south = 69.6, west = 18.9, north = 69.7, east = 19.1)

    @Test
    fun `a point inside a complete region is covered`() {
        assertTrue(OfflineCoverage.covers(listOf(region(OfflineStatus.Complete, tromso)), LatLng(69.65, 19.0)))
    }

    @Test
    fun `a point outside every region is not covered`() {
        assertFalse(OfflineCoverage.covers(listOf(region(OfflineStatus.Complete, tromso)), LatLng(60.0, 10.0)))
    }

    @Test
    fun `incomplete or legacy regions never cover`() {
        assertFalse(OfflineCoverage.covers(listOf(region(OfflineStatus.Downloading, tromso)), LatLng(69.65, 19.0)))
        assertFalse(OfflineCoverage.covers(listOf(region(OfflineStatus.Complete, null)), LatLng(69.65, 19.0)))
    }
}
