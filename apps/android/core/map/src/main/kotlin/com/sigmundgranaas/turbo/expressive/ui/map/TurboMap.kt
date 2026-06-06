package com.sigmundgranaas.turbo.expressive.ui.map

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.draw.clip
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
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
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.ui.components.MarkerPin
import com.sigmundgranaas.turbo.expressive.ui.theme.icon
import org.maplibre.android.MapLibre
import org.maplibre.android.camera.CameraUpdateFactory
import org.maplibre.android.maps.MapLibreMap
import org.maplibre.android.maps.MapView
import org.maplibre.android.maps.Style
import kotlin.math.roundToInt
import org.maplibre.android.geometry.LatLng as MlLatLng

/** Thin camera controller handed up to the screen for the zoom/locate buttons. */
class MapController(internal val map: MapLibreMap) {
    fun zoomIn() = map.animateCamera(CameraUpdateFactory.zoomBy(1.0))
    fun zoomOut() = map.animateCamera(CameraUpdateFactory.zoomBy(-1.0))
    fun flyTo(target: LatLng, zoom: Double) =
        map.animateCamera(CameraUpdateFactory.newLatLngZoom(MlLatLng(target.lat, target.lng), zoom))

    /** The current camera centre — a sensible route origin when there's no GPS fix. */
    fun center(): LatLng = map.cameraPosition.target.let { LatLng(it!!.latitude, it.longitude) }

    /** The currently visible lat/lng box — the area to download for offline use. */
    fun visibleBounds(): GeoBounds {
        val b = map.projection.visibleRegion.latLngBounds
        return GeoBounds(south = b.latitudeSouth, west = b.longitudeWest, north = b.latitudeNorth, east = b.longitudeEast)
    }

    /** Current camera zoom level. */
    fun zoom(): Double = map.cameraPosition.zoom
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
    overlay: com.sigmundgranaas.turbo.expressive.domain.OverlayId? = null,
    markers: List<Marker> = emptyList(),
    route: List<LatLng>? = null,
    routeColor: Color = Color(0xFF8F4C38),
    measurePoints: List<LatLng> = emptyList(),
    measureColor: Color = Color(0xFF00696D),
    selectedMarkerId: String? = null,
    userLocation: LatLng? = null,
    onMarkerClick: (Marker) -> Unit = {},
    onMapLongClick: (LatLng) -> Unit = {},
    onMapTap: ((LatLng) -> Unit)? = null,
    onMapReady: (MapController) -> Unit = {},
) {
    val longClick by rememberUpdatedState(onMapLongClick)
    val tap by rememberUpdatedState(onMapTap)
    val context = LocalContext.current
    val density = LocalDensity.current
    val mapView = rememberMapViewWithLifecycle()
    var map by remember { mutableStateOf<MapLibreMap?>(null) }
    val cameraTick = remember { mutableIntStateOf(0) }
    var styledBase by remember { mutableStateOf<BaseLayer?>(null) }
    var styledOverlay by remember { mutableStateOf<com.sigmundgranaas.turbo.expressive.domain.OverlayId?>(null) }

    Box(modifier = modifier) {
        AndroidView(factory = {
            MapLibre.getInstance(context)
            mapView.apply {
                getMapAsync { ml ->
                    map = ml
                    ml.setStyle(Style.Builder().fromJson(MapStyles.styleJson(base, overlay))) {
                        styledBase = base
                        styledOverlay = overlay
                    }
                    ml.cameraPosition = org.maplibre.android.camera.CameraPosition.Builder()
                        .target(MlLatLng(initialCamera.lat, initialCamera.lng))
                        .zoom(initialZoom)
                        .build()
                    ml.addOnCameraMoveListener { cameraTick.intValue++ }
                    ml.addOnCameraIdleListener { cameraTick.intValue++ }
                    ml.addOnMapLongClickListener { point ->
                        longClick(LatLng(point.latitude, point.longitude))
                        true
                    }
                    ml.addOnMapClickListener { point ->
                        val onTap = tap
                        if (onTap != null) { onTap(LatLng(point.latitude, point.longitude)); true } else false
                    }
                    onMapReady(MapController(ml))
                }
            }
        })

        // Re-style when the base layer or the data overlay changes.
        val ml = map
        if (ml != null && (styledBase != base || styledOverlay != overlay)) {
            ml.setStyle(Style.Builder().fromJson(MapStyles.styleJson(base, overlay))) {
                styledBase = base
                styledOverlay = overlay
            }
        }

        // ---- Compose overlay: route + pins, reprojected on every camera change ----
        if (ml != null) {
            if (route != null && route.size > 1) {
                Canvas(modifier = Modifier.matchParentSize()) {
                    @Suppress("UNUSED_EXPRESSION") cameraTick.intValue // invalidate on camera move
                    val proj = ml.projection
                    val path = Path()
                    route.forEachIndexed { i, p ->
                        val pt = proj.toScreenLocation(MlLatLng(p.lat, p.lng))
                        if (i == 0) path.moveTo(pt.x, pt.y) else path.lineTo(pt.x, pt.y)
                    }
                    drawPath(path, color = routeColor, style = Stroke(width = 5.dp.toPx()))
                }
            }
            // Measuring tool: dashed-feel polyline + a dot at each tapped vertex.
            if (measurePoints.isNotEmpty()) {
                Canvas(modifier = Modifier.matchParentSize()) {
                    @Suppress("UNUSED_EXPRESSION") cameraTick.intValue
                    val proj = ml.projection
                    if (measurePoints.size > 1) {
                        val path = Path()
                        measurePoints.forEachIndexed { i, p ->
                            val pt = proj.toScreenLocation(MlLatLng(p.lat, p.lng))
                            if (i == 0) path.moveTo(pt.x, pt.y) else path.lineTo(pt.x, pt.y)
                        }
                        drawPath(path, color = measureColor, style = Stroke(width = 4.dp.toPx()))
                    }
                    measurePoints.forEach { p ->
                        val pt = proj.toScreenLocation(MlLatLng(p.lat, p.lng))
                        drawCircle(Color.White, radius = 6.dp.toPx(), center = androidx.compose.ui.geometry.Offset(pt.x, pt.y))
                        drawCircle(measureColor, radius = 4.dp.toPx(), center = androidx.compose.ui.geometry.Offset(pt.x, pt.y))
                    }
                }
            }
            // User location: a blue dot with a white ring, projected like markers.
            if (userLocation != null) {
                val dotPx = with(density) { 18.dp.toPx() }
                Box(
                    Modifier
                        .offset {
                            @Suppress("UNUSED_EXPRESSION") cameraTick.intValue
                            val pt = ml.projection.toScreenLocation(MlLatLng(userLocation.lat, userLocation.lng))
                            IntOffset((pt.x - dotPx / 2f).roundToInt(), (pt.y - dotPx / 2f).roundToInt())
                        }
                        .size(18.dp)
                        .clip(CircleShape)
                        .background(Color.White)
                        .padding(3.dp)
                        .clip(CircleShape)
                        .background(Color(0xFF1A73E8)),
                )
            }
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
            markers.forEach { m ->
                val selected = m.id == selectedMarkerId
                val boxPx = with(density) { (if (selected) 42.dp else 33.dp).toPx() }
                MarkerPin(
                    icon = m.kind.icon,
                    selected = selected,
                    color = m.colorArgb?.let { Color(it) } ?: routeColor,
                    modifier = Modifier
                        .offset {
                            @Suppress("UNUSED_EXPRESSION") cameraTick.intValue
                            val pt = ml.projection.toScreenLocation(MlLatLng(m.position.lat, m.position.lng))
                            IntOffset((pt.x - boxPx / 2f).roundToInt(), (pt.y - boxPx).roundToInt())
                        }
                        .clickable { onMarkerClick(m) },
                )
            }
        }
    }
}

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
