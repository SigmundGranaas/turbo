package com.sigmundgranaas.turbo.expressive.ui.map

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
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
    markers: List<Marker> = emptyList(),
    route: List<LatLng>? = null,
    routeColor: Color = Color(0xFF8F4C38),
    selectedMarkerId: String? = null,
    onMarkerClick: (Marker) -> Unit = {},
    onMapReady: (MapController) -> Unit = {},
) {
    val context = LocalContext.current
    val density = LocalDensity.current
    val mapView = rememberMapViewWithLifecycle()
    var map by remember { mutableStateOf<MapLibreMap?>(null) }
    val cameraTick = remember { mutableIntStateOf(0) }
    var styledBase by remember { mutableStateOf<BaseLayer?>(null) }

    Box(modifier = modifier) {
        AndroidView(factory = {
            MapLibre.getInstance(context)
            mapView.apply {
                getMapAsync { ml ->
                    map = ml
                    ml.setStyle(Style.Builder().fromJson(MapStyles.styleJson(base))) {
                        styledBase = base
                    }
                    ml.cameraPosition = org.maplibre.android.camera.CameraPosition.Builder()
                        .target(MlLatLng(initialCamera.lat, initialCamera.lng))
                        .zoom(initialZoom)
                        .build()
                    ml.addOnCameraMoveListener { cameraTick.intValue++ }
                    ml.addOnCameraIdleListener { cameraTick.intValue++ }
                    onMapReady(MapController(ml))
                }
            }
        })

        // Re-style when the base layer changes.
        val ml = map
        if (ml != null && styledBase != base) {
            ml.setStyle(Style.Builder().fromJson(MapStyles.styleJson(base))) { styledBase = base }
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
