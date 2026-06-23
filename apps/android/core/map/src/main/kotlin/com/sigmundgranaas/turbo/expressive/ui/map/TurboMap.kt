package com.sigmundgranaas.turbo.expressive.ui.map

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material3.Icon
import androidx.compose.ui.Alignment
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.produceState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.MapEngine
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.ui.components.MapOverlay
import com.sigmundgranaas.turbo.expressive.ui.components.PhotoPin
import com.sigmundgranaas.turbo.expressive.ui.theme.icon
import org.maplibre.android.MapLibre
import org.maplibre.android.camera.CameraUpdateFactory
import org.maplibre.android.maps.MapLibreMap
import org.maplibre.android.maps.MapView
import org.maplibre.android.maps.Style
import org.maplibre.android.style.layers.CircleLayer
import org.maplibre.android.style.layers.LineLayer
import org.maplibre.android.style.layers.Property
import org.maplibre.android.style.layers.PropertyFactory
import org.maplibre.android.style.sources.GeoJsonSource
import org.maplibre.geojson.Feature
import org.maplibre.geojson.FeatureCollection
import org.maplibre.geojson.LineString
import org.maplibre.geojson.Point
import kotlin.math.roundToInt
import org.maplibre.android.geometry.LatLng as MlLatLng

/**
 * The MapLibre implementation of the renderer-agnostic [MapEngine] seam — a thin
 * camera/projection controller over `MapLibreMap`, handed up to the screen for the
 * zoom/locate rail and the route/measure/offline tools. The wgpu `TurbomapEngine`
 * will be a second [MapEngine] behind the same seam (see
 * `docs/architecture/2026-06-android-renderer-swap-test-plan.md`).
 */
class MapLibreEngine(internal val map: MapLibreMap) : MapEngine {
    override fun zoomIn() = map.animateCamera(CameraUpdateFactory.zoomBy(1.0))
    override fun zoomOut() = map.animateCamera(CameraUpdateFactory.zoomBy(-1.0))
    override fun flyTo(target: LatLng, zoom: Double) =
        map.animateCamera(CameraUpdateFactory.newLatLngZoom(MlLatLng(target.lat, target.lng), zoom))

    override fun center(): LatLng = map.cameraPosition.target.let { LatLng(it!!.latitude, it.longitude) }

    override fun fromScreen(xPx: Float, yPx: Float): LatLng =
        map.projection.fromScreenLocation(android.graphics.PointF(xPx, yPx)).let { LatLng(it.latitude, it.longitude) }

    // MapLibre is a 2D top-down renderer with no relief, so the terrain hit is the
    // flat-plane unproject — keeps the MapEngine seam total across both hosts.
    override fun screenToGround(xPx: Float, yPx: Float): LatLng = fromScreen(xPx, yPx)

    override fun toScreen(point: LatLng): Pair<Float, Float> =
        map.projection.toScreenLocation(MlLatLng(point.lat, point.lng)).let { it.x to it.y }

    override fun visibleBounds(): GeoBounds {
        val b = map.projection.visibleRegion.latLngBounds
        return GeoBounds(south = b.latitudeSouth, west = b.longitudeWest, north = b.latitudeNorth, east = b.longitudeEast)
    }

    override fun setBottomInset(bottomPx: Int) = map.setPadding(0, 0, 0, bottomPx.coerceAtLeast(0))

    override fun zoom(): Double = map.cameraPosition.zoom

    override fun bearing(): Double = map.cameraPosition.bearing

    override fun resetNorth() = map.animateCamera(CameraUpdateFactory.bearingTo(0.0))

    override fun frameTo(points: List<LatLng>, paddingPx: Int) {
        when {
            points.isEmpty() -> Unit
            points.size == 1 -> flyTo(points.first(), 14.0)
            else -> {
                val bounds = org.maplibre.android.geometry.LatLngBounds.Builder()
                    .apply { points.forEach { include(MlLatLng(it.lat, it.lng)) } }
                    .build()
                map.animateCamera(CameraUpdateFactory.newLatLngBounds(bounds, paddingPx))
            }
        }
    }
}

/**
 * Full-bleed MapLibre map with the base map [base], plus a Compose overlay that
 * projects [markers] (as [MarkerPin]s) and an optional [route] polyline onto the
 * live camera. The map raster keeps its natural brightness in both themes — only
 * the chrome floating above it flips.
 */
@Composable
fun TurboMap(
    base: BaseLayer,
    initialCamera: LatLng,
    initialZoom: Double,
    modifier: Modifier = Modifier,
    overlays: Set<com.sigmundgranaas.turbo.expressive.domain.OverlayId> = emptySet(),
    markers: List<Marker> = emptyList(),
    route: List<LatLng>? = null,
    routeColor: Color = Color(0xFF8F4C38),
    /** The already-walked prefix of [route] while following — drawn dim over the guide (US-3). */
    routeCovered: List<LatLng>? = null,
    track: List<LatLng>? = null,
    trackColor: Color = Color(0xFF00696D),
    measurePoints: List<LatLng> = emptyList(),
    measureColor: Color = Color(0xFF00696D),
    selectedMarkerId: String? = null,
    userLocation: LatLng? = null,
    photoPins: List<PhotoPin> = emptyList(),
    onPhotoPinClick: (PhotoPin) -> Unit = {},
    // ── Editable route waypoints (the "Manage track" on-map editor) ──
    // A/B/C… + flag markers along [route]; tap = select, drag the selected one = move,
    // long-press = remove. Empty unless the route builder is active.
    waypoints: List<LatLng> = emptyList(),
    /** Follow-mode checkpoints (position → crossed) drawn as on-map markers (US-3). */
    checkpoints: List<Pair<LatLng, Boolean>> = emptyList(),
    selectedWaypoint: Int? = null,
    onWaypointTap: (Int) -> Unit = {},
    onWaypointLongPress: (Int) -> Unit = {},
    onWaypointMoved: (Int, LatLng) -> Unit = { _, _ -> },
    onWaypointDragStart: (Int) -> Unit = {},
    onWaypointDragEnd: (Int) -> Unit = {},
    /** Pending route origin (first point before a destination exists) → drawn as an origin pin. */
    routeOrigin: LatLng? = null,
    onMarkerClick: (Marker) -> Unit = {},
    onMapLongClick: (LatLng) -> Unit = {},
    onMapTap: ((LatLng) -> Unit)? = null,
    onMapReady: (MapEngine) -> Unit = {},
    onBearingChange: (Double) -> Unit = {},
    /// Fired when the USER pans/zooms/rotates the map (not a programmatic move) —
    /// the signal to release camera-follow (US-6).
    onUserPanned: () -> Unit = {},
) {
    val userPanned by rememberUpdatedState(onUserPanned)
    val longClick by rememberUpdatedState(onMapLongClick)
    val tap by rememberUpdatedState(onMapTap)
    val context = LocalContext.current
    val density = LocalDensity.current
    val mapView = rememberMapViewWithLifecycle()
    var map by remember { mutableStateOf<MapLibreMap?>(null) }
    val cameraTick = remember { mutableIntStateOf(0) }
    val moving = remember { mutableStateOf(false) }
    var styledBase by remember { mutableStateOf<BaseLayer?>(null) }
    var styledOverlays by remember { mutableStateOf<Set<com.sigmundgranaas.turbo.expressive.domain.OverlayId>>(emptySet()) }
    // The loaded style — on-map geometry (track/route/measure/user) is rendered as native
    // MapLibre layers on it, so it moves in the same GL frame as the base map (no drift).
    var style by remember { mutableStateOf<Style?>(null) }

    Box(modifier = modifier) {
        AndroidView(factory = {
            MapLibre.getInstance(context)
            mapView.apply {
                getMapAsync { ml ->
                    map = ml
                    // We surface a compass in our own (properly-inset, tappable) control
                    // rail, so suppress MapLibre's default top-right widget that otherwise
                    // lands under the status bar / search pill.
                    ml.uiSettings.isCompassEnabled = false
                    // 2D map (MapLibre host) never rotates — pan + zoom only. Rotation
                    // belongs to the 3D (wgpu) mode's two-finger twist. Matches the
                    // turbomap-host gesture model.
                    ml.uiSettings.isRotateGesturesEnabled = false
                    ml.setStyle(Style.Builder().fromJson(MapStyles.styleJson(base, overlays))) { loaded ->
                        loaded.installTurboLayers(trackColor, routeColor, measureColor)
                        style = loaded
                        styledBase = base
                        styledOverlays = overlays
                    }
                    ml.cameraPosition = org.maplibre.android.camera.CameraPosition.Builder()
                        .target(MlLatLng(initialCamera.lat, initialCamera.lng))
                        .zoom(initialZoom)
                        .build()
                    // While the camera is moving we reproject the Compose overlay from the
                    // frame clock (see the LaunchedEffect below) so pins/route track the GL
                    // base map in lockstep. The listeners just gate that loop + keep a final
                    // frame on idle; bumping cameraTick straight from the move listener lands
                    // a frame late, which is what made the overlay float/drift.
                    ml.addOnCameraMoveStartedListener { reason ->
                        moving.value = true
                        // 1 == OnCameraMoveStartedListener.REASON_API_GESTURE (user-initiated);
                        // programmatic follow/flyTo uses other reasons, so it won't self-cancel.
                        if (reason == 1) userPanned()
                    }
                    ml.addOnCameraMoveListener { onBearingChange(ml.cameraPosition.bearing) }
                    ml.addOnCameraIdleListener {
                        moving.value = false
                        cameraTick.intValue++
                        onBearingChange(ml.cameraPosition.bearing)
                    }
                    ml.addOnMapLongClickListener { point ->
                        longClick(LatLng(point.latitude, point.longitude))
                        true
                    }
                    ml.addOnMapClickListener { point ->
                        val onTap = tap
                        if (onTap != null) { onTap(LatLng(point.latitude, point.longitude)); true } else false
                    }
                    onMapReady(MapLibreEngine(ml))
                }
            }
        })

        // Drive overlay reprojection from the frame clock while the camera moves, so the
        // Compose pins/route advance on the same VSYNC as the GL map (no float/drift).
        LaunchedEffect(moving.value) {
            if (moving.value) {
                while (true) {
                    withFrameNanos { }
                    cameraTick.intValue++
                }
            }
        }

        // Re-style when the base layer or the data overlay changes.
        val ml = map
        if (ml != null && (styledBase != base || styledOverlays != overlays)) {
            style = null // the old Style is torn down; re-installed in the callback below
            ml.setStyle(Style.Builder().fromJson(MapStyles.styleJson(base, overlays))) { loaded ->
                loaded.installTurboLayers(trackColor, routeColor, measureColor)
                style = loaded
                styledBase = base
                styledOverlays = overlays
            }
        }

        // Push current geometry into the native sources whenever the data (or style) changes.
        LaunchedEffect(style, track, route, routeCovered, measurePoints, userLocation) {
            val s = style ?: return@LaunchedEffect
            s.getSourceAs<GeoJsonSource>(SRC_TRACK)?.setGeoJson(lineFc(track))
            s.getSourceAs<GeoJsonSource>(SRC_ROUTE)?.setGeoJson(lineFc(route))
            s.getSourceAs<GeoJsonSource>(SRC_ROUTE_COVERED)?.setGeoJson(lineFc(routeCovered))
            s.getSourceAs<GeoJsonSource>(SRC_MEASURE_LINE)?.setGeoJson(lineFc(measurePoints))
            s.getSourceAs<GeoJsonSource>(SRC_MEASURE_PTS)?.setGeoJson(pointsFc(measurePoints))
            s.getSourceAs<GeoJsonSource>(SRC_USER)?.setGeoJson(pointsFc(listOfNotNull(userLocation)))
        }

        // ---- Native MapLibre layers render track/route/measure/user (see LaunchedEffect
        // above). Markers + scale bar stay Compose, reprojected on every camera change. ----
        if (ml != null) {
            // Scale bar (bottom-left), recomputed on camera change from centre lat + zoom.
            run {
                @Suppress("UNUSED_EXPRESSION") cameraTick.intValue
                val cam = ml.cameraPosition
                val lat = cam.target?.latitude ?: initialCamera.lat
                val maxPx = with(density) { 96.dp.toPx() }
                val spec = ScaleBar.compute(lat, cam.zoom, maxPx)
                val widthDp = with(density) { spec.widthPx.toDp() }
                Box(
                    Modifier
                        .align(androidx.compose.ui.Alignment.BottomStart)
                        .padding(start = 12.dp, bottom = 64.dp),
                ) {
                    Box(
                        Modifier
                            .background(Color.White.copy(alpha = 0.75f), shape = androidx.compose.foundation.shape.RoundedCornerShape(4.dp))
                            .padding(horizontal = 6.dp, vertical = 3.dp),
                    ) {
                        androidx.compose.foundation.layout.Column(horizontalAlignment = androidx.compose.ui.Alignment.Start) {
                            androidx.compose.material3.Text(
                                spec.label,
                                style = androidx.compose.material3.MaterialTheme.typography.labelSmall,
                                color = Color(0xFF1A1A1A),
                            )
                            Box(
                                Modifier
                                    .padding(top = 2.dp)
                                    .size(width = widthDp.coerceAtLeast(8.dp), height = 3.dp)
                                    .background(Color(0xFF1A1A1A)),
                            )
                        }
                    }
                }
            }
            // Markers, photo pins, and editable waypoints — the shared overlay layer,
            // projected through the MapEngine seam (pixel-identical on the wgpu host).
            val overlayEngine = remember(ml) { MapLibreEngine(ml) }
            MapOverlay(
                engine = overlayEngine,
                cameraTick = cameraTick.intValue,
                markers = markers,
                selectedMarkerId = selectedMarkerId,
                markerFallbackColor = routeColor,
                onMarkerClick = onMarkerClick,
                photoPins = photoPins,
                onPhotoPinClick = onPhotoPinClick,
                waypoints = waypoints,
                selectedWaypoint = selectedWaypoint,
                onWaypointTap = onWaypointTap,
                onWaypointLongPress = onWaypointLongPress,
                onWaypointMoved = onWaypointMoved,
                onWaypointDragStart = onWaypointDragStart,
                onWaypointDragEnd = onWaypointDragEnd,
                routeOrigin = routeOrigin,
                checkpoints = checkpoints,
            )
        }
    }
}

private const val SRC_TRACK = "turbo-track-src"
private const val SRC_ROUTE = "turbo-route-src"
private const val SRC_ROUTE_COVERED = "turbo-route-covered-src"
private const val ROUTE_COVERED_GRAY = 0xFF9AA0A6.toInt()
private const val SRC_MEASURE_LINE = "turbo-measure-line-src"
private const val SRC_MEASURE_PTS = "turbo-measure-pts-src"
private const val SRC_USER = "turbo-user-src"
private const val USER_BLUE = 0xFF1A73E8.toInt()
private const val WHITE = 0xFFFFFFFF.toInt()

/**
 * Install the empty GeoJSON sources + line/circle layers for the on-map geometry, once
 * per loaded [Style]. Idempotent (no-op if already present). Data is pushed in later.
 */
private fun Style.installTurboLayers(
    trackColor: Color,
    routeColor: Color,
    measureColor: Color,
) {
    if (getSource(SRC_TRACK) != null) return
    listOf(SRC_TRACK, SRC_ROUTE, SRC_ROUTE_COVERED, SRC_MEASURE_LINE, SRC_MEASURE_PTS, SRC_USER)
        .forEach { addSource(GeoJsonSource(it)) }

    // MapLibre line-width / circle-radius are in density-independent (logical) pixels —
    // the renderer applies the display pixel-ratio itself, so these are dp-equivalent and
    // must NOT be pre-multiplied by density (that double-scaled the line + user dot huge).
    fun line(id: String, src: String, color: Color, widthDp: Float) = LineLayer(id, src).withProperties(
        PropertyFactory.lineColor(color.toArgb()),
        PropertyFactory.lineWidth(widthDp),
        PropertyFactory.lineCap(Property.LINE_CAP_ROUND),
        PropertyFactory.lineJoin(Property.LINE_JOIN_ROUND),
    )
    addLayer(line("turbo-route-layer", SRC_ROUTE, routeColor, 4f))
    // The covered prefix of the guide, drawn dim ABOVE the bright route so the walked
    // portion reads as "done" while the road ahead stays highlighted (US-3).
    addLayer(
        LineLayer("turbo-route-covered-layer", SRC_ROUTE_COVERED).withProperties(
            PropertyFactory.lineColor(ROUTE_COVERED_GRAY),
            PropertyFactory.lineWidth(4f),
            PropertyFactory.lineOpacity(0.9f),
            PropertyFactory.lineCap(Property.LINE_CAP_ROUND),
            PropertyFactory.lineJoin(Property.LINE_JOIN_ROUND),
        ),
    )
    // The real travelled track sits on top of both, so you always see where you actually went.
    addLayer(line("turbo-track-layer", SRC_TRACK, trackColor, 4f))
    addLayer(line("turbo-measure-line-layer", SRC_MEASURE_LINE, measureColor, 3f))
    addLayer(
        CircleLayer("turbo-measure-pts-layer", SRC_MEASURE_PTS).withProperties(
            PropertyFactory.circleRadius(4f),
            PropertyFactory.circleColor(measureColor.toArgb()),
            PropertyFactory.circleStrokeColor(WHITE),
            PropertyFactory.circleStrokeWidth(1.5f),
        ),
    )
    addLayer(
        CircleLayer("turbo-user-layer", SRC_USER).withProperties(
            PropertyFactory.circleRadius(7f),
            PropertyFactory.circleColor(USER_BLUE),
            PropertyFactory.circleStrokeColor(WHITE),
            PropertyFactory.circleStrokeWidth(3f),
        ),
    )
}

/** A one-feature collection holding the polyline (empty if fewer than 2 points). */
private fun lineFc(points: List<LatLng>?): FeatureCollection {
    val pts = points.orEmpty()
    if (pts.size < 2) return FeatureCollection.fromFeatures(emptyArray<Feature>())
    val line = LineString.fromLngLats(pts.map { Point.fromLngLat(it.lng, it.lat) })
    return FeatureCollection.fromFeature(Feature.fromGeometry(line))
}

/** A feature collection of point markers. */
private fun pointsFc(points: List<LatLng>): FeatureCollection =
    FeatureCollection.fromFeatures(points.map { Feature.fromGeometry(Point.fromLngLat(it.lng, it.lat)) })

/** Creates a [MapView] bound to the current lifecycle (forwards all callbacks). */
@Composable
private fun rememberMapViewWithLifecycle(): MapView {
    val context = LocalContext.current
    val mapView = remember {
        MapLibre.getInstance(context)
        MapView(context).apply { onCreate(null) }
    }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    DisposableEffect(lifecycle) {
        // The observer only fires future events; catch up to the current state so
        // the GL surface actually starts rendering when added to a RESUMED activity.
        if (lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) mapView.onStart()
        if (lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) mapView.onResume()
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_START -> mapView.onStart()
                Lifecycle.Event.ON_RESUME -> mapView.onResume()
                Lifecycle.Event.ON_PAUSE -> mapView.onPause()
                Lifecycle.Event.ON_STOP -> mapView.onStop()
                Lifecycle.Event.ON_DESTROY -> mapView.onDestroy()
                else -> {}
            }
        }
        lifecycle.addObserver(observer)
        onDispose {
            lifecycle.removeObserver(observer)
            mapView.onStop()
            mapView.onDestroy()
        }
    }
    return mapView
}
