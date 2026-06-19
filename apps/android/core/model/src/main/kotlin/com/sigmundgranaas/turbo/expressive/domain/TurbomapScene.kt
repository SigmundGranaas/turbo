package com.sigmundgranaas.turbo.expressive.domain

/**
 * Authors the app's live-map state as a turbomap **Scene-IR JSON** document —
 * the core of Stage E ("track/route/measure stop being special; they're
 * `Line`/`Circle` layers over a `geojson` source"). Where the MapLibre path
 * imperatively pushes `GeoJsonSource.setGeoJson` into a style, the turbomap path
 * rebuilds an immutable Scene and hands it to `applyScene`.
 *
 * Renderer-agnostic: takes raster URL templates (the `BaseLayer`/`OverlayId` →
 * URL mapping stays app-side, in `MapStyles`) + the live polylines, and emits
 * the document the engine's `apply` consumes. Markers/waypoints stay native
 * Compose overlays (reprojected via `MapEngine`), exactly as on MapLibre.
 */
object TurbomapScene {

    /** sRGB colour for a paint `const`. */
    data class Rgba(val r: Int, val g: Int, val b: Int, val a: Int = 255)

    /** A raster basemap/overlay: an XYZ tile URL template, bottom-to-top order. */
    data class RasterSpec(val id: String, val tileUrlTemplate: String)

    // Defaults match the MapLibre path (TurboMap.kt installTurboLayers).
    val TrackColor = Rgba(0, 105, 109)
    val RouteColor = Rgba(143, 76, 56)
    val MeasureColor = Rgba(0, 105, 109)
    val UserColor = Rgba(26, 115, 232)

    /**
     * Build the Scene-IR JSON for the given map state. Layers are emitted only
     * when they have renderable geometry, so the result always validates (every
     * layer's source exists).
     */
    @Suppress("LongParameterList")
    fun build(
        rasters: List<RasterSpec> = emptyList(),
        track: List<LatLng>? = null,
        route: List<LatLng>? = null,
        measure: List<LatLng> = emptyList(),
        user: LatLng? = null,
        // When set, the engine loads this Mapbox-Terrain-RGB DEM as the shared
        // heightmap and ALL ground layers (basemap raster, hillshade, vector)
        // displace their vertices by elevation — real 3D terrain. The host
        // fetches the DEM tiles (it owns this same URL template). null = flat.
        demUrl: String? = null,
        trackColor: Rgba = TrackColor,
        routeColor: Rgba = RouteColor,
        measureColor: Rgba = MeasureColor,
    ): String {
        val sources = mutableListOf<String>()
        val layers = mutableListOf<String>()

        rasters.forEach { r ->
            val src = "r_${r.id}"
            sources += "\"$src\": { \"type\": \"raster-xyz\", \"tiles\": [\"${r.tileUrlTemplate}\"] }"
            layers += "{ \"type\": \"raster\", \"id\": \"${r.id}\", \"source\": \"$src\" }"
        }

        // Terrain: a DEM source declared height-only. The engine loads it as
        // the shared heightmap so the whole ground displaces, but draws NO
        // relief overlay — the DEM is height, not a visible tile. The basemap
        // raster lights itself from the sun instead (one lit 3D surface).
        // Placed after the rasters, under the vectors.
        if (demUrl != null) {
            // halo MUST match the `?halo=N` the host fetches (MapStyles): each
            // tile is 256+2N px with the neighbours' elevation in the ring, so
            // adjacent terrain mesh edges agree and the surface doesn't crack.
            sources += "\"dem\": { \"type\": \"dem-xyz\", \"tiles\": [\"$demUrl\"], " +
                "\"encoding\": \"mapbox-rgb\", \"halo\": $TERRAIN_HALO_PX }"
            layers += "{ \"type\": \"hillshade\", \"id\": \"hillshade\", \"source\": \"dem\", " +
                "\"exaggeration\": $TERRAIN_EXAGGERATION, \"height_only\": true }"
        }

        fun line(id: String, pts: List<LatLng>?, color: Rgba, width: Double) {
            val p = pts.orEmpty()
            if (p.size < 2) return
            sources += "\"$id\": { \"type\": \"geo-json\", \"data\": \"${escape(lineString(p))}\" }"
            layers += "{ \"type\": \"line\", \"id\": \"$id\", \"source\": \"$id\", " +
                "\"color\": ${const(color)}, \"width\": { \"const\": $width } }"
        }

        line("track", track, trackColor, TRACK_WIDTH)
        line("route", route, routeColor, ROUTE_WIDTH)
        line("measure-line", measure, measureColor, MEASURE_WIDTH)

        if (measure.isNotEmpty()) {
            sources += "\"measure-pts\": { \"type\": \"geo-json\", \"data\": \"${escape(multiPoint(measure))}\" }"
            layers += "{ \"type\": \"circle\", \"id\": \"measure-pts\", \"source\": \"measure-pts\", " +
                "\"color\": ${const(measureColor)}, \"radius\": { \"const\": $MEASURE_RADIUS } }"
        }

        if (user != null) {
            sources += "\"user\": { \"type\": \"geo-json\", \"data\": \"${escape(point(user))}\" }"
            layers += "{ \"type\": \"circle\", \"id\": \"user\", \"source\": \"user\", " +
                "\"color\": ${const(UserColor)}, \"radius\": { \"const\": $USER_RADIUS } }"
        }

        return "{ \"sources\": { ${sources.joinToString(", ")} }, " +
            "\"layers\": [ ${layers.joinToString(", ")} ] }"
    }

    private fun const(c: Rgba) = "{ \"const\": { \"r\": ${c.r}, \"g\": ${c.g}, \"b\": ${c.b}, \"a\": ${c.a} } }"

    private fun lineString(pts: List<LatLng>) =
        "{\"type\":\"LineString\",\"coordinates\":[${pts.joinToString(",") { "[${it.lng},${it.lat}]" }}]}"

    private fun multiPoint(pts: List<LatLng>) =
        "{\"type\":\"MultiPoint\",\"coordinates\":[${pts.joinToString(",") { "[${it.lng},${it.lat}]" }}]}"

    private fun point(p: LatLng) = "{\"type\":\"Point\",\"coordinates\":[${p.lng},${p.lat}]}"

    /** Escape a JSON document for embedding as the string `data` field. */
    private fun escape(json: String) = json.replace("\"", "\\\"")

    private const val TRACK_WIDTH = 4.0
    private const val ROUTE_WIDTH = 4.0
    private const val MEASURE_WIDTH = 3.0
    private const val MEASURE_RADIUS = 4.0
    private const val USER_RADIUS = 7.0

    /** Vertical exaggeration for 3D terrain. 1.0 = true scale; we push to a
     *  dramatic, game-like 2.5 so relief really reads when tilted — paired
     *  with sun-direction shading on the basemap, the height pops instead of
     *  looking flat. Tunable; lower it if peaks feel caricatured. */
    private const val TERRAIN_EXAGGERATION = 2.5

    /** DEM tile halo (px) — must equal the `?halo=N` the host fetches (MapStyles).
     *  Stitches adjacent terrain tiles crack-free. */
    private const val TERRAIN_HALO_PX = 1
}
