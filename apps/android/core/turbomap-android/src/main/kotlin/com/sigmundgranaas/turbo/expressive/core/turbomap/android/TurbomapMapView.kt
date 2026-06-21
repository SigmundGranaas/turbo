package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import android.os.Handler
import android.os.HandlerThread
import android.os.Looper
import android.view.Choreographer
import android.view.SurfaceHolder
import android.view.SurfaceView
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
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
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
import android.os.SystemClock
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancelChildren
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import okhttp3.Call
import okhttp3.Callback
import okhttp3.Dispatcher
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import org.json.JSONArray
import java.io.File
import java.io.IOException
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.resume
import kotlin.math.abs

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
/** Tilt eased in when 3D mode is entered — reads as 3D + gives clouds their rake. */
// Entering 3D tilts steeply by default so the (exaggerated) relief reads
// immediately. The engine allows up to 80°; 62° gives a dramatic, gamey
// vantage while keeping the near ground legible.
private const val DEFAULT_3D_PITCH_DEG = 62.0

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
    // When true the map is in 3D mode: a 1-finger drag orbits about the user
    // location (or screen centre if it's off-screen) and two fingers pan + zoom.
    // Default false → unchanged 2D pan/zoom. See the 2D/3D mode design doc.
    threeDMode: Boolean = false,
    // DEM tile URL template (Mapbox-Terrain-RGB, `{z}/{x}/{y}`). When non-null the
    // ground displaces by real elevation (3D terrain). Pass it only in 3D mode.
    demUrl: String? = null,
    onMapLongClick: (LatLng) -> Unit = {},
    onMapTap: ((LatLng) -> Unit)? = null,
    onBearingChange: (Double) -> Unit = {},
    onMapReady: (MapEngine) -> Unit = {},
    onEngineError: (String) -> Unit = {},
) {
    val context = LocalContext.current
    val cameraTick = remember { mutableIntStateOf(0) }
    val controller = remember { TurbomapSurfaceController() }
    controller.cameraTick = cameraTick
    controller.onBearingChange = onBearingChange
    controller.onError = onEngineError
    controller.cacheDir = remember(context) { File(context.cacheDir, "turbomap-tiles") }
    fun scene() = TurbomapScene.build(rasters, track, route, measure, userLocation, demUrl = demUrl)

    // Latest 3D flag read by the long-lived gesture lambda (pointerInput(Unit) never
    // restarts), so toggling 3D takes effect without recreating the detector.
    val threeDState = rememberUpdatedState(threeDMode)

    // 2D↔3D transition: ease into a default tilt on entering 3D (so it reads as
    // 3D immediately + the cloud overlay gets its camera-ray side-reveal), back
    // to flat on leaving. The orbit gesture takes over from there.
    LaunchedEffect(threeDMode) {
        controller.easePitch(if (threeDMode) DEFAULT_3D_PITCH_DEG else 0.0)
    }

    Box(modifier.fillMaxSize()) {
        AndroidView(
            factory = { ctx ->
                SurfaceView(ctx).apply {
                    holder.addCallback(object : SurfaceHolder.Callback {
                        override fun surfaceCreated(holder: SurfaceHolder) = Unit
                        override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                            controller.attachOrResize(holder.surface, width, height, initialCamera, initialZoom, scene(), rasters, demUrl, onMapReady)
                        }
                        override fun surfaceDestroyed(holder: SurfaceHolder) = controller.detach()
                    })
                }
            },
            modifier = Modifier.fillMaxSize(),
        )
        // Pan/zoom + momentum + tap/long-press → camera + map events.
        Box(
            Modifier.fillMaxSize()
                .pointerInput(Unit) {
                    detectMapGestures(
                        onDown = { controller.onGestureDown() },
                        onTransform = { panX, panY, zoom, fx, fy -> controller.onTransform(panX, panY, zoom, fx, fy) },
                        onFling = { vx, vy -> controller.onFling(vx, vy) },
                        onZoomFling = { zv, fx, fy -> controller.onZoomFling(zv, fx, fy) },
                        mode = {
                            if (threeDState.value) MapGestureMode.ThreeD else MapGestureMode.TwoD
                        },
                        onOrbit = { db, dp, fx, fy -> controller.onOrbit(db, dp, fx, fy) },
                    )
                }
                .pointerInput(onMapTap, onMapLongClick) {
                    detectTapAndLongPress(
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

    LaunchedEffect(rasters, track, route, measure, userLocation, demUrl) {
        controller.applyScene(scene(), rasters, demUrl)
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
    var onError: (String) -> Unit = {}
    var cacheDir: File? = null

    private val tileCache: TurbomapTileCache? by lazy { cacheDir?.let { TurbomapTileCache(it) } }

    // Mutated on the main thread (attach/detach), read on the render thread.
    @Volatile private var handle = 0L
    private var width = 0
    private var height = 0
    private var rasters: List<TurbomapScene.RasterSpec> = emptyList()
    /** DEM tile URL template for 3D terrain ("terrain" pending tiles fetch this); null = flat. */
    private var demUrl: String? = null
    private val scope = CoroutineScope(Dispatchers.Main.immediate)
    private var lastBearing = Double.NaN

    // ── Tile reconciler ────────────────────────────────────────────────────
    // The engine's `pending_tiles` (desired-minus-present, nearest-first) is the
    // single source of truth. A loop continuously drives the host toward it
    // rather than firing once per gesture: each pass starts fetches for desired
    // tiles, CANCELS fetches for tiles no longer desired (so a fast pan can't
    // starve the current viewport behind stale work), and — because a missing
    // tile simply reappears in `desired` next pass — retries failures for free.
    // This is what makes loading consistent and self-healing after panning.
    private val inFlight = HashMap<String, Job>()
    private val retryAt = HashMap<String, Long>()
    private val wake = Channel<Unit>(Channel.CONFLATED)
    private var reconcileLoop: Job? = null
    private var lastStatsLogMs = 0L
    private var lastAnimReconcileMs = 0L
    private var lastPendingCount = 0
    private val http = OkHttpClient.Builder()
        .connectTimeout(TIMEOUT_MS.toLong(), TimeUnit.MILLISECONDS)
        .readTimeout(TIMEOUT_MS.toLong(), TimeUnit.MILLISECONDS)
        // All tiles share one host; OkHttp caps at 5/host by default, which would
        // throttle below our own concurrency cap. Align them so our reconciler is
        // the only limiter.
        .dispatcher(
            Dispatcher().apply {
                maxRequests = MAX_CONCURRENT_FETCHES
                maxRequestsPerHost = MAX_CONCURRENT_FETCHES
            },
        )
        .build()

    // ── Render thread ──────────────────────────────────────────────────────
    // Rendering (GPU encode/submit/present, which can block on vsync) runs on a
    // dedicated thread so it never stalls the UI thread. The native engine is
    // serialised by a Mutex inside the FFI, so gestures / projection / the tile
    // reconciler stay on the main thread and just contend briefly for the lock.
    // UI-touching results (overlay tick, bearing) are marshalled back to main.
    @Volatile private var running = false
    private val mainHandler = Handler(Looper.getMainLooper())
    private var renderThread: HandlerThread? = null
    private var renderHandler: Handler? = null
    private var choreographer: Choreographer? = null // this render thread's, set on it
    // Render-on-demand: only draw when something changed (camera/scene/tiles),
    // and only reproject the overlays when the camera actually moved. The vsync
    // loop stays armed (a cheap flag check) so a dirty flag is always picked up
    // next frame — no parking that could freeze the map.
    private val pendingRender = AtomicBoolean(true)
    private val pendingCamera = AtomicBoolean(true)
    private val frame = object : Choreographer.FrameCallback {
        override fun doFrame(frameTimeNanos: Long) {
            if (!running || handle == 0L) return
            val needsRender = pendingRender.getAndSet(false)
            var cameraMoved = pendingCamera.getAndSet(false)
            if (needsRender) {
                // nativeRender ticks any in-flight camera animation (fling/ease) first,
                // so the camera advances under our feet — treat an animating frame as a
                // camera move so overlays follow and tiles load along the trajectory.
                NativeSurfaceMap.nativeRender(handle)
                val animating = NativeSurfaceMap.nativeIsAnimating(handle)
                if (animating) cameraMoved = true
                if (cameraMoved) {
                    // Camera changed → reproject overlays (cameraTick, read by the
                    // overlays' `offset {}` on main) + report bearing.
                    mainHandler.post { cameraTick?.let { it.intValue++ } }
                    val cam = NativeSurfaceMap.nativeCamera(handle)
                    val b = if (cam.size >= 4) cam[3] else 0.0
                    if (lastBearing.isNaN() || abs(b - lastBearing) > BEARING_EPSILON) {
                        lastBearing = b
                        mainHandler.post { onBearingChange(b) }
                    }
                }
                if (animating) {
                    // Keep drawing until the fling/ease/fade settles (render-on-demand
                    // resumes parking afterward), and pull tiles along the path — but
                    // throttled, not 60×/s.
                    pendingRender.set(true)
                    val now = SystemClock.uptimeMillis()
                    if (now - lastAnimReconcileMs > ANIM_RECONCILE_MS) {
                        lastAnimReconcileMs = now
                        requestReconcile()
                    }
                }
            }
            choreographer?.postFrameCallback(this)
        }
    }

    /** Mark the next vsync dirty so the render thread draws it (render-on-demand). */
    fun requestRender(cameraMoved: Boolean) {
        pendingRender.set(true)
        if (cameraMoved) pendingCamera.set(true)
    }

    fun attachOrResize(
        surface: android.view.Surface,
        w: Int,
        h: Int,
        camera: LatLng,
        zoom: Double,
        sceneJson: String,
        rasterSpecs: List<TurbomapScene.RasterSpec>,
        demUrlTemplate: String?,
        onMapReady: (MapEngine) -> Unit,
    ) {
        width = w
        height = h
        rasters = rasterSpecs
        demUrl = demUrlTemplate
        if (handle == 0L) {
            handle = NativeSurfaceMap.nativeCreate(surface, w, h, camera.lat, camera.lng, zoom)
            if (handle == 0L) {
                // No fallback by design: report the GPU/surface failure loudly.
                onError(NativeSurfaceMap.nativeLastError() ?: "wgpu surface init failed")
                return
            }
            NativeSurfaceMap.nativeApplyScene(handle, sceneJson)
            // Light the scene by the real clock so terrain shading + the
            // sky take on the current time-of-day colours.
            NativeSurfaceMap.nativeSetSunTime(handle, System.currentTimeMillis() / 1000.0)
            // Terrain CAST shadows are owned by "sun mode" (the time-of-day
            // slider) — off by default, enabled when the user turns sun mode on.
            // See SunOverlayControls / TerrainSunOverlay.
            NativeSurfaceMap.nativePumpLocal(handle)
            val eng = TurbomapMapEngine(handle, w, h)
            // Camera/inset changes from the rail/flyTo/sheet must redraw (render-on-demand).
            eng.onMutated = { requestRender(cameraMoved = true) }
            engine = eng
            onMapReady(eng)
            startRenderLoop()
            startReconcileLoop()
            requestReconcile()
        } else {
            engine?.onResized(w, h)
            NativeSurfaceMap.nativeResize(handle, w, h)
            requestRender(cameraMoved = true)
            requestReconcile()
        }
    }

    fun applyScene(sceneJson: String, rasterSpecs: List<TurbomapScene.RasterSpec>, demUrlTemplate: String?) {
        if (handle == 0L) return
        rasters = rasterSpecs
        demUrl = demUrlTemplate
        NativeSurfaceMap.nativeApplyScene(handle, sceneJson)
        NativeSurfaceMap.nativePumpLocal(handle)
        requestRender(cameraMoved = false)
        requestReconcile()
    }

    // ── Weather-cloud overlay ───────────────────────────────────────────────
    // Thin forwarders to the native overlay. The engine is serialised by a
    // Mutex inside the FFI, so these are safe to call from the main thread
    // while the render thread draws — they just contend briefly for the lock,
    // like the tile reconciler.

    /** Enable the cloud overlay with a [gridW]×[gridH] radar grid. */
    fun enableClouds(gridW: Int, gridH: Int) {
        if (handle == 0L) return
        NativeSurfaceMap.nativeEnableClouds(handle, gridW, gridH)
        requestRender(cameraMoved = false)
    }

    /** Hide/show the overlay without discarding uploaded frames. */
    fun setCloudsVisible(visible: Boolean) {
        if (handle == 0L) return
        NativeSurfaceMap.nativeSetCloudsVisible(handle, visible)
        requestRender(cameraMoved = false)
    }

    /** Geo-register the radar to its lat/lng box → world-locked overlay. */
    fun setCloudGeoBounds(west: Double, south: Double, east: Double, north: Double) {
        if (handle == 0L) return
        NativeSurfaceMap.nativeSetCloudGeoBounds(handle, west, south, east, north)
        requestRender(cameraMoved = false)
    }

    /**
     * Upload a radar frame into [slot] (0 = current timestep, 1 = next) from
     * two [gridW]×[gridH] byte planes — [precip] and [coverage], each 0..255.
     */
    fun ingestRadarFrame(slot: Int, gridW: Int, gridH: Int, precip: ByteArray, coverage: ByteArray) {
        if (handle == 0L) return
        NativeSurfaceMap.nativeIngestRadarFrame(handle, slot, gridW, gridH, precip, coverage)
        requestRender(cameraMoved = false)
    }

    /**
     * Set the cloud animation clock ([time], seconds) and the slot-0→slot-1
     * crossfade ([blend], 0..1) — what the time slider scrubs. Redraws.
     */
    fun setCloudTime(time: Float, blend: Float) {
        if (handle == 0L) return
        NativeSurfaceMap.nativeSetCloudTime(handle, time, blend)
        requestRender(cameraMoved = false)
    }

    fun onTransform(panX: Float, panY: Float, zoomFactor: Float, focusX: Float, focusY: Float) {
        if (handle == 0L) return
        // Pan: recenter so the world follows the finger translation (the centroid delta).
        if (panX != 0f || panY != 0f) {
            val unp = NativeSurfaceMap.nativeUnproject(handle, width / 2.0 - panX, height / 2.0 - panY)
            if (unp.size >= 3 && unp[2] == 1.0) {
                val cam = NativeSurfaceMap.nativeCamera(handle)
                if (cam.size >= 4) NativeSurfaceMap.nativeSetCamera(handle, unp[0], unp[1], cam[2], cam[3])
            }
        }
        // Zoom about the pinch FOCUS — the world point under the fingers stays put, instead of
        // the map zooming toward the screen centre. The engine clamps the zoom into range.
        if (zoomFactor != 1f) {
            NativeSurfaceMap.nativeZoomAround(handle, zoomFactor.toDouble(), focusX.toDouble(), focusY.toDouble())
        }
        requestRender(cameraMoved = true)
        requestReconcile()
    }

    /**
     * Two-finger rotate + tilt step: spin the bearing by [dBearingDeg] and (3D) tilt the
     * pitch by [dPitchDeg], both about the gesture centroid ([focusX],[focusY]) so that
     * pixel stays over its world point. Either delta may be zero (rotate-only / tilt-only).
     */
    fun onOrbit(dBearingDeg: Float, dPitchDeg: Float, focusX: Float, focusY: Float) {
        if (handle == 0L) return
        NativeSurfaceMap.nativeOrbitAround(
            handle,
            dBearingDeg.toDouble(),
            dPitchDeg.toDouble(),
            focusX.toDouble(),
            focusY.toDouble(),
        )
        requestRender(cameraMoved = true)
        requestReconcile()
    }

    /** Ease the tilt to [pitchDeg] over [durationMs] — the 2D↔3D transition. */
    fun easePitch(pitchDeg: Double, durationMs: Int = 350) {
        if (handle == 0L) return
        NativeSurfaceMap.nativeEasePitch(handle, pitchDeg, durationMs)
        requestRender(cameraMoved = true)
    }

    /** Finger down: catch any in-flight fling/ease so the map stops where it is. */
    fun onGestureDown() {
        if (handle != 0L) NativeSurfaceMap.nativeCancelAnimation(handle)
    }

    /** Drag release: throw the map with the centroid velocity (px/s); (0,0) = rest. */
    fun onFling(vx: Float, vy: Float) {
        if (handle == 0L) return
        NativeSurfaceMap.nativeFling(handle, vx.toDouble(), vy.toDouble())
        requestRender(cameraMoved = true) // kick the loop so it ticks the fling
    }

    /** Pinch release: coast the zoom about ([fx],[fy]) at [zv] levels/s (no pan drift). */
    fun onZoomFling(zv: Float, fx: Float, fy: Float) {
        if (handle == 0L) return
        NativeSurfaceMap.nativeZoomFling(handle, zv.toDouble(), fx.toDouble(), fy.toDouble())
        requestRender(cameraMoved = true) // kick the loop so it ticks the zoom fling
    }

    /** Geographic point under a screen pixel, or null before the map is ready. */
    fun unproject(xPx: Float, yPx: Float): LatLng? {
        if (handle == 0L) return null
        val r = NativeSurfaceMap.nativeUnproject(handle, xPx.toDouble(), yPx.toDouble())
        return if (r.size >= 3 && r[2] == 1.0) LatLng(r[0], r[1]) else null
    }

    /** Coalesced nudge — ask the reconcile loop to run a pass as soon as it can. */
    private fun requestReconcile() {
        wake.trySend(Unit)
    }

    /**
     * The reconcile loop: continuously drive the host's loaded tiles toward the
     * engine's desired set. Wakes on [requestReconcile] (camera/scene change, a
     * fetch finishing) or a periodic safety tick (catches retries + any missed
     * edge), and exits once the surface is gone. Replaces the old edge-triggered,
     * fire-and-forget pump that froze after panning.
     */
    private fun startReconcileLoop() {
        if (reconcileLoop != null) return
        reconcileLoop = scope.launch {
            while (isActive && handle != 0L) {
                reconcile()
                logStatsThrottled()
                withTimeoutOrNull(SAFETY_TICK_MS) { wake.receive() }
            }
        }
    }

    /** While tiles are outstanding, log pending/in-flight + cache telemetry (throttled). */
    private fun logStatsThrottled() {
        if (handle == 0L || (inFlight.isEmpty() && lastPendingCount == 0)) return
        val now = SystemClock.uptimeMillis()
        if (now - lastStatsLogMs < STATS_LOG_INTERVAL_MS) return
        lastStatsLogMs = now
        android.util.Log.d(
            "TurbomapTiles",
            "pending=$lastPendingCount inflight=${inFlight.size} backoff=${retryAt.size} stats=${NativeSurfaceMap.nativeStats(handle)}",
        )
    }

    private fun reconcile() {
        if (handle == 0L) return
        val arr = runCatching { JSONArray(NativeSurfaceMap.nativePendingTilesJson(handle)) }.getOrNull() ?: return
        val desired = (0 until arr.length()).mapNotNull { parsePendingRaster(arr.optJSONObject(it)) }
        lastPendingCount = desired.size
        val byKey = desired.associateBy { it.key }
        val decision = planReconcile(
            desiredOrdered = desired.map { it.key },
            inFlight = inFlight.keys,
            retryAt = retryAt,
            now = SystemClock.uptimeMillis(),
            cap = MAX_CONCURRENT_FETCHES,
        )
        decision.toCancel.forEach { key -> inFlight.remove(key)?.cancel() }
        retryAt.keys.retainAll(byKey.keys)
        decision.toStart.forEach { key -> byKey[key]?.let { inFlight[key] = launchTileFetch(it) } }
    }

    private data class TileFetch(
        val layer: String,
        val z: Int,
        val x: Int,
        val y: Int,
        val key: String,
        val url: String,
        val terrain: Boolean = false,
    )

    /** Turn one pending-tiles entry into a fetchable raster/DEM request, or null.
     *  "terrain" entries use the DEM URL (3D heightmap) and ingest via the DEM
     *  path; "raster" entries use their layer's template. */
    private fun parsePendingRaster(o: org.json.JSONObject?): TileFetch? {
        if (o == null) return null
        val kind = o.optString("kind")
        val layer = o.optString("layer")
        val template = when (kind) {
            "raster" -> rasters.firstOrNull { it.id == layer }?.tileUrlTemplate
            "terrain" -> demUrl
            else -> null
        } ?: return null
        val z = o.optInt("z")
        val x = o.optInt("x")
        val y = o.optInt("y")
        val url = template.replace("{z}", "$z").replace("{x}", "$x").replace("{y}", "$y")
        return TileFetch(layer, z, x, y, "$layer/$z/$x/$y", url, terrain = kind == "terrain")
    }

    private fun launchTileFetch(t: TileFetch): Job {
        // LAZY so `self` is assigned BEFORE the body can run. With an immediate
        // dispatcher on a scope that's being cancelled (activity destroy / detach
        // racing a reconcile pass), an eagerly-started body jumps straight to the
        // `finally` below — reaching `=== self` before `self =` completes —
        // throwing UninitializedPropertyAccessException ("froze/crashed on
        // close"). LAZY + explicit start() makes that impossible.
        lateinit var self: Job
        self = scope.launch(start = CoroutineStart.LAZY) {
            try {
            // Read-through disk cache: serve an already-fetched tile offline, else
            // fetch (cancelable OkHttp Call, pooled/HTTP2) + persist.
            val cached = withContext(Dispatchers.IO) { tileCache?.get(t.layer, t.z, t.x, t.y) }
            val bytes = cached ?: fetchBytes(t.url)?.also { fetched ->
                withContext(Dispatchers.IO) { tileCache?.put(t.layer, t.z, t.x, t.y, fetched) }
            }
            when {
                bytes == null -> {
                    // Genuine miss (HTTP error/timeout): back off, then the next
                    // reconcile pass retries it for free (it's still desired).
                    retryAt[t.key] = SystemClock.uptimeMillis() + RETRY_BACKOFF_MS
                }
                handle == 0L -> Unit
                (if (t.terrain) {
                    NativeSurfaceMap.nativeIngestTerrain(handle, t.z, t.x, t.y, bytes)
                } else {
                    NativeSurfaceMap.nativeIngestRaster(handle, t.layer, t.z, t.x, t.y, bytes)
                }) -> {
                    requestRender(cameraMoved = false) // new tile → redraw, overlays unchanged
                }
                else -> {
                    // Bytes didn't decode (a 200-with-error-body / corrupt tile, e.g. a
                    // throttled upstream). Drop the poisoned cache entry and back off so
                    // the next pass re-fetches from the network instead of looping on it
                    // forever — the cause of grey gaps accumulating over a session.
                    withContext(Dispatchers.IO) { tileCache?.remove(t.layer, t.z, t.x, t.y) }
                    retryAt[t.key] = SystemClock.uptimeMillis() + RETRY_BACKOFF_MS
                    android.util.Log.w("TurbomapTiles", "tile ${t.key} did not decode (${bytes.size}B); evicted + backing off")
                }
            }
            } finally {
                // Only clear the slot if it's still *ours*. Under rapid zoom a key can be
                // cancelled and a fresh fetch started for it before this one finalises;
                // removing by key alone would drop that new job from the map (orphaning it
                // — still running, untracked) and, repeated over zoom cycles, those orphans
                // flood the HTTP pool until real tiles starve. That was the persistent
                // checkerboard stall.
                if (inFlight[t.key] === self) inFlight.remove(t.key)
                requestReconcile() // a slot freed — fill it with the next-nearest tile
            }
        }
        self.start()
        return self
    }

    fun detach() {
        running = false
        reconcileLoop?.cancel()
        reconcileLoop = null
        // Snapshot before cancelling: on the immediate dispatcher each cancel()
        // runs the job's `finally` synchronously, which removes the job from
        // `inFlight` — mutating the map mid-iteration would throw
        // ConcurrentModificationException (an "Unable to destroy activity" crash
        // on close/rotate). Iterate a copy, then clear any stragglers.
        inFlight.values.toList().forEach { it.cancel() }
        inFlight.clear()
        retryAt.clear()
        scope.coroutineContext.cancelChildren()
        val h = handle
        handle = 0L
        engine = null
        val thread = renderThread
        val handler = renderHandler
        renderThread = null
        renderHandler = null
        if (thread != null && handler != null) {
            // Stop the loop + free the native map ON the render thread, after any
            // in-flight frame drains (serial queue) — so destroy never races a
            // render. Then block-join briefly so surfaceDestroyed doesn't return
            // (and Android tear down the ANativeWindow) while we're still using it.
            handler.post {
                choreographer?.removeFrameCallback(frame)
                choreographer = null
                if (h != 0L) NativeSurfaceMap.nativeDestroy(h)
            }
            thread.quitSafely()
            runCatching { thread.join(DETACH_JOIN_MS) }
        } else if (h != 0L) {
            NativeSurfaceMap.nativeDestroy(h)
        }
    }

    private fun startRenderLoop() {
        if (running) return
        running = true
        val thread = HandlerThread("turbomap-render").apply { start() }
        val handler = Handler(thread.looper)
        renderThread = thread
        renderHandler = handler
        // Grab THIS thread's Choreographer (vsync) and drive the frame loop on it.
        handler.post {
            val c = Choreographer.getInstance()
            choreographer = c
            c.postFrameCallback(frame)
        }
    }

    /** Fetch a tile over the pooled OkHttp client; cancels the live HTTP call if the coroutine is cancelled. */
    private suspend fun fetchBytes(url: String): ByteArray? = suspendCancellableCoroutine { cont ->
        val call = http.newCall(Request.Builder().url(url).header("User-Agent", "turbo-android-wgpu").build())
        cont.invokeOnCancellation { call.cancel() }
        call.enqueue(object : Callback {
            override fun onFailure(call: Call, e: IOException) {
                if (cont.isActive) cont.resume(null)
            }

            override fun onResponse(call: Call, response: Response) {
                // CRITICAL: reading the body can throw (e.g. StreamResetException when
                // the HTTP/2 stream is reset mid-read under the concurrent-fetch burst).
                // If we let that escape, the continuation is never resumed → this worker
                // hangs forever and its concurrency slot leaks; after a few, all slots
                // are dead and tile loading silently stalls. Always resume.
                val bytes = runCatching {
                    response.use { r -> if (r.isSuccessful) r.body?.bytes() else null }
                }.getOrNull()
                if (cont.isActive) cont.resume(bytes)
            }
        })
    }

    private companion object {
        const val TIMEOUT_MS = 10_000
        const val BEARING_EPSILON = 0.01
        const val MAX_CONCURRENT_FETCHES = 8
        // Safety re-pump cadence: catches retries + any edge a wake missed, and
        // keeps loading self-healing even with no gestures. Cheap (one JNI call
        // returning "[]" when idle).
        const val SAFETY_TICK_MS = 350L
        const val RETRY_BACKOFF_MS = 1500L
        // Bounded wait for the render thread to release the surface on detach.
        const val DETACH_JOIN_MS = 350L
        const val STATS_LOG_INTERVAL_MS = 3000L
        // During an animation, pull tiles along the path at ~8 Hz, not every frame.
        const val ANIM_RECONCILE_MS = 120L
    }
}
