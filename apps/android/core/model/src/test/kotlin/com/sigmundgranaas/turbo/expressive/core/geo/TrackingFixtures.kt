package com.sigmundgranaas.turbo.expressive.core.geo

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.double
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Loads the shared cross-platform tracking fixtures from `fixtures/tracking/` at
 * the repo root — the SAME files the iOS tests load, so the two implementations
 * can't silently diverge. Parsed via kotlinx-serialization's JsonElement (no
 * compiler plugin needed).
 */
object TrackingFixtures {

    data class ProgressExpect(val fraction: Double, val arrived: Boolean, val offRoute: Boolean)
    data class ProgressParams(
        val windowBackM: Double,
        val windowAheadM: Double,
        val offRouteM: Double,
        val arriveEndM: Double,
    )
    data class ProgressFixture(
        val name: String,
        val params: ProgressParams,
        val route: List<LatLng>,
        val fixes: List<LatLng>,
        val expect: List<ProgressExpect>,
    )

    fun progress(name: String): ProgressFixture {
        val obj = Json.parseToJsonElement(file("progress/$name.json").readText()).jsonObject
        val p = obj["params"]!!.jsonObject
        return ProgressFixture(
            name = obj["name"]!!.jsonPrimitive.content,
            params = ProgressParams(
                p["windowBackM"]!!.jsonPrimitive.double,
                p["windowAheadM"]!!.jsonPrimitive.double,
                p["offRouteM"]!!.jsonPrimitive.double,
                p["arriveEndM"]!!.jsonPrimitive.double,
            ),
            route = obj["route"]!!.jsonArray.map { it.jsonArray.let { a -> LatLng(a[0].jsonPrimitive.double, a[1].jsonPrimitive.double) } },
            fixes = obj["fixes"]!!.jsonArray.map { it.jsonArray.let { a -> LatLng(a[0].jsonPrimitive.double, a[1].jsonPrimitive.double) } },
            expect = obj["expect"]!!.jsonArray.map {
                val e = it.jsonObject
                ProgressExpect(e["fraction"]!!.jsonPrimitive.double, e["arrived"]!!.jsonPrimitive.boolean, e["offRoute"]!!.jsonPrimitive.boolean)
            },
        )
    }

    /** Walk up from the test working directory until `fixtures/tracking` is found. */
    fun file(relative: String): File {
        var dir: File? = File(System.getProperty("user.dir") ?: ".").absoluteFile
        while (dir != null) {
            val candidate = File(dir, "fixtures/tracking")
            if (candidate.isDirectory) return File(candidate, relative)
            dir = dir.parentFile
        }
        error("fixtures/tracking not found above ${System.getProperty("user.dir")}")
    }
}

class RouteProgressFixtureTest {
    @Test fun `straight line`() = run("straight")
    @Test fun `out-and-back (start == end loop bug)`() = run("out-and-back")

    private fun run(name: String) {
        val fx = TrackingFixtures.progress(name)
        val tracker = RouteProgressTracker(
            route = fx.route,
            windowBackM = fx.params.windowBackM,
            windowAheadM = fx.params.windowAheadM,
            offRouteM = fx.params.offRouteM,
            arriveEndM = fx.params.arriveEndM,
        )
        fx.fixes.forEachIndexed { i, fix ->
            val p = tracker.update(fix)
            val e = fx.expect[i]
            assertEquals("$name fix $i fraction", e.fraction, p.fraction, 0.02)
            assertEquals("$name fix $i arrived", e.arrived, p.arrived)
            assertEquals("$name fix $i offRoute", e.offRoute, p.offRoute)
        }
    }
}

class TrackingFixtureLoadingTest {
    @Test
    fun `progress fixtures parse with matching expect counts`() {
        val straight = TrackingFixtures.progress("straight")
        assertEquals(2, straight.route.size)
        assertEquals(straight.fixes.size, straight.expect.size)
        assertEquals(1500.0, straight.params.windowAheadM, 0.0)

        val oab = TrackingFixtures.progress("out-and-back")
        assertEquals(3, oab.route.size)
        assertEquals(9, oab.fixes.size)
        assertTrue(oab.expect.last().arrived)
    }
}
