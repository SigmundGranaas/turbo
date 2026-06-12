package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import android.view.Choreographer
import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableIntState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.MapEngine
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.TurbomapScene
import com.sigmundgranaas.turbo.expressive.ui.components.MapOverlay
import com.sigmundgranaas.turbo.expressive.ui.components.PhotoPin
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import kotlin.math.abs
import kotlin.math.ln

/**
 * On-screen wgpu map host (Stage E cutover, **experimental**, flag-gated).
 *
 * Renders the basemap + overlays + live track/route/measure/user as a turbomap
 * [TurbomapScene] (driven by a `SurfaceView` + `Choreographer` loop with
 * host-driven raster tile fetching), and draws markers / editable waypoints /
 * photo pins with the **shared [MapOverlay]** projected through the [MapEngine]
 * seam — so the on-map UI is identical to the MapLibre `TurboMap`. Pan/zoom,
 * tap, and long-press translate to camera + map events; the [TurbomapMapEngine]
 * is handed up via [onMapReady] so the zoom/locate rail works unchanged.
 *
 * All native calls are marshalled to the main thread (the on-screen engine isn't
 * internally locked); only tile HTTP runs off-main, ingesting back on main.
 */
@Composable
@Suppress("LongParameterList")
fun TurbomapMapView(
    rasters: List<TurbomapScene.RasterSpec>,
    initialCamera: LatLng,
    initialZoom: Double,
    modifier: Modifier = Modifier,
    track: List<LatLng>? = null,
    route: List<LatLng>? = null,
    measure: List<LatLng> = emptyList(),
    userLocation: LatLng? = null,
    markers: List<Marker> = emptyList(),
    selectedMarkerId: String? = null,
    markerFallbackColor: Color = Color(0xFF8F4C38),
    photoPins: List<PhotoPin> = emptyList(),
    onPhotoPinClick: (PhotoPin) -> Unit = {},
    waypoints: List<LatLng> = emptyList(),
    selectedWaypoint: Int? = null,
    onWaypointTap: (Int) -> Unit = {},
    onWaypointLongPress: (Int) -> Unit = {},
    onWaypointMoved: (Int, LatLng) -> Unit = { _, _ -> },
    onMarkerClick: (Marker) -> Unit = {},
    onMapLongClick: (LatLng) -> Unit = {},
    onMapTap: ((LatLng) -> Unit)? = null,
    onBearingChange: (Double) -> Unit = {},
    onMapReady: (MapEngine) -> Unit = {},
) {
    val context = LocalContext.current
    val cameraTick = remember { mutableIntStateOf(0) }
    val controller = remember { TurbomapSurfaceController() }
    controller.cameraTick = cameraTick
    controller.onBearingChange = onBearingChange
    controller.cacheDir = remember(context) { File(context.cacheDir, "turbomap-tiles") }
    fun scene() = TurbomapScene.build(rasters, track, route, measure, userLocation)

    Box(modifier.fillMaxSize()) {
        AndroidView(
            factory = { ctx ->
                SurfaceView(ctx).apply {
                    holder.addCallback(object : SurfaceHolder.Callback {
                        override fun surfaceCreated(holder: SurfaceHolder) = Unit
                        override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                            controller.attachOrResize(holder.surface, width, height, initialCamera, initialZoom, scene(), rasters, onMapReady)
                        }
                        override fun surfaceDestroyed(holder: SurfaceHolder) = controller.detach()
                    })
                }
            },
            modifier = Modifier.fillMaxSize(),
        )
        // Pan/zoom + tap/long-press → camera + map events. (Same thread as render.)
        Box(
            Modifier.fillMaxSize()
                .pointerInput(Unit) {
                    detectTransformGestures(onGesture = { _: Offset, pan: Offset, zoom: Float, _: Float ->
                        controller.onTransform(pan.x, pan.y, zoom)
                    })
                }
                .pointerInput(onMapTap, onMapLongClick) {
                    detectTapGestures(
                        onTap = { o -> controller.unproject(o.x, o.y)?.let { onMapTap?.invoke(it) } },
                        onLongPress = { o -> controller.unproject(o.x, o.y)?.let(onMapLongClick) },
                    )
                },
        )
        // Markers / waypoints / photo pins — the shared overlay, projected via the engine.
        controller.engine?.let { engine ->
            MapOverlay(
                engine = engine,
                cameraTick = cameraTick.intValue,
                markers = markers,
                selectedMarkerId = selectedMarkerId,
                markerFallbackColor = markerFallbackColor,
                onMarkerClick = onMarkerClick,
                photoPins = photoPins,
                onPhotoPinClick = onPhotoPinClick,
                waypoints = waypoints,
                selectedWaypoint = selectedWaypoint,
                onWaypointTap = onWaypointTap,
                onWaypointLongPress = onWaypointLongPress,
                onWaypointMoved = onWaypointMoved,
            )
        }
    }

    LaunchedEffect(rasters, track, route, measure, userLocation) {
        controller.applyScene(scene(), rasters)
    }
    DisposableEffect(Unit) { onDispose { controller.detach() } }
}

/**
 * Owns the native on-screen handle + the render/tile loops for one surface.
 * Single-threaded by contract: every `NativeSurfaceMap` call happens on the main
 * (Compose) thread; tile bytes are fetched off-main and ingested back on main.
 */
internal class TurbomapSurfaceController {
    var engine by mutableStateOf<TurbomapMapEngine?>(null)
        private set
    var cameraTick: MutableIntState? = null
    var onBearingChange: (Double) -> Unit = {}
    var cacheDir: File? = null

    private val tileCache: TurbomapTileCache? by lazy { cacheDir?.let { TurbomapTileCache(it) } }

    private var handle = 0L
    private var width = 0
    private var height = 0
    private var rasters: List<TurbomapScene.RasterSpec> = emptyList()
    private val fetched = HashSet<String>()
    private val scope = CoroutineScope(Dispatchers.Main.immediate)
    private var fetchJob: Job? = null
    private var lastBearing = Double.NaN

    private val choreographer = Choreographer.getInstance()
    private var running = false
    private val frame = object : Choreographer.FrameCallback {
        override fun doFrame(frameTimeNanos: Long) {
            if (!running || handle == 0L) return
            NativeSurfaceMap.nativeRender(handle)
            // Re-place the Compose overlays in lockstep with the GPU frame, whatever
            // moved the camera (gesture, zoom rail, fit-to-track). Reading cameraTick in
            // the overlays' `offset {}` makes this a cheap re-layout, not a recompose.
            cameraTick?.let { it.intValue++ }
            val b = engine?.bearing() ?: 0.0
            if (lastBearing.isNaN() || abs(b - lastBearing) > BEARING_EPSILON) {
                lastBearing = b
                onBearingChange(b)
            }
            choreographer.postFrameCallback(this)
        }
    }

    fun attachOrResize(
        surface: android.view.Surface,
        w: Int,
        h: Int,
        camera: LatLng,
        zoom: Double,
        sceneJson: String,
        rasterSpecs: List<TurbomapScene.RasterSpec>,
        onMapReady: (MapEngine) -> Unit,
    ) {
        width = w
        height = h
        rasters = rasterSpecs
        if (handle == 0L) {
            handle = NativeSurfaceMap.nativeCreate(surface, w, h, camera.lat, camera.lng, zoom)
            if (handle == 0L) return
            NativeSurfaceMap.nativeApplyScene(handle, sceneJson)
            NativeSurfaceMap.nativePumpLocal(handle)
            val eng = TurbomapMapEngine(handle, w, h)
            engine = eng
            onMapReady(eng)
            startRenderLoop()
            scheduleTileFetch()
        } else {
            engine?.onResized(w, h)
            NativeSurfaceMap.nativeResize(handle, w, h)
            scheduleTileFetch()
        }
    }

    fun applyScene(sceneJson: String, rasterSpecs: List<TurbomapScene.RasterSpec>) {
        if (handle == 0L) return
        rasters = rasterSpecs
        NativeSurfaceMap.nativeApplyScene(handle, sceneJson)
        NativeSurfaceMap.nativePumpLocal(handle)
        scheduleTileFetch()
    }

    fun onTransform(panX: Float, panY: Float, zoomFactor: Float) {
        if (handle == 0L) return
        val unp = NativeSurfaceMap.nativeUnproject(handle, width / 2.0 - panX, height / 2.0 - panY)
        val cam = NativeSurfaceMap.nativeCamera(handle)
        if (cam.size < 4) return
        val newCenter = if (unp.size >= 3 && unp[2] == 1.0) LatLng(unp[0], unp[1]) else LatLng(cam[0], cam[1])
        var z = cam[2]
        if (zoomFactor != 1f) z = (z + ln(zoomFactor.toDouble()) / LN2).coerceIn(MIN_ZOOM, MAX_ZOOM)
        NativeSurfaceMap.nativeSetCamera(handle, newCenter.lat, newCenter.lng, z, cam[3])
        scheduleTileFetch()
    }

    /** Geographic point under a screen pixel, or null before the map is ready. */
    fun unproject(xPx: Float, yPx: Float): LatLng? {
        if (handle == 0L) return null
        val r = NativeSurfaceMap.nativeUnproject(handle, xPx.toDouble(), yPx.toDouble())
        return if (r.size >= 3 && r[2] == 1.0) LatLng(r[0], r[1]) else null
    }

    private data class TileFetch(val layer: String, val z: Int, val x: Int, val y: Int, val key: String, val url: String)

    /** Fetch any pending raster tiles host-side, ingesting back on the main thread. */
    private fun scheduleTileFetch() {
        if (handle == 0L) return
        fetchJob?.cancel()
        fetchJob = scope.launch {
            val pending = NativeSurfaceMap.nativePendingTilesJson(handle)
            val arr = runCatching { JSONArray(pending) }.getOrNull() ?: return@launch
            val toFetch = (0 until arr.length()).mapNotNull { i ->
                val o = arr.optJSONObject(i)
                val layer = o?.optString("layer").orEmpty()
                val template = rasters.firstOrNull { it.id == layer }?.tileUrlTemplate
                val z = o?.optInt("z") ?: 0
                val x = o?.optInt("x") ?: 0
                val y = o?.optInt("y") ?: 0
                val key = "$layer/$z/$x/$y"
                if (o == null || o.optString("kind") != "raster" || template == null || !fetched.add(key)) {
                    null
                } else {
                    TileFetch(layer, z, x, y, key, template.replace("{z}", "$z").replace("{x}", "$x").replace("{y}", "$y"))
                }
            }
            for (t in toFetch) {
                // Read-through disk cache: serve a previously-fetched tile offline, else
                // fetch and persist it (the host owns caching, per the pull/push contract).
                val bytes = withContext(Dispatchers.IO) {
                    tileCache?.get(t.layer, t.z, t.x, t.y)
                        ?: fetchBytes(t.url)?.also { tileCache?.put(t.layer, t.z, t.x, t.y, it) }
                }
                if (bytes != null && handle != 0L) {
                    NativeSurfaceMap.nativeIngestRaster(handle, t.layer, t.z, t.x, t.y, bytes)
                } else {
                    fetched.remove(t.key)
                }
            }
        }
    }

    fun detach() {
        running = false
        choreographer.removeFrameCallback(frame)
        fetchJob?.cancel()
        val h = handle
        handle = 0L
        engine = null
        if (h != 0L) NativeSurfaceMap.nativeDestroy(h)
        fetched.clear()
    }

    private fun startRenderLoop() {
        if (running) return
        running = true
        choreographer.postFrameCallback(frame)
    }

    private fun fetchBytes(url: String): ByteArray? = runCatching {
        val conn = (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = TIMEOUT_MS
            readTimeout = TIMEOUT_MS
            setRequestProperty("User-Agent", "turbo-android-wgpu")
        }
        try {
            if (conn.responseCode in 200..299) conn.inputStream.use { it.readBytes() } else null
        } finally {
            conn.disconnect()
        }
    }.getOrNull()

    private companion object {
        const val LN2 = 0.6931471805599453
        const val MIN_ZOOM = 1.0
        const val MAX_ZOOM = 20.0
        const val TIMEOUT_MS = 10_000
        const val BEARING_EPSILON = 0.01
    }
}
