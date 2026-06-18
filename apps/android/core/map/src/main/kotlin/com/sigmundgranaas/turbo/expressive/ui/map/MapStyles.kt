package com.sigmundgranaas.turbo.expressive.ui.map

import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import com.sigmundgranaas.turbo.expressive.domain.TurbomapScene

/**
 * MapLibre raster style JSON for each base map. Norgeskart is the official
 * Kartverket topo cache; OSM and a satellite layer are the alternates shown in
 * the layer selector. Attribution is baked into each source. Any number of
 * transparent data [OverlayId] rasters are composited on top in a stable order.
 */
object MapStyles {

    /** Transparent XYZ overlay tiles for an [OverlayId], or null when it has no raster source. */
    private fun overlayTiles(overlay: OverlayId): Pair<String, String>? = when (overlay) {
        // Waymarked Trails hiking layer — transparent marked-route raster (CC-BY-SA).
        OverlayId.Trails ->
            "https://tile.waymarkedtrails.org/hiking/{z}/{x}/{y}.png" to
                "© Waymarked Trails, © OpenStreetMap contributors"
        // NVE Bratthetskart — slope-steepness classes (≥27°), the standard Norwegian
        // avalanche-terrain overlay. Public ArcGIS fused tile cache (WebMercator).
        OverlayId.Avalanche ->
            "https://gis3.nve.no/arcgis/rest/services/wmts/Bratthet_2024/MapServer/tile/{z}/{y}/{x}" to
                "© NVE — Bratthetskart"
        // Wave/Wind heat/flow fields need a keyed/commercial raster source (OWM/Windy/
        // MET thredds with auth) that this build doesn't carry — left unwired rather
        // than shipped as a dead toggle.
        OverlayId.Waves, OverlayId.Wind -> null
    }

    /** The overlays that actually have a tile source today (drives the layer sheet). */
    val renderableOverlays: List<OverlayId> = OverlayId.entries.filter { overlayTiles(it) != null }

    // Base tile sources, shared between the MapLibre style JSON and the turbomap
    // Scene specs so the two renderers fetch identical tiles.
    private const val NORGESKART_URL = "https://cache.kartverket.no/v1/wmts/1.0.0/topo/default/webmercator/{z}/{y}/{x}.png"
    private const val OSM_URL = "https://tile.openstreetmap.org/{z}/{x}/{y}.png"
    private const val SATELLITE_URL = "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}"

    /** Mapbox-Terrain-RGB DEM from our tileserver — the heightmap that elevates
     *  the ground in 3D mode (wgpu engine only). `?halo=1` bakes a 1px ring of
     *  the neighbours' elevation into each tile (258px) so adjacent terrain mesh
     *  edges agree and the surface doesn't crack at tile boundaries; MUST match
     *  the `halo` the scene declares (TurbomapScene.TERRAIN_HALO_PX). */
    const val TERRAIN_DEM_URL = "https://kart-api.sandring.no/v1/dem/rgb/{z}/{x}/{y}.png?halo=1"

    private fun baseTiles(base: BaseLayer): Pair<String, String> = when (base) {
        BaseLayer.Norgeskart -> "norgeskart" to NORGESKART_URL
        BaseLayer.Osm -> "osm" to OSM_URL
        BaseLayer.Satellite -> "satellite" to SATELLITE_URL
    }

    /**
     * The same base + overlay tile sources as [styleJson], but as turbomap
     * [TurbomapScene.RasterSpec]s (bottom→top). Lets the wgpu host fetch the
     * identical tiles without reimplementing the URL knowledge.
     */
    fun turbomapRasterSpecs(base: BaseLayer, overlays: Set<OverlayId> = emptySet()): List<TurbomapScene.RasterSpec> {
        val (id, url) = baseTiles(base)
        val baseSpec = TurbomapScene.RasterSpec(id, url)
        val overlaySpecs = overlays.mapNotNull { ov ->
            overlayTiles(ov)?.let { TurbomapScene.RasterSpec("ov_${ov.name}", it.first) }
        }
        return listOf(baseSpec) + overlaySpecs
    }

    private fun raster(id: String, url: String, attribution: String, bg: String, overlays: Set<OverlayId>): String {
        val sourced = overlays.mapNotNull { ov -> overlayTiles(ov)?.let { ov to it } }
        val overlaySources = sourced.joinToString("") { (ov, t) ->
            ""","ov_${ov.name}": { "type": "raster", "tiles": ["${t.first}"], "tileSize": 256, "attribution": "${t.second}" }"""
        }
        val overlayLayers = sourced.joinToString("") { (ov, _) ->
            """, { "id": "ov_${ov.name}", "type": "raster", "source": "ov_${ov.name}" }"""
        }
        return """
        {
          "version": 8,
          "sources": {
            "$id": {
              "type": "raster",
              "tiles": ["$url"],
              "tileSize": 256,
              "attribution": "$attribution"
            }$overlaySources
          },
          "layers": [
            { "id": "bg", "type": "background", "paint": { "background-color": "$bg" } },
            { "id": "$id", "type": "raster", "source": "$id" }$overlayLayers
          ]
        }
        """.trimIndent()
    }

    fun styleJson(base: BaseLayer, overlays: Set<OverlayId> = emptySet()): String = when (base) {
        BaseLayer.Norgeskart -> raster(
            id = "norgeskart",
            url = NORGESKART_URL,
            attribution = "© Kartverket",
            bg = "#EAF1F4",
            overlays = overlays,
        )
        BaseLayer.Osm -> raster(
            id = "osm",
            url = OSM_URL,
            attribution = "© OpenStreetMap contributors",
            bg = "#AAD3DF",
            overlays = overlays,
        )
        BaseLayer.Satellite -> raster(
            id = "satellite",
            url = SATELLITE_URL,
            attribution = "© Esri",
            bg = "#0B1A2B",
            overlays = overlays,
        )
    }
}
