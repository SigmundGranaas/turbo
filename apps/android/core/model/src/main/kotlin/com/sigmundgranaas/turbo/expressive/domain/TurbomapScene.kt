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

    /**
     * A route/track drawn as a raised 3D `tube` layer — a single lit mesh,
     * occluded by relief, constant on-screen radius. Scene-declared content
     * like every other layer (plan P5.2); replaces the old imperative
     * `nativeSetRouteTube` side-door. Fewer than 2 points = omitted.
     */
    data class TubeSpec(
        val id: String,
        val points: List<LatLng>,
        val color: Rgba,
        val radiusPx: Double,
    )

    /**
     * The scene-declared weather-cloud overlay: WHAT renders (a `field-2d`
     * radar source of [gridW]×[gridH] cells), WHERE (its geo box), and
     * WHETHER it shows. Frame DATA still pushes through
     * `nativeIngestRadarFrame` (transport, like tiles) and the playback
     * clock stays the `nativeSetCloudTime` control verb — everything a
     * scene diff should reproduce is here (plan P5.2).
     */
    data class CloudsSpec(
        val gridW: Int,
        val gridH: Int,
        val west: Double,
        val south: Double,
        val east: Double,
        val north: Double,
        val visible: Boolean = true,
    )

    /**
     * The scene-declared environment (plan P5.2: ONE content plane — these
     * were the imperative `nativeSetSunTime`/`nativeSetTerrainShadows`/
     * `nativeEnableClouds`… side-doors). Defaults match a fresh engine, so
     * an unspecified environment is a no-op.
     */
    data class EnvironmentSpec(
        /** Track the sun (terrain shading + sky) to this UTC instant;
         *  null = the engine's fixed default light. */
        val sunUnixSeconds: Double? = null,
        /** Terrain cast-shadow strength in `[0,1]`; 0 = off. */
        val terrainShadows: Float = 0f,
        val clouds: CloudsSpec? = null,
    )

    /** Parse "#RRGGBB" (case-insensitive, '#' optional) into an [Rgba]; null when malformed.
     *  The inverse of the "#RRGGBB" `colorHex` convention tracks sync with. */
    fun rgbaFromHex(hex: String): Rgba? {
        val h = hex.removePrefix("#")
        if (h.length != 6 || h.any { it.digitToIntOrNull(16) == null }) return null
        return Rgba(
            r = h.substring(0, 2).toInt(16),
            g = h.substring(2, 4).toInt(16),
            b = h.substring(4, 6).toInt(16),
        )
    }

    val TrackColor = Rgba(0, 105, 109)
    val RouteColor = Rgba(143, 76, 56)
    /** The dim already-walked segment in follow mode — a muted, desaturated
     *  [RouteColor] so the bright remaining-ahead route stands out (US-3). */
    val RouteCoveredColor = Rgba(150, 128, 120)
    val MeasureColor = Rgba(0, 105, 109)
    val UserColor = Rgba(26, 115, 232)

    /**
     * Build the Scene-IR JSON for the given map state. Layers are emitted only
     * when they have renderable geometry, so the result always validates (every
     * layer's source exists).
     */
    // Track + route render as raised 3D [TubeSpec] layers (a single lit mesh,
    // not a per-tile flat line); measure stays a flat geojson line. The live
    // user position is a Compose pin, not a layer.
    @Suppress("LongParameterList", "CyclomaticComplexMethod")
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
        tubes: List<TubeSpec> = emptyList(),
        environment: EnvironmentSpec = EnvironmentSpec(),
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

        // Route/track tubes: over the ground stack (rasters/hillshade/vectors),
        // under the measure annotations. Field names match the Rust IR's
        // `Layer::Tube` (`radius_px`; a plain colour, not a paint).
        tubes.forEach { t ->
            if (t.points.size < 2) return@forEach
            val src = "t_${t.id}"
            sources += "\"$src\": { \"type\": \"geo-json\", \"data\": \"${escape(lineString(t.points))}\" }"
            layers += "{ \"type\": \"tube\", \"id\": \"${t.id}\", \"source\": \"$src\", " +
                "\"color\": ${rgba(t.color)}, \"radius_px\": ${t.radiusPx} }"
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

        // The environment block. Keys are the Rust IR's kebab-case
        // (`EnvironmentDef` is `rename_all = "kebab-case"`); the lighting
        // enum's variant FIELDS keep their snake_case Rust names. Fields at
        // their engine-neutral default are omitted (serde defaults them).
        val envParts = mutableListOf<String>()
        environment.sunUnixSeconds?.let {
            envParts += "\"lighting\": { \"mode\": \"time-tracked\", \"unix_seconds\": $it }"
        }
        if (environment.terrainShadows > 0f) {
            envParts += "\"terrain-shadows\": ${environment.terrainShadows}"
        }
        environment.clouds?.let { c ->
            // The radar grid is a scene SOURCE (`field-2d`); its bounds anchor
            // the overlay geographically (world-locked clouds). `animate` is
            // false: this host scrubs the clock via `nativeSetCloudTime`.
            sources += "\"radar\": { \"type\": \"field-2d\", " +
                "\"bounds\": [${c.west}, ${c.south}, ${c.east}, ${c.north}] }"
            envParts += "\"clouds\": { \"source\": \"radar\", \"grid\": [${c.gridW}, ${c.gridH}], " +
                "\"visible\": ${c.visible}, \"animate\": false }"
        }
        val environmentJson =
            if (envParts.isEmpty()) "" else ", \"environment\": { ${envParts.joinToString(", ")} }"

        return "{ \"sources\": { ${sources.joinToString(", ")} }, " +
            "\"layers\": [ ${layers.joinToString(", ")} ]$environmentJson }"
    }

    private fun const(c: Rgba) = "{ \"const\": { \"r\": ${c.r}, \"g\": ${c.g}, \"b\": ${c.b}, \"a\": ${c.a} } }"

    private fun rgba(c: Rgba) = "{ \"r\": ${c.r}, \"g\": ${c.g}, \"b\": ${c.b}, \"a\": ${c.a} }"

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
