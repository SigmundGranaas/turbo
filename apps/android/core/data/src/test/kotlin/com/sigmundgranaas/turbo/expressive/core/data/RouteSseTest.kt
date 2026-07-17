package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import com.sigmundgranaas.turbo.expressive.domain.RouteStreamEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RouteSseTest {

    @Test
    fun `encodeRequest emits lon-lat pairs with preset and profile`() {
        val body = RouteSse.encodeRequest(
            points = listOf(LatLng(69.65, 18.95), LatLng(69.66, 18.96)),
            preset = RoutePreset.AvoidRoads,
            profile = "foot",
        )
        // GeoJSON order is [lon, lat].
        assertTrue(body.contains("[18.95,69.65]"))
        assertTrue(body.contains("\"preset\":\"avoid_roads\""))
        assertTrue(body.contains("\"profile\":\"foot\""))
    }

    @Test
    fun `round trip is serialized only when enabled`() {
        val pts = listOf(LatLng(69.65, 18.95), LatLng(69.66, 18.96))
        val on = RouteSse.encodeRequest(pts, RoutePreset.Balanced, "foot", roundTrip = true)
        assertTrue(on.contains("\"round_trip\":true"))
        // Default (off) keeps the request body unchanged — no field at all.
        val off = RouteSse.encodeRequest(pts, RoutePreset.Balanced, "foot", roundTrip = false)
        assertTrue(!off.contains("round_trip"))
    }

    @Test
    fun `progress frame maps lon-lat to LatLng`() {
        val event = RouteSse.parse("progress", """{"coordinates":[[18.95,69.65],[18.96,69.66]]}""")
        val progress = event as RouteStreamEvent.Progress
        assertEquals(2, progress.coordinates.size)
        assertEquals(69.65, progress.coordinates[0].lat, 1e-9)
        assertEquals(18.95, progress.coordinates[0].lng, 1e-9)
    }

    @Test
    fun `result frame maps the full plan`() {
        val data = """
            {"distance_m":8537.5,"duration_s":3456.0,"ascent_m":234.0,"on_trail_pct":78.5,
             "surfaces":{"trail":6700.0,"road":1837.5},
             "geometry":{"type":"LineString","coordinates":[[18.95,69.65],[18.96,69.66]]},
             "legs":[{"from_index":0,"to_index":1,"distance_m":8537.5}]}
        """.trimIndent()
        val plan = (RouteSse.parse("result", data) as RouteStreamEvent.Result).plan
        assertEquals(8537.5, plan.distanceM, 1e-6)
        assertEquals(3456.0, plan.durationS, 1e-6)
        assertEquals(234.0, plan.ascentM, 1e-6)
        assertEquals(78.5, plan.onTrailPct, 1e-6)
        assertEquals(6700.0, plan.surfaces["trail"]!!, 1e-6)
        assertEquals(2, plan.geometry.size)
        assertEquals(69.66, plan.geometry[1].lat, 1e-9)
    }

    @Test
    fun `error frame yields failure with message`() {
        val event = RouteSse.parse("error", """{"error":"endpoint refused"}""")
        assertEquals("endpoint refused", (event as RouteStreamEvent.Failure).message)
    }

    @Test
    fun `error frame without message falls back to default`() {
        val event = RouteSse.parse("error", "{}")
        assertEquals(RouteSse.DEFAULT_ERROR, (event as RouteStreamEvent.Failure).message)
    }

    @Test
    fun `unknown or keep-alive frames are ignored`() {
        assertNull(RouteSse.parse(null, "anything"))
        assertNull(RouteSse.parse("ping", "{}"))
    }
}
