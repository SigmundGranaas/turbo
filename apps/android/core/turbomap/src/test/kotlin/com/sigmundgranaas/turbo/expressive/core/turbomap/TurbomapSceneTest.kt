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
import uniffi.turbomap_ffi.TileKind
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

    private val track = listOf(LatLng(60.30, 5.10), LatLng(60.39, 5.32), LatLng(60.48, 5.55))
    private val route = listOf(LatLng(60.31, 5.12), LatLng(60.40, 5.33))
    private val measure = listOf(LatLng(60.35, 5.20), LatLng(60.42, 5.40), LatLng(60.45, 5.50))
    private val user = LatLng(60.39, 5.32)

    @Test
    fun `the app's live state is a valid renderable scene`() {
        newMap().use { map ->
            val json = TurbomapScene.build(
                rasters = listOf(TurbomapScene.RasterSpec("basemap", "https://example.test/{z}/{x}/{y}.png")),
                track = track,
                route = route,
                measure = measure,
                user = user,
            )
            // 1 raster + track + route + measure-line + measure-pts + user = 6 layers / 6 sources.
            val delta = map.applyScene(json)
            assertEquals("layers: $delta", 6u, delta.layersAdded)
            assertEquals("sources: $delta", 6u, delta.sourcesChanged)
            assertTrue("no layer unsupported: ${map.unsupportedLayers()}", map.unsupportedLayers().isEmpty())

            // Every geojson overlay drains in-process; only the raster basemap is pending.
            val local = map.pumpLocalTiles()
            assertTrue("geojson should drain: $local", local.vectorTiles > 0u)
            val pending = map.pendingTiles()
            assertTrue("only the basemap raster pends", pending.all { it.kind == TileKind.RASTER && it.layerId == "basemap" })

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
    fun `short polylines are omitted, not emitted as broken layers`() {
        newMap().use { map ->
            // A 1-point "track" can't be a line; only the user circle should appear.
            val json = TurbomapScene.build(track = listOf(LatLng(60.0, 5.0)), user = user)
            val delta = map.applyScene(json)
            assertEquals("only the user layer", 1u, delta.layersAdded)
            assertFalse("scene must stay valid", json.contains("\"track\""))
            assertTrue(map.unsupportedLayers().isEmpty())
        }
    }
}
