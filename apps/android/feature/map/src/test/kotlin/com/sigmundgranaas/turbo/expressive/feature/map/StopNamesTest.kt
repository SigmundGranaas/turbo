package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.LocationDescription
import com.sigmundgranaas.turbo.expressive.domain.PlaceQualifier
import com.sigmundgranaas.turbo.expressive.feature.map.route.StopLabels
import com.sigmundgranaas.turbo.expressive.feature.map.route.StopNames
import com.sigmundgranaas.turbo.expressive.feature.map.route.StopPalette
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** A scriptable reverse-geocoder: returns [next] as the stop's title (or fails), counting calls. */
private class FakeGeocoder(var next: LocationDescription?) : ReverseGeocodeRepository {
    var calls = 0
    override suspend fun describe(point: LatLng): Outcome<LocationDescription> {
        calls++
        return next?.let { Outcome.Success(it) } ?: Outcome.Failure(IllegalStateException("offline"))
    }
}

private fun desc(title: String) = LocationDescription(title, PlaceQualifier.On)

class StopNamesTest {

    private val p = LatLng(69.9607, 23.2715)

    @Test
    fun `trimmed coords are plain comma-separated decimals`() {
        assertEquals("69.9607, 23.2715", StopLabels.trimmedCoords(p))
    }

    @Test
    fun `label shows the cached name when present, else the trimmed coords`() {
        assertEquals("Besseggen", StopLabels.label("Besseggen", p))
        assertEquals(StopLabels.trimmedCoords(p), StopLabels.label(null, p))
        assertEquals(StopLabels.trimmedCoords(p), StopLabels.label("  ", p)) // blank name ignored
    }

    @Test
    fun `resolve returns the geocoded name when online`() = runTest {
        val names = StopNames(FakeGeocoder(desc("Galdhøpiggen")))
        assertEquals("Galdhøpiggen", names.resolve(p))
    }

    @Test
    fun `offline resolve yields null and never throws (row falls back to coords)`() = runTest {
        val names = StopNames(FakeGeocoder(next = null))
        assertNull(names.resolve(p))
        // The display rule then shows coordinates — no exception, no blank row.
        assertEquals(StopLabels.trimmedCoords(p), StopLabels.label(names.cached(p), p))
    }

    @Test
    fun `a resolved name is cached — a re-render does not geocode again`() = runTest {
        val geo = FakeGeocoder(desc("Besseggen"))
        val names = StopNames(geo)
        assertEquals("Besseggen", names.resolve(p))
        assertEquals(1, geo.calls)
        // The place "changes name" upstream, but the cached value stands (no second fetch).
        geo.next = desc("Somewhere else")
        assertEquals("Besseggen", names.resolve(p))
        assertEquals("no second geocode for the same cell", 1, geo.calls)
        assertEquals("Besseggen", names.cached(p))
    }

    @Test
    fun `a via keeps its palette colour regardless of list position`() {
        // Colour is keyed on the coordinate's grid cell, so reordering the stops around it
        // never changes the colour the user sees for that stop.
        val before = StopPalette.colorOf(p)
        val after = StopPalette.colorOf(LatLng(p.lat, p.lng)) // same place, different object
        assertEquals(before, after)
    }

    @Test
    fun `distinct places generally get distinct palette colours`() {
        val c1 = StopPalette.colorOf(LatLng(69.0, 18.0))
        val c2 = StopPalette.colorOf(LatLng(60.0, 10.0))
        assertNotEquals(c1, c2)
    }
}
