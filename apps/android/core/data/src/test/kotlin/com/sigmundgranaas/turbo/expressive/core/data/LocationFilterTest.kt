package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.double
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.File

/**
 * Runs the shared filter fixtures (`fixtures/tracking/filter/`) through
 * [LocationFilter], asserting the same accepted indices the iOS test does — the
 * lockstep contract for GPS hygiene.
 */
class LocationFilterTest {

    @Test fun `valid walk`() = run("valid-walk")
    @Test fun `stale resume fix dropped`() = run("resume-stale")
    @Test fun `low-accuracy fix dropped`() = run("low-accuracy")
    @Test fun `isolated teleport rejected, confirmed jump accepted`() = run("teleport-jump")

    private fun run(name: String) {
        val obj = Json.parseToJsonElement(fixtureFile("filter/$name.json").readText()).jsonObject
        val p = obj["params"]!!.jsonObject
        val filter = LocationFilter(
            accuracyMaxM = p["accuracyMaxM"]!!.jsonPrimitive.double,
            stalenessMaxMs = p["stalenessMaxMs"]!!.jsonPrimitive.double,
            jumpMaxM = p["jumpMaxM"]!!.jsonPrimitive.double,
        )
        val accepted = mutableListOf<Int>()
        obj["fixes"]!!.jsonArray.forEachIndexed { i, el ->
            val f = el.jsonObject
            val ok = filter.accept(
                LatLng(f["lat"]!!.jsonPrimitive.double, f["lng"]!!.jsonPrimitive.double),
                f["accuracyM"]!!.jsonPrimitive.double,
                f["ageMs"]!!.jsonPrimitive.double,
            )
            if (ok) accepted.add(i)
        }
        val expected = obj["acceptedIndices"]!!.jsonArray.map { it.jsonPrimitive.int }
        assertEquals("$name accepted indices", expected, accepted)
    }

    private fun fixtureFile(relative: String): File {
        var dir: File? = File(System.getProperty("user.dir") ?: ".").absoluteFile
        while (dir != null) {
            val candidate = File(dir, "fixtures/tracking")
            if (candidate.isDirectory) return File(candidate, relative)
            dir = dir.parentFile
        }
        error("fixtures/tracking not found")
    }
}
