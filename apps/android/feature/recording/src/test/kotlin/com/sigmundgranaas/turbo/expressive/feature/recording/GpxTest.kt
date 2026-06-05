package com.sigmundgranaas.turbo.expressive.feature.recording

import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class GpxTest {

    private fun path(elevations: List<Double?>? = null, name: String = "My Hike") = SavedPath(
        id = "p1",
        name = name,
        path = GeoPath(
            points = listOf(LatLng(69.0, 18.0), LatLng(69.001, 18.001)),
            source = GeoPathSource.Recording,
            elevations = elevations,
        ),
    )

    @Test
    fun `emits valid gpx with a named track and a point per coordinate`() {
        val gpx = pathToGpx(path())
        assertTrue(gpx.startsWith("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"))
        assertTrue(gpx.contains("<gpx version=\"1.1\""))
        assertTrue(gpx.contains("<name>My Hike</name>"))
        assertEquals(2, Regex("<trkpt ").findAll(gpx).count())
        assertTrue(gpx.contains("lat=\"69.0\" lon=\"18.0\""))
    }

    @Test
    fun `includes ele only when elevation is present`() {
        assertTrue(pathToGpx(path(elevations = listOf(12.0, 40.0))).contains("<ele>12.0</ele>"))
        assertFalse(pathToGpx(path(elevations = null)).contains("<ele>"))
    }

    @Test
    fun `escapes xml-significant characters in the name`() {
        assertTrue(pathToGpx(path(name = "A & B <hike>")).contains("<name>A &amp; B &lt;hike&gt;</name>"))
    }

    @Test
    fun `gpxFileName sanitises to a safe stem`() {
        assertEquals("My_Hike.gpx", gpxFileName("My Hike"))
        assertEquals("track.gpx", gpxFileName("   "))
    }
}
