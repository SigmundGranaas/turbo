package com.sigmundgranaas.turbo.expressive.feature.markers

import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class MarkerGpxTest {

    @Test
    fun `encodes wpt waypoints with name, notes and kind symbol`() {
        val gpx = MarkerGpx.encode(
            listOf(Marker("m1", "Camp", ActivityKindId.Camping, LatLng(69.6, 20.0), notes = "by the river")),
        )
        assertTrue(gpx.contains("<gpx version=\"1.1\""))
        assertTrue(gpx.contains("<wpt lat=\"69.6\" lon=\"20.0\">"))
        assertTrue(gpx.contains("<name>Camp</name>"))
        assertTrue(gpx.contains("<desc>by the river</desc>"))
        assertTrue(gpx.contains("<sym>${ActivityKindId.Camping.key}</sym>"))
    }

    @Test
    fun `omits blank notes and escapes xml`() {
        val gpx = MarkerGpx.encode(
            listOf(Marker("m2", "A <B> & C", ActivityKindId.Cabin, LatLng(60.0, 10.0))),
        )
        assertTrue(!gpx.contains("<desc>"))
        assertTrue(gpx.contains("<name>A &lt;B&gt; &amp; C</name>"))
    }

    @Test
    fun `filename sanitises to a safe gpx stem`() {
        assertEquals("My_markers.gpx", MarkerGpx.fileName("My markers"))
        assertEquals("markers.gpx", MarkerGpx.fileName("  "))
    }
}
