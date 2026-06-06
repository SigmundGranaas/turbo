package com.sigmundgranaas.turbo.expressive.feature.recording

import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class TrackExportTest {

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
    fun `geojson emits a LineString with lng-lat-ele coordinates`() {
        val json = pathToGeoJson(path(elevations = listOf(12.0, 40.0)))
        assertTrue(json.contains("\"type\":\"Feature\""))
        assertTrue(json.contains("\"type\":\"LineString\""))
        // GeoJSON order is [lng, lat, ele]
        assertTrue(json.contains("[18.0,69.0,12.0]"))
        assertTrue(json.contains("\"name\":\"My Hike\""))
    }

    @Test
    fun `geojson omits elevation when absent`() {
        val json = pathToGeoJson(path(elevations = null))
        assertTrue(json.contains("[18.0,69.0]"))
    }

    @Test
    fun `kml emits a Placemark LineString with lng,lat,ele tuples`() {
        val kml = pathToKml(path(elevations = listOf(12.0, 40.0)))
        assertTrue(kml.contains("<kml xmlns=\"http://www.opengis.net/kml/2.2\">"))
        assertTrue(kml.contains("<name>My Hike</name>"))
        assertTrue(kml.contains("18.0,69.0,12.0"))
    }

    @Test
    fun `serialize dispatches by format and filenames use the right extension`() {
        assertTrue(serialize(path(), ExportFormat.Gpx).contains("<gpx"))
        assertTrue(serialize(path(), ExportFormat.GeoJson).contains("\"type\":\"Feature\""))
        assertTrue(serialize(path(), ExportFormat.Kml).contains("<kml"))
        assertEquals("My_Hike.geojson", exportFileName("My Hike", ExportFormat.GeoJson))
        assertEquals("My_Hike.kml", exportFileName("My Hike", ExportFormat.Kml))
    }
}
