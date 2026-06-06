package com.sigmundgranaas.turbo.expressive.core.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class TrailWfsTest {

    @Test
    fun `LineString first vertex is swapped to lat-lng`() {
        val body = """
            {"features":[
              {"properties":{"navn":"Besseggen","rutenummer":"R1","merkemetode":"Merket"},
               "geometry":{"type":"LineString","coordinates":[[8.70,61.50],[8.71,61.51]]}}
            ]}
        """.trimIndent()
        val hits = TrailWfs.parse(body)
        assertEquals(1, hits.size)
        assertEquals("Besseggen", hits[0].name)
        // GeoJSON [lng, lat] → LatLng(lat, lng)
        assertEquals(61.50, hits[0].position.lat, 1e-9)
        assertEquals(8.70, hits[0].position.lng, 1e-9)
        assertTrue(hits[0].description.contains("Trail"))
        assertTrue(hits[0].description.contains("R1"))
    }

    @Test
    fun `MultiLineString descends to the first pair`() {
        val body = """
            {"features":[
              {"properties":{"navn":"Hardangervidda"},
               "geometry":{"type":"MultiLineString","coordinates":[[[7.50,60.20],[7.51,60.21]]]}}
            ]}
        """.trimIndent()
        val hits = TrailWfs.parse(body)
        assertEquals(1, hits.size)
        assertEquals(60.20, hits[0].position.lat, 1e-9)
        assertEquals(7.50, hits[0].position.lng, 1e-9)
    }

    @Test
    fun `features without a name or geometry are dropped`() {
        val body = """
            {"features":[
              {"properties":{"navn":""},"geometry":{"type":"LineString","coordinates":[[1.0,2.0]]}},
              {"properties":{"navn":"No geom"},"geometry":null}
            ]}
        """.trimIndent()
        assertTrue(TrailWfs.parse(body).isEmpty())
    }

    @Test
    fun `malformed body yields an empty list rather than throwing`() {
        assertTrue(TrailWfs.parse("not json").isEmpty())
        assertTrue(TrailWfs.parse("{}").isEmpty())
    }
}
