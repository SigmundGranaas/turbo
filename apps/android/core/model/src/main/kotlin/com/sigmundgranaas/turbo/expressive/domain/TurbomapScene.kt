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

    /**
     * A raster basemap/overlay: an XYZ tile URL template, bottom-to-top order.
     * [minZoom]/[maxZoom] are the source's *native* tile coverage — the engine
     * only requests tiles in this range and upsamples the deepest level when
     * the camera over-zooms past it, instead of requesting tiles the server
     * 404s (which used to blank the map past the default ceiling).
     */
    data class RasterSpec(
        val id: String,
        val tileUrlTemplate: String,
        val minZoom: Int = 0,
        val maxZoom: Int = DEFAULT_RASTER_MAX_ZOOM,
    )

    /**
     * A vector (MVT) source the engine fetches host-side. Currently used for the
     * realistic-water overlay: the `water` layer of the N50 basemap tiles is
     * drawn through the water pipeline (waves/reflection/foam) over the raster
     * basemap. [layerName] is the MVT source-layer to draw (e.g. `"water"`);
     * [id] doubles as the engine layer id the host fetches against.
     */
    data class VectorSpec(
        val id: String,
        val tileUrlTemplate: String,
        val layerName: String,
        val color: Rgba,
        val minZoom: Int = 4,
        val maxZoom: Int = DEFAULT_VECTOR_MAX_ZOOM,
    )

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
    // NOTE: track + route are NOT scene layers anymore — they render as raised
    // 3D tubes via `NativeSurfaceMap.nativeSetRouteTube` (a single lit mesh, not
    // a per-tile flat line). Only measure + user stay flat geojson here.
    @Suppress("LongParameterList")
    fun build(
        rasters: List<RasterSpec> = emptyList(),
        vectors: List<VectorSpec> = emptyList(),
        measure: List<LatLng> = emptyList(),
        // When set, the engine loads this Mapbox-Terrain-RGB DEM as the shared
        // heightmap and ALL ground layers (basemap raster, hillshade, vector)
        // displace their vertices by elevation — real 3D terrain. The host
        // fetches the DEM tiles (it owns this same URL template). null = flat.
        demUrl: String? = null,
        measureColor: Rgba = MeasureColor,
    ): String {
        val sources = mutableListOf<String>()
        val layers = mutableListOf<String>()

        rasters.forEach { r ->
            val src = "r_${r.id}"
            sources += "\"$src\": { \"type\": \"raster-xyz\", \"tiles\": [\"${r.tileUrlTemplate}\"], " +
                "\"min_zoom\": ${r.minZoom}, \"max_zoom\": ${r.maxZoom} }"
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

        // Vector overlays (e.g. realistic water): an MVT source + a fill layer
        // for the chosen source-layer, drawn over the raster basemap and draped
        // on the DEM. The water fill routes through the water pipeline core-side
        // (source-layer "water"). Placed after rasters/hillshade, under measure.
        vectors.forEach { v ->
            val src = "v_${v.id}"
            sources += "\"$src\": { \"type\": \"vector-xyz\", \"tiles\": [\"${v.tileUrlTemplate}\"], " +
                "\"min_zoom\": ${v.minZoom}, \"max_zoom\": ${v.maxZoom} }"
            layers += "{ \"type\": \"fill\", \"id\": \"${v.id}\", \"source\": \"$src\", " +
                "\"source-layer\": \"${v.layerName}\", \"color\": ${const(v.color)} }"
        }

        fun line(id: String, pts: List<LatLng>?, color: Rgba, width: Double) {
            val p = pts.orEmpty()
            if (p.size < 2) return
            sources += "\"$id\": { \"type\": \"geo-json\", \"data\": \"${escape(lineString(p))}\" }"
            layers += "{ \"type\": \"line\", \"id\": \"$id\", \"source\": \"$id\", " +
                "\"color\": ${const(color)}, \"width\": { \"const\": $width } }"
        }

        line("measure-line", measure, measureColor, MEASURE_WIDTH)

        if (measure.isNotEmpty()) {
            sources += "\"measure-pts\": { \"type\": \"geo-json\", \"data\": \"${escape(multiPoint(measure))}\" }"
            layers += "{ \"type\": \"circle\", \"id\": \"measure-pts\", \"source\": \"measure-pts\", " +
                "\"color\": ${const(measureColor)}, \"radius\": { \"const\": $MEASURE_RADIUS } }"
        }

        // NOTE: the live user position is NOT a scene layer — it draped flat on the ground and
        // looked wrong over 3D terrain. It's now a Compose `MyPositionPin` in `MapOverlay`,
        // projected through the elevation-aware engine seam so it stands on the surface.

        return "{ \"sources\": { ${sources.joinToString(", ")} }, " +
            "\"layers\": [ ${layers.joinToString(", ")} ] }"
    }

    private fun const(c: Rgba) = "{ \"const\": { \"r\": ${c.r}, \"g\": ${c.g}, \"b\": ${c.b}, \"a\": ${c.a} } }"

    private fun lineString(pts: List<LatLng>) =
        "{\"type\":\"LineString\",\"coordinates\":[${pts.joinToString(",") { "[${it.lng},${it.lat}]" }}]}"

    private fun multiPoint(pts: List<LatLng>) =
        "{\"type\":\"MultiPoint\",\"coordinates\":[${pts.joinToString(",") { "[${it.lng},${it.lat}]" }}]}"

    /** Escape a JSON document for embedding as the string `data` field. */
    private fun escape(json: String) = json.replace("\"", "\\\"")

    /** Fallback native max zoom when a source doesn't state one. Kept modest
     *  so an unspecified source doesn't claim depth the server lacks. */
    const val DEFAULT_RASTER_MAX_ZOOM = 19

    /** Native max zoom for the N50 vector basemap (water etc.). The engine
     *  over-zooms past this by upsampling the deepest tile. */
    const val DEFAULT_VECTOR_MAX_ZOOM = 15

    private const val MEASURE_WIDTH = 4.0
    private const val MEASURE_RADIUS = 4.0

    /** Vertical exaggeration for 3D terrain over true Mercator scale. Pushed
     *  hard (6×) so the height genuinely reads on a tilted phone screen —
     *  real proportions are correct but, framed against the huge horizontal
     *  extent of a map, look flat without strong exaggeration. Paired with
     *  sun-direction shading + a steep tilt, the relief becomes dramatic.
     *  Tunable; lower it if peaks feel caricatured. */
    private const val TERRAIN_EXAGGERATION = 6.0

    /** DEM tile halo (px) — must equal the `?halo=N` the host fetches (MapStyles).
     *  Stitches adjacent terrain tiles crack-free. */
    private const val TERRAIN_HALO_PX = 1
}
