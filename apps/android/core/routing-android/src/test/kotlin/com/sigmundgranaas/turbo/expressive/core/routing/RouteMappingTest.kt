package com.sigmundgranaas.turbo.expressive.core.routing

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/**
 * Everything about the on-device path that a JVM can judge.
 *
 * No NDK, no device, no pack — which is the point of keeping the translation in
 * pure functions. What is left untested here needs hardware, and pretending
 * otherwise would be worse than saying so.
 */
class RouteMappingTest {

    @get:Rule
    val tmp = TemporaryFolder()

    @Test
    fun `preset keys are the engine's own names`() {
        // The enum was written against `route-presets.toml`, so this is a read
        // rather than a translation — and it must stay one. A preset the engine
        // does not know is an error naming the valid ones, not a fallback, so a
        // drift here fails every route in that style rather than degrading it.
        assertEquals("balanced", RouteMapping.presetKey(RoutePreset.Balanced))
        assertEquals("avoid_roads", RouteMapping.presetKey(RoutePreset.AvoidRoads))
        assertEquals("trail_purist", RouteMapping.presetKey(RoutePreset.TrailPurist))
        RoutePreset.entries.forEach {
            assertTrue("preset key must be snake_case: ${it.key}", it.key.matches(Regex("[a-z_]+")))
        }
    }

    @Test
    fun `on-trail share counts every surface except off_trail`() {
        val surfaces = mapOf("sti" to 3000.0, "vei" to 1000.0, "off_trail" to 1000.0)
        assertEquals(0.8, RouteMapping.onTrailFraction(surfaces), 1e-9)

        // A surface class this build has never heard of came from the trail
        // network, so it counts as trail. Listing the known-good ones instead
        // would silently reclassify it as wilderness.
        val withNewSurface = surfaces + ("klopp" to 1000.0)
        assertEquals(0.8, RouteMapping.onTrailFraction(withNewSurface), 1e-9)

        assertEquals(1.0, RouteMapping.onTrailFraction(mapOf("sti" to 500.0)), 1e-9)
        assertEquals(0.0, RouteMapping.onTrailFraction(mapOf("off_trail" to 500.0)), 1e-9)
        // An empty breakdown must not divide by zero.
        assertEquals(0.0, RouteMapping.onTrailFraction(emptyMap()), 1e-9)
    }

    @Test
    fun `plan carries the engine's numbers through unchanged`() {
        val plan = RouteMapping.plan(
            geometry = listOf(LatLng(67.06, 15.04), LatLng(67.07, 15.05)),
            lengthM = 1671.6,
            durationS = 1400.0,
            ascentM = 88.0,
            surfaces = mapOf("sti" to 1200.0, "off_trail" to 400.0),
        )
        assertEquals(1671.6, plan.distanceM, 1e-9)
        assertEquals(1400.0, plan.durationS, 1e-9)
        assertEquals(88.0, plan.ascentM, 1e-9)
        assertEquals(0.75, plan.onTrailPct, 1e-9)
        // Latitude first in the app's LatLng, longitude first in the engine's
        // GeoPoint. Both are doubles in overlapping ranges over Norway, so a
        // transposition compiles and runs and puts the route in the Barents Sea.
        assertEquals(67.06, plan.geometry.first().lat, 1e-9)
        assertEquals(15.04, plan.geometry.first().lng, 1e-9)
    }

    @Test
    fun `a pack is discovered by its manifest extent`() {
        val dir = tmp.newFolder("sjunkhatten")
        java.io.File(dir, "pack.toml").writeText(
            """
            # Region pack manifest
            [pack]
            format_version = 1
            frame = "utm33n"
            extent = [14.95, 67.02, 15.2, 67.12]
            halo_m = 1000.0
            """.trimIndent(),
        )
        val store = PackStore(tmp.root)
        assertEquals(listOf("sjunkhatten"), store.packs().map { it.id })

        // Both endpoints inside → this pack answers.
        assertEquals("sjunkhatten", store.covering(listOf(15.04 to 67.06, 15.05 to 67.07))?.id)
        // One endpoint outside → no pack, rather than the nearest one. A route
        // the chosen pack only half covers fails at solve time with an error
        // about terrain, which reads as "the router is broken" instead of "you
        // have not downloaded that area".
        assertNull(store.covering(listOf(15.04 to 67.06, 16.90 to 67.06)))
    }

    @Test
    fun `a directory without a manifest is not a pack`() {
        tmp.newFolder("half-downloaded")
        // Artifacts present but no manifest: a download interrupted between the
        // files and the marker. Skipped rather than opened, because the manifest
        // is what says the pack is complete and what this build can read.
        java.io.File(tmp.newFolder("no-manifest"), "norway.dem").writeText("x")
        assertTrue(PackStore(tmp.root).packs().isEmpty())
    }

    @Test
    fun `a manifest with a malformed extent is skipped, not guessed`() {
        val dir = tmp.newFolder("broken")
        java.io.File(dir, "pack.toml").writeText(
            "[pack]\nformat_version = 1\nextent = [14.95, 67.02]\n",
        )
        assertTrue(PackStore(tmp.root).packs().isEmpty())
    }
}
