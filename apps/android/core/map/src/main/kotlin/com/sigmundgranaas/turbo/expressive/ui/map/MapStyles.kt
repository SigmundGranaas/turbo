package com.sigmundgranaas.turbo.expressive.ui.map

import com.sigmundgranaas.turbo.expressive.domain.BaseLayer

/**
 * MapLibre raster style JSON for each base map. Norgeskart is the official
 * Kartverket topo cache; OSM and a satellite layer are the alternates shown in
 * the layer selector. Attribution is baked into each source.
 */
object MapStyles {

    private fun raster(id: String, url: String, attribution: String, bg: String): String = """
        {
          "version": 8,
          "sources": {
            "$id": {
              "type": "raster",
              "tiles": ["$url"],
              "tileSize": 256,
              "attribution": "$attribution"
            }
          },
          "layers": [
            { "id": "bg", "type": "background", "paint": { "background-color": "$bg" } },
            { "id": "$id", "type": "raster", "source": "$id" }
          ]
        }
    """.trimIndent()

    fun styleJson(base: BaseLayer): String = when (base) {
        BaseLayer.Norgeskart -> raster(
            id = "norgeskart",
            url = "https://cache.kartverket.no/v1/wmts/1.0.0/topo/default/webmercator/{z}/{y}/{x}.png",
            attribution = "© Kartverket",
            bg = "#EAF1F4",
        )
        BaseLayer.Osm -> raster(
            id = "osm",
            url = "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
            attribution = "© OpenStreetMap contributors",
            bg = "#AAD3DF",
        )
        BaseLayer.Satellite -> raster(
            id = "satellite",
            url = "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
            attribution = "© Esri",
            bg = "#0B1A2B",
        )
    }
}
