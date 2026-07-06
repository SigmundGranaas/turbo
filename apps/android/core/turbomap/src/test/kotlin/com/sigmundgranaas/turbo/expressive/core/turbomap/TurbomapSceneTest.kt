package com.sigmundgranaas.turbo.expressive.core.turbomap

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.TurbomapScene
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import uniffi.turbomap_ffi.Camera
import uniffi.turbomap_ffi.FfiException
import uniffi.turbomap_ffi.TurboMap
import java.io.ByteArrayInputStream
import javax.imageio.ImageIO

/**
 * Stage-E scene authoring: the app's full live-map state ([TurbomapScene]) is a
 * valid, renderable turbomap Scene. Drives the real engine on the host GPU — if
 * green, "track/route/measure as Scene layers" holds: the engine accepts every
 * layer (none unsupported), the geojson drains locally, only the basemap raster
 * stays pending for host fetch, and the frame renders.
 */
class TurbomapSceneTest {

    private fun newMap(): TurboMap =
        try {
            TurboMap.headless(512u, 384u, Camera(60.39, 5.32, 9.0, 0.0, 0.0))
        } catch (e: FfiException.NoAdapter) {
            if (System.getenv("REQUIRE_GPU") == "1") throw e
            assumeTrue("no usable GPU adapter: ${e.message}", false)
            error("unreachable")
        }

    private val measure = listOf(LatLng(60.35, 5.20), LatLng(60.42, 5.40), LatLng(60.45, 5.50))

    @Test
    fun `the app's live state is a valid renderable scene`() {
        newMap().use { map ->
            val json = TurbomapScene.build(
                rasters = listOf(TurbomapScene.RasterSpec("basemap", "https://example.test/{z}/{x}/{y}.png")),
                measure = measure,
            )
            // 1 raster + measure-line + measure-pts = 3 layers / 3 sources. (track +
            // route are scene-declared `tube` layers, and the live user position
            // is a Compose MyPositionPin — not emitted for this state.)
            val delta = map.applyScene(json)
            assertEquals("layers: $delta", 3u, delta.layersAdded)
            assertEquals("sources: $delta", 3u, delta.sourcesChanged)
            assertTrue("no layer unsupported: ${map.unsupportedLayers()}", map.unsupportedLayers().isEmpty())

            // Every geojson overlay drains in-process; only the raster basemap
            // surfaces to the host, through the streaming plan (P5.1).
            val local = map.pumpLocalTiles()
            assertTrue("geojson should drain: $local", local.vectorTiles > 0u)
            val starts = planStarts(map.streamingPlanJson(64u))
            assertTrue("the basemap should need tiles", starts.isNotEmpty())
            assertTrue(
                "only the basemap raster starts: $starts",
                starts.all { it.kind == "raster" && it.layer == "basemap" },
            )

            // The composed frame renders.
            val img = ImageIO.read(ByteArrayInputStream(map.renderPng()))
            assertEquals(512 to 384, img.width to img.height)
        }
    }

    @Test
    fun `empty state is still a valid scene`() {
        newMap().use { map ->
            val delta = map.applyScene(TurbomapScene.build())
            assertEquals(0u, delta.layersAdded)
            assertTrue(map.unsupportedLayers().isEmpty())
        }
    }

    @Test
    fun `tubes and the environment are scene state, not side-doors`() {
        newMap().use { map ->
            // Route/track tubes + sun/shadows/clouds ride the ONE scene
            // document (plan P5.2) — the imperative natives are gone.
            val json = TurbomapScene.build(
                rasters = listOf(TurbomapScene.RasterSpec("basemap", "https://example.test/{z}/{x}/{y}.png")),
                tubes = listOf(
                    TurbomapScene.TubeSpec("route", measure, TurbomapScene.RouteColor, radiusPx = 8.0),
                    // A 1-point tube can't be a line — omitted, like short polylines.
                    TurbomapScene.TubeSpec("track", listOf(LatLng(60.0, 5.0)), TurbomapScene.TrackColor, 8.0),
                ),
                environment = TurbomapScene.EnvironmentSpec(
                    sunUnixSeconds = 1_751_700_000.0,
                    terrainShadows = 0.85f,
                    clouds = TurbomapScene.CloudsSpec(16, 16, west = 4.0, south = 59.0, east = 7.0, north = 61.0),
                ),
            )
            assertFalse("the 1-point tube is omitted", json.contains("\"track\""))
            assertTrue("the environment block is declared: $json", json.contains("\"environment\""))

            // basemap raster + the route tube; the engine renders both.
            val delta = map.applyScene(json)
            assertEquals("layers: $delta", 2u, delta.layersAdded)
            assertTrue("tube must be renderable: ${map.unsupportedLayers()}", map.unsupportedLayers().isEmpty())
            // The applied scene round-trips with the environment intact.
            assertTrue("engine kept the lighting: ${map.sceneJson()}", map.sceneJson().contains("time-tracked"))
        }
    }

    @Test
    fun `short polylines are omitted, not emitted as broken layers`() {
        newMap().use { map ->
            // A 1-point measure can't be a line; the measure-line layer must be
            // omitted (the single vertex still shows as measure-pts).
            val json = TurbomapScene.build(measure = listOf(LatLng(60.0, 5.0)))
            val delta = map.applyScene(json)
            assertEquals("measure-pts only", 1u, delta.layersAdded)
            assertFalse("no broken line layer", json.contains("\"measure-line\""))
            assertTrue(map.unsupportedLayers().isEmpty())
        }
    }
}
