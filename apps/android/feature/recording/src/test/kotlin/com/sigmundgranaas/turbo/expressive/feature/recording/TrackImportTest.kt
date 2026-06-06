package com.sigmundgranaas.turbo.expressive.feature.recording

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class TrackImportTest {

    @Test
    fun `parses a gpx track with elevation and name`() {
        val gpx = """
            <?xml version="1.0"?>
            <gpx version="1.1"><trk><name>Besseggen</name><trkseg>
              <trkpt lat="61.50" lon="8.70"><ele>1100.0</ele></trkpt>
              <trkpt lat="61.51" lon="8.71"><ele>1200.0</ele></trkpt>
            </trkseg></trk></gpx>
        """.trimIndent()
        val track = TrackImport.parse(gpx)!!
        assertEquals("Besseggen", track.name)
        assertEquals(2, track.geo.points.size)
        assertEquals(61.50, track.geo.points[0].lat, 1e-9)
        assertEquals(8.70, track.geo.points[0].lng, 1e-9)
        assertEquals(1100.0, track.geo.elevations!![0]!!, 1e-9)
    }

    @Test
    fun `parses self-closing gpx trkpt tags`() {
        val gpx = """<gpx><trkseg><trkpt lat="1.0" lon="2.0"/><trkpt lat="1.1" lon="2.1"/></trkseg></gpx>"""
        val track = TrackImport.parse(gpx)!!
        assertEquals(2, track.geo.points.size)
        assertNull(track.geo.elevations)
    }

    @Test
    fun `parses a kml LineString with lng,lat,ele tuples`() {
        val kml = """
            <kml><Document><Placemark><name>Trip</name>
              <LineString><coordinates>8.70,61.50,1100 8.71,61.51,1200</coordinates></LineString>
            </Placemark></Document></kml>
        """.trimIndent()
        val track = TrackImport.parse(kml)!!
        assertEquals("Trip", track.name)
        assertEquals(61.50, track.geo.points[0].lat, 1e-9)
        assertEquals(1100.0, track.geo.elevations!![0]!!, 1e-9)
    }

    @Test
    fun `parses a geojson Feature LineString`() {
        val gj = """
            {"type":"Feature","properties":{"name":"GJ Hike"},
             "geometry":{"type":"LineString","coordinates":[[8.70,61.50,1100],[8.71,61.51]]}}
        """.trimIndent()
        val track = TrackImport.parse(gj)!!
        assertEquals("GJ Hike", track.name)
        assertEquals(2, track.geo.points.size)
        assertEquals(8.71, track.geo.points[1].lng, 1e-9)
    }

    @Test
    fun `parses a geojson FeatureCollection by taking the first feature`() {
        val gj = """
            {"type":"FeatureCollection","features":[
              {"type":"Feature","properties":{},"geometry":{"type":"LineString","coordinates":[[1.0,2.0],[1.1,2.1]]}}
            ]}
        """.trimIndent()
        val track = TrackImport.parse(gj)!!
        assertEquals(2, track.geo.points.size)
    }

    @Test
    fun `rejects unknown content and single-point tracks`() {
        assertNull(TrackImport.parse("hello world"))
        assertNull(TrackImport.parse("""<gpx><trkseg><trkpt lat="1.0" lon="2.0"/></trkseg></gpx>"""))
        assertNull(TrackImport.parse("""{"type":"Feature","geometry":{"type":"Point","coordinates":[1.0,2.0]}}"""))
    }

    @Test
    fun `import roundtrips an exported gpx`() {
        val original = TrackImport.parse(
            """<gpx><trk><name>RT</name><trkseg><trkpt lat="60.0" lon="10.0"/><trkpt lat="60.1" lon="10.1"/></trkseg></trk></gpx>""",
        )!!
        assertTrue(original.geo.points.size == 2)
    }
}
