package com.sigmundgranaas.turbo.expressive.feature.markers

import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class MarkerGeoJsonTest {

    @Test
    fun `encodes a FeatureCollection of points in lng-lat order`() {
        val json = MarkerGeoJson.encode(
            listOf(Marker("m1", "Camp", ActivityKindId.Camping, LatLng(69.6, 20.0), notes = "by the river")),
        )
        assertTrue(json.startsWith("{\"type\":\"FeatureCollection\""))
        assertTrue(json.contains("\"type\":\"Point\""))
        assertTrue(json.contains("\"coordinates\":[20.0,69.6]"))
        assertTrue(json.contains("\"name\":\"Camp\""))
        assertTrue(json.contains("\"notes\":\"by the river\""))
    }

    @Test
    fun `omits blank notes and escapes quotes`() {
        val json = MarkerGeoJson.encode(
            listOf(Marker("m2", "A \"B\"", ActivityKindId.Cabin, LatLng(60.0, 10.0))),
        )
        assertTrue(!json.contains("notes"))
        assertTrue(json.contains("\\\"B\\\""))
    }

    @Test
    fun `filename sanitises to a safe geojson stem`() {
        assertEquals("My_markers.geojson", MarkerGeoJson.fileName("My markers"))
        assertEquals("markers.geojson", MarkerGeoJson.fileName("  "))
    }
}
