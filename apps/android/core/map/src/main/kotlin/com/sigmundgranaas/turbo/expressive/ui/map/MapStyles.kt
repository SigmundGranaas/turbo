package com.sigmundgranaas.turbo.expressive.ui.map

import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.OverlayId

/**
 * MapLibre raster style JSON for each base map. Norgeskart is the official
 * Kartverket topo cache; OSM and a satellite layer are the alternates shown in
 * the layer selector. Attribution is baked into each source. An optional
 * transparent data [OverlayId] raster is composited on top.
 */
object MapStyles {

    /** Transparent XYZ overlay tiles, or null when an overlay has no raster source yet. */
    private fun overlayTiles(overlay: OverlayId?): Pair<String, String>? = when (overlay) {
        // Waymarked Trails hiking layer — transparent marked-route raster (CC-BY-SA).
        OverlayId.Trails ->
            "https://tile.waymarkedtrails.org/hiking/{z}/{x}/{y}.png" to
                "© Waymarked Trails, © OpenStreetMap contributors"
        else -> null // Waves/Wind/Avalanche need a data-derived source (not yet wired).
    }

    private fun raster(id: String, url: String, attribution: String, bg: String, overlay: OverlayId?): String {
        val ov = overlayTiles(overlay)
        val overlaySource = ov?.let {
            ""","overlay": { "type": "raster", "tiles": ["${it.first}"], "tileSize": 256, "attribution": "${it.second}" }"""
        }.orEmpty()
        val overlayLayer = if (ov != null) {
            """, { "id": "overlay", "type": "raster", "source": "overlay" }"""
        } else {
            ""
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
            }$overlaySource
          },
          "layers": [
            { "id": "bg", "type": "background", "paint": { "background-color": "$bg" } },
            { "id": "$id", "type": "raster", "source": "$id" }$overlayLayer
          ]
        }
        """.trimIndent()
    }

    fun styleJson(base: BaseLayer, overlay: OverlayId? = null): String = when (base) {
        BaseLayer.Norgeskart -> raster(
            id = "norgeskart",
            url = "https://cache.kartverket.no/v1/wmts/1.0.0/topo/default/webmercator/{z}/{y}/{x}.png",
            attribution = "© Kartverket",
            bg = "#EAF1F4",
            overlay = overlay,
        )
        BaseLayer.Osm -> raster(
            id = "osm",
            url = "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
            attribution = "© OpenStreetMap contributors",
            bg = "#AAD3DF",
            overlay = overlay,
        )
        BaseLayer.Satellite -> raster(
            id = "satellite",
            url = "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
            attribution = "© Esri",
            bg = "#0B1A2B",
            overlay = overlay,
        )
    }
}
