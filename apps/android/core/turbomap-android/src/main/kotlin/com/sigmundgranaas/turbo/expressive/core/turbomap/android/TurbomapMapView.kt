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
@Composable
@Suppress("LongParameterList")
fun TurbomapMapView(
    rasters: List<TurbomapScene.RasterSpec>,
    initialCamera: LatLng,
    initialZoom: Double,
    modifier: Modifier = Modifier,
    /** Vector (MVT) overlays — the realistic-water layer is one. Fetched host-side. */
    vectors: List<TurbomapScene.VectorSpec> = emptyList(),
    track: List<LatLng>? = null,
    /** Tube colour for [track]; null = the default [TurbomapScene.TrackColor]. Lets a
     *  saved track render in its user-chosen colour (live recording stays default). */
    trackColor: TurbomapScene.Rgba? = null,
    route: List<LatLng>? = null,
    /** While following, the already-walked prefix of [route] — drawn as a dim tube
     *  behind the bright remaining-ahead [route] so progress reads at a glance (US-3). */
    routeCovered: List<LatLng>? = null,
    /** Follow-mode checkpoints (position → crossed): filled+checked when passed, else outlined. */
    checkpoints: List<Pair<LatLng, Boolean>> = emptyList(),
    measure: List<LatLng> = emptyList(),
    userLocation: LatLng? = null,
    /** Course over ground (deg, 0 = N) for the my-position heading beam; null = no heading. */
    userHeading: Float? = null,
    /** My-position dot colour; null = the default blue. A settings customization. */
    userDotColor: Color? = null,
    markers: List<Marker> = emptyList(),
    selectedMarkerId: String? = null,
    markerFallbackColor: Color = Color(0xFF8F4C38),
    photoPins: List<PhotoPin> = emptyList(),
    onPhotoPinClick: (PhotoPin) -> Unit = {},
    waypoints: List<LatLng> = emptyList(),
    /** Per-waypoint pin colour (parallel to [waypoints]) so on-map vias match their stop-list dots. */
    waypointColors: List<androidx.compose.ui.graphics.Color> = emptyList(),
    selectedWaypoint: Int? = null,
    onWaypointTap: (Int) -> Unit = {},
    onWaypointLongPress: (Int) -> Unit = {},
    onWaypointMoved: (Int, LatLng) -> Unit = { _, _ -> },
    onWaypointDragStart: (Int) -> Unit = {},
    onWaypointDragEnd: (Int) -> Unit = {},
    /** Pending route origin (first point before a destination exists) → drawn as an origin pin. */
    routeOrigin: LatLng? = null,
    onMarkerClick: (Marker) -> Unit = {},
    // When true, tilt/orbit gestures are unlocked (the 3D-terrain slider is > 0):
    // a 1-finger drag orbits about the user location and two fingers pan + zoom.
    // False → the flat 2D pan/zoom (pitch gestures rejected). The slider NEVER
    // forces a tilt itself — the camera only pitches when the user orbits.
    tiltEnabled: Boolean = false,
    // Compass "Lock rotation" (persisted): when true, gesture bearing changes are suppressed
    // in BOTH modes — the 2D twist never engages and the 3D orbit's horizontal (bearing)
    // component is pinned; pitch stays free. See MapGestureDetector.detectMapGestures.
    rotationLocked: Boolean = false,
    // Twist (deg from gesture start) that must lead pinch/pan to engage a 2D bearing rotate —
    // the Settings → Gestures "rotation strictness". Sampled once per gesture by the detector.
    rotationGateDeg: Float = GestureConfig.DEFAULT_ROTATION_GATE_DEG,
    // DEM tile URL template (Mapbox-Terrain-RGB, `{z}/{x}/{y}`). When non-null the
    // ground displaces by real elevation. Present when 3D OR 2D sun-lit relief is on.
    demUrl: String? = null,
    // Vertical exaggeration for the DEM mesh — the 3D slider's dialed value.
    demExaggeration: Float = com.sigmundgranaas.turbo.expressive.domain.DEFAULT_3D_DETENT,
    onMapLongClick: (LatLng) -> Unit = {},
    onMapTap: ((LatLng) -> Unit)? = null,
    onBearingChange: (Double) -> Unit = {},
    // Fired when the user manually moves the camera (pan / pinch / orbit / fling).
    // The host uses it to release camera-follow so it doesn't snap back to the dot.
    // Programmatic moves (flyTo/follow/easePitch) go through the controller, not the
    // gesture detector, so they never trigger this.
    onUserPanned: () -> Unit = {},
    onMapReady: (MapEngine) -> Unit = {},
    onEngineError: (String) -> Unit = {},
) {
    val context = LocalContext.current
    val cameraTick = remember { mutableIntStateOf(0) }
    val controller = remember { TurbomapSurfaceController() }
    controller.cameraTick = cameraTick
    controller.onBearingChange = onBearingChange
    controller.onError = onEngineError
    // Namespace is versioned: bump the suffix to abandon a poisoned cache. v2
    // drops entries written while the tileserver was still provisioning and
    // returned 200-with-0-bytes for vector basemap tiles — those empty MVTs
    // ingest as valid-but-empty (no water) and, being "resident", were never
    // refetched, so water never appeared even once the server had data. A
    // sideload upgrade keeps cacheDir, so the only way to shed them is a bump.
    controller.cacheDir = remember(context) { File(context.cacheDir, TURBOMAP_TILE_DIR) }
    // The live user position is NOT in the scene anymore — it's a Compose MyPositionPin in
    // the overlay (stands on the terrain via the engine projection). See TurbomapScene.

    // Latest tilt-enable flag read by the long-lived gesture lambda (pointerInput(Unit)
    // never restarts), so toggling 3D takes effect without recreating the detector.
    val tiltEnabledState = rememberUpdatedState(tiltEnabled)
    // Same: the gesture lambda is captured once, so read the latest callback / flags.
    val onUserPannedState = rememberUpdatedState(onUserPanned)
    val rotationLockedState = rememberUpdatedState(rotationLocked)
    val rotationGateDegState = rememberUpdatedState(rotationGateDeg)

    // Hard decoupling rule (Phase 1): the 3D slider must NEVER change the camera
    // pitch. Entering 3D only *unlocks* orbit gestures — it does not auto-tilt.
    // Leaving 3D (slider → 0) flattens the camera back to top-down so a 2D map is
    // never stuck at an angle. The sun slider touches pitch in neither direction.
    LaunchedEffect(tiltEnabled) {
        if (!tiltEnabled) controller.easePitch(0.0)
    }

    Box(modifier.fillMaxSize()) {
        AndroidView(
            factory = { ctx ->
                SurfaceView(ctx).apply {
                    holder.addCallback(object : SurfaceHolder.Callback {
                        override fun surfaceCreated(holder: SurfaceHolder) = Unit
                        override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                            controller.attachOrResize(holder.surface, width, height, initialCamera, initialZoom, rasters, vectors, measure, demUrl, onMapReady)
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
                        onTransform = { panX, panY, zoom, fx, fy ->
                            onUserPannedState.value()
                            controller.onTransform(panX, panY, zoom, fx, fy)
                        },
                        onFling = { vx, vy ->
                            // Only an actual fling (non-zero release velocity) is a manual
                            // camera move — report it so the host releases follow + dismisses
                            // the map-point card. EVERY tap and long-press also ends here with
                            // a (0,0) rest (the detector's no-fling path); reporting that as a
                            // pan would instantly dismiss the card the tap/long-press just
                            // opened — the "taps and long-presses do nothing" regression.
                            if (vx != 0f || vy != 0f) onUserPannedState.value()
                            controller.onFling(vx, vy)
                        },
                        onZoomFling = { zv, fx, fy -> controller.onZoomFling(zv, fx, fy) },
                        mode = {
                            if (tiltEnabledState.value) MapGestureMode.ThreeD else MapGestureMode.TwoD
                        },
                        onOrbit = { db, dp, fx, fy ->
                            onUserPannedState.value()
                            controller.onOrbit(db, dp, fx, fy)
                        },
                        rotationLocked = { rotationLockedState.value },
                        rotationGateDeg = { rotationGateDegState.value },
                    )
                }
                .pointerInput(onMapTap, onMapLongClick) {
                    detectTapAndLongPress(
                        // Ground (terrain-aware) unprojection: taps/long-presses PLACE things
                        // (waypoints, markers), so the point must round-trip through the
                        // terrain-lifted overlay projection back to the tapped pixel.
                        onTap = { o -> controller.unprojectGround(o.x, o.y)?.let { onMapTap?.invoke(it) } },
                        onLongPress = { o -> controller.unprojectGround(o.x, o.y)?.let(onMapLongClick) },
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
                userLocation = userLocation,
                userHeading = userHeading,
                userDotColor = userDotColor,
                onMarkerClick = onMarkerClick,
                photoPins = photoPins,
                onPhotoPinClick = onPhotoPinClick,
                waypoints = waypoints,
                waypointColors = waypointColors,
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

    LaunchedEffect(rasters, vectors, measure, demUrl, demExaggeration) {
        controller.applyContent(rasters, vectors, measure, demUrl, demExaggeration)
    }
    // Route + track render as raised 3D tubes — `tube` layers in the same
    // Scene document (plan P5.2), rebuilt whenever their geometry changes.
    LaunchedEffect(track, trackColor) {
        controller.setRouteTube("track", track, trackColor ?: TurbomapScene.TrackColor)
    }
    // The dim already-walked segment goes down first so the bright remaining-ahead
    // route tube renders over it where they meet at the progress cursor (US-3).
    LaunchedEffect(routeCovered) {
        controller.setRouteTube("route_covered", routeCovered, TurbomapScene.RouteCoveredColor)
    }
    LaunchedEffect(route) {
        controller.setRouteTube("route", route, TurbomapScene.RouteColor)
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

    private val tileCache: TileStore? by lazy { cacheDir?.let { TileStore(it) } }

    // Mutated on the main thread (attach/detach), read on the render thread.
    @Volatile private var handle = 0L
    private var width = 0
    private var height = 0
    private var rasters: List<TurbomapScene.RasterSpec> = emptyList()
    /** Vector (MVT) overlays — the realistic-water layer. "vector" pending tiles fetch these. */
    private var vectors: List<TurbomapScene.VectorSpec> = emptyList()
    /** DEM tile URL template for 3D terrain ("terrain" pending tiles fetch this); null = flat. */
    private var demUrl: String? = null
    /** Vertical exaggeration for the DEM mesh — the 3D-terrain slider's dialed value. */
    private var demExaggeration: Float = com.sigmundgranaas.turbo.expressive.domain.DEFAULT_3D_DETENT
    /** Measure polyline — flat geojson content in the scene. */
    private var measure: List<LatLng> = emptyList()

    // ── Scene state the old imperative side-doors carried (plan P5.2) ──────
    // Route/track tubes, the sun, terrain shadows, and the cloud overlay are
    // SCENE content now: each mutation rebuilds the one Scene document and
    // re-applies it (the engine diffs, so a tube/environment-only change is
    // cheap). Kept controller-side so a surface (re)create replays them.
    private val tubes = LinkedHashMap<String, TurbomapScene.TubeSpec>()
    private var environment = TurbomapScene.EnvironmentSpec()
    /** Radar grid size, set by [enableClouds]; null = no cloud overlay. */
    private var cloudGrid: Pair<Int, Int>? = null
    /** Radar geo box `[west, south, east, north]`; the overlay only enters the
     *  scene once geo-registered (the IR's field source is world-locked). */
    private var cloudBounds: DoubleArray? = null
    private var cloudsVisible = true
    private val scope = CoroutineScope(Dispatchers.Main.immediate)
    private var lastBearing = Double.NaN

    // ── Tile reconciler ────────────────────────────────────────────────────
    // The engine's streaming plan (desired-minus-present, nearest-first) is the
    // single source of truth. A loop continuously drives the host toward it
    // rather than firing once per gesture: each pass starts fetches for desired
    // tiles, CANCELS fetches for tiles no longer desired (so a fast pan can't
    // starve the current viewport behind stale work), and — because a missing
    // tile simply reappears in `desired` next pass — retries failures for free.
    // This is what makes loading consistent and self-healing after panning.
    private val inFlight = HashMap<String, Job>()
    private val retryAt = HashMap<String, Long>()

    /** Plan request-id → reconcile key, for honouring `cancel` entries. */
    private val fetchIds = HashMap<Long, String>()
    private val wake = Channel<Unit>(Channel.CONFLATED)
    private var reconcileLoop: Job? = null
    private var lastStatsLogMs = 0L
    private var lastAnimReconcileMs = 0L
    private var lastPendingCount = 0
    private var coldLoadTraceStart = 0L
    private var coldLoadTraceDone = false
    private var lastTracedFrame = ""
    private val http = OkHttpClient.Builder()
        .connectTimeout(TIMEOUT_MS.toLong(), TimeUnit.MILLISECONDS)
        .readTimeout(TIMEOUT_MS.toLong(), TimeUnit.MILLISECONDS)
        // Raster + DEM are different hosts; let OkHttp's caps be wide enough that
        // OUR per-kind lanes (RASTER_FETCH_LANE + DEM_FETCH_LANE) are the only
        // limiter. Global cap = both lanes; per-host cap = the wider (raster)
        // lane (the DEM host only ever sees DEM_FETCH_LANE). OkHttp's 5/host
        // default would otherwise throttle below our reconciler.
        .dispatcher(
            Dispatcher().apply {
                maxRequests = RASTER_FETCH_LANE + DEM_FETCH_LANE + VECTOR_FETCH_LANE
                maxRequestsPerHost = RASTER_FETCH_LANE
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
        rasterSpecs: List<TurbomapScene.RasterSpec>,
        vectorSpecs: List<TurbomapScene.VectorSpec>,
        measurePts: List<LatLng>,
        demUrlTemplate: String?,
        onMapReady: (MapEngine) -> Unit,
    ) {
        width = w
        height = h
        rasters = rasterSpecs
        vectors = vectorSpecs
        measure = measurePts
        demUrl = demUrlTemplate
        if (handle == 0L) {
            handle = NativeSurfaceMap.nativeCreate(surface, w, h, camera.lat, camera.lng, zoom)
            if (handle == 0L) {
                // No fallback by design: report the GPU/surface failure loudly.
                onError(NativeSurfaceMap.nativeLastError() ?: "wgpu surface init failed")
                return
            }
            // Light the scene by the real clock so terrain shading + the sky
            // take on the current time-of-day colours. Terrain CAST shadows
            // stay off until "sun mode" raises them (SunOverlayControls).
            environment = environment.copy(sunUnixSeconds = System.currentTimeMillis() / 1000.0)
            // One Scene document carries everything — content, tubes set
            // before the surface existed, and the environment (plan P5.2).
            NativeSurfaceMap.nativeApplyScene(handle, sceneJson())
            NativeSurfaceMap.nativePumpLocal(handle)
            val eng = TurbomapMapEngine(handle, w, h)
            // Camera/inset changes from the rail/flyTo/sheet must redraw (render-on-demand).
            eng.onMutated = { requestRender(cameraMoved = true) }
            // Sun/shadow/cloud mutations from features route back here and
            // become scene rebuilds — the engine has no imperative side-doors.
            eng.environmentHost = this
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

    /** The complete Scene document — content + tubes + environment — from the
     *  controller's current state. Consecutive applies are diffed engine-side,
     *  so a rebuild that only changes one tube or the sun is cheap. */
    private fun sceneJson() = TurbomapScene.build(
        rasters = rasters,
        vectors = vectors,
        measure = measure,
        demUrl = demUrl,
        demExaggeration = demExaggeration,
        tubes = tubes.values.toList(),
        environment = environment.copy(clouds = cloudsSpec()),
    )

    private fun cloudsSpec(): TurbomapScene.CloudsSpec? {
        val (gw, gh) = cloudGrid ?: return null
        val b = cloudBounds ?: return null
        return TurbomapScene.CloudsSpec(gw, gh, b[0], b[1], b[2], b[3], cloudsVisible)
    }

    /** Rebuild + re-apply the Scene after any content/environment mutation. */
    private fun reapplyScene() {
        if (handle == 0L) return
        NativeSurfaceMap.nativeApplyScene(handle, sceneJson())
        NativeSurfaceMap.nativePumpLocal(handle)
        requestRender(cameraMoved = false)
        requestReconcile()
    }

    /** Set (or clear, with < 2 points) a route/track polyline drawn as a
     *  raised 3D tube — a `tube` layer in the scene (plan P5.2). */
    fun setRouteTube(
        id: String,
        points: List<LatLng>?,
        color: TurbomapScene.Rgba,
        radiusPx: Double = ROUTE_TUBE_RADIUS_PX,
    ) {
        val pts = points.orEmpty()
        if (pts.size < 2) {
            tubes.remove(id)
        } else {
            tubes[id] = TurbomapScene.TubeSpec(id, pts, color, radiusPx)
        }
        reapplyScene()
    }

    fun applyContent(
        rasterSpecs: List<TurbomapScene.RasterSpec>,
        vectorSpecs: List<TurbomapScene.VectorSpec>,
        measurePts: List<LatLng>,
        demUrlTemplate: String?,
        demExaggerationValue: Float = demExaggeration,
    ) {
        if (handle == 0L) return
        rasters = rasterSpecs
        vectors = vectorSpecs
        measure = measurePts
        demUrl = demUrlTemplate
        demExaggeration = demExaggerationValue
        reapplyScene()
    }

    // ── Scene environment (plan P5.2) ───────────────────────────────────────
    // The sun, terrain shadows, and the cloud overlay's declaration are scene
    // state; each mutation rebuilds + re-applies the document. Only radar
    // frame DATA ([TurbomapMapEngine.ingestRadarFrame], transport like tiles)
    // and the playback clock (nativeSetCloudTime) stay imperative verbs.

    /** Track the sun (terrain shading + sky colour) to a real UTC instant;
     *  negative reverts to the engine's fixed default light (the
     *  [com.sigmundgranaas.turbo.expressive.domain.TerrainSunOverlay] contract). */
    fun setSunTime(unixSeconds: Double) {
        environment = environment.copy(sunUnixSeconds = unixSeconds.takeIf { it >= 0 })
        reapplyScene()
    }

    /** Terrain cast-shadow strength in `[0,1]`; 0 = off ("sun mode"). */
    fun setTerrainShadows(strength: Float) {
        environment = environment.copy(terrainShadows = strength)
        reapplyScene()
    }

    /** Declare the cloud overlay's radar grid. The overlay enters the scene
     *  once [setCloudGeoBounds] world-locks it (the IR has no screen-locked
     *  clouds — an unregistered field renders nothing, by design). */
    fun enableClouds(gridW: Int, gridH: Int) {
        cloudGrid = gridW to gridH
        reapplyScene()
    }

    /** Hide/show the overlay without discarding uploaded frames. */
    fun setCloudsVisible(visible: Boolean) {
        cloudsVisible = visible
        reapplyScene()
    }

    /** Geo-register the radar to its lat/lng box → world-locked overlay. */
    fun setCloudGeoBounds(west: Double, south: Double, east: Double, north: Double) {
        cloudBounds = doubleArrayOf(west, south, east, north)
        reapplyScene()
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
        // Pan: hand the raw finger delta to the engine. It recenters against the
        // LIVE camera on the render thread, so rapid sub-frame moves accumulate
        // instead of each recomputing from a stale snapshot and overwriting the
        // last (the dropped-motion "throttle"/jitter, worst in 3D). One wait-free
        // command replaces the old unproject → camera → setCamera round-trip.
        if (panX != 0f || panY != 0f) {
            NativeSurfaceMap.nativePanBy(handle, panX.toDouble(), panY.toDouble())
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

    /**
     * TERRAIN-aware ground point under a screen pixel (identical to [unproject]
     * in 2D / with no relief), or null before the map is ready. Taps place things
     * — waypoints, markers, route stops — and placement must invert the
     * terrain-LIFTED render projection, or the committed geo point re-projects to
     * a different pixel in 3D and the pin "lands somewhere else".
     */
    fun unprojectGround(xPx: Float, yPx: Float): LatLng? {
        if (handle == 0L) return null
        val r = NativeSurfaceMap.nativeUnprojectGround(handle, xPx.toDouble(), yPx.toDouble())
        return if (r.size >= 5 && r[4] == 1.0) LatLng(r[0], r[1]) else unproject(xPx, yPx)
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
        coldLoadTraceStart = SystemClock.uptimeMillis()
        reconcileLoop = scope.launch {
            while (isActive && handle != 0L) {
                reconcile()
                traceColdLoad()
                logStatsThrottled()
                withTimeoutOrNull(SAFETY_TICK_MS) { wake.receive() }
            }
        }
    }

    /**
     * Slice-1 cold-load trace. For the first [COLD_LOAD_TRACE_MS] after the
     * surface comes up, log one line per reconcile tick under the dedicated
     * `TurbomapTrace` tag: the engine's structured per-frame trace
     * ([NativeSurfaceMap.nativeStats], same field-set the offline harness writes
     * to `profile.csv`) PLUS the host-owned fetch-transport counts the engine
     * can't know — `fetching` (in-flight HTTP) and `backoff` (failed, retrying).
     * Together that's the full tile-state histogram over the cold-load window:
     * the first-load ORDERING + jitter the synchronous harness can't measure.
     *
     * Pull it with:
     *   adb logcat -c && adb logcat -s TurbomapTrace:I > coldload.log
     * (open the map fresh to trigger a cold load), then parse `t_ms=` + JSON.
     */
    private fun traceColdLoad() {
        if (coldLoadTraceDone || handle == 0L) return
        val t = SystemClock.uptimeMillis() - coldLoadTraceStart
        if (t > COLD_LOAD_TRACE_MS) {
            coldLoadTraceDone = true
            android.util.Log.i("TurbomapTrace", "cold-load trace complete (window ${COLD_LOAD_TRACE_MS}ms)")
            return
        }
        // The reconcile loop ticks far faster than the render thread publishes
        // frames (it wakes on every fetch completion), so dedupe by the engine's
        // frame index — one trace line per actually-rendered frame, not per tick.
        val stats = NativeSurfaceMap.nativeStats(handle)
        val frame = stats.substringAfter("\"frame\":", "").substringBefore(",")
        if (frame.isNotEmpty() && frame == lastTracedFrame) return
        lastTracedFrame = frame
        android.util.Log.i(
            "TurbomapTrace",
            "t_ms=$t fetching=${inFlight.size} backoff=${retryAt.size} $stats",
        )
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

    /**
     * Consume the engine's STREAMING PLAN (plan P5.1): the engine plans —
     * priority order, dedup against in-flight fetches, cancel of stale ones —
     * and this host only executes transport. The host still owns two things
     * the engine deliberately leaves above the table: per-kind fetch LANES
     * (the narrow MVT lane that keeps the 2-core tileserver inside its
     * timeout, DEM separate from raster churn) and failure BACKOFF. Starts
     * that a full lane or a backoff window declines are reported cancelled,
     * so the engine re-issues them on a later plan — nothing is lost.
     */
    private fun reconcile() {
        if (handle == 0L) return
        val now = SystemClock.uptimeMillis()
        retryAt.entries.removeAll { it.value <= now }
        val free = (RASTER_FETCH_LANE + DEM_FETCH_LANE + VECTOR_FETCH_LANE - inFlight.size)
            .coerceAtLeast(0)
        val plans = runCatching {
            JSONArray(NativeSurfaceMap.nativeTakeStreamingPlanJson(handle, free))
        }.getOrNull() ?: return
        var started = 0
        for (p in 0 until plans.length()) {
            val plan = plans.optJSONObject(p) ?: continue
            honourCancels(plan.optJSONArray("cancel"))
            started += dispatchStarts(plan.optJSONArray("start"), now)
        }
        lastPendingCount = started + inFlight.size
    }

    /** Honour a plan's `cancel` list: abort our fetch (if it is ours) and
     *  acknowledge every id so the engine's table settles. */
    private fun honourCancels(cancels: JSONArray?) {
        val list = cancels ?: return
        for (i in 0 until list.length()) {
            val id = list.optLong(i, -1L)
            if (id < 0) continue
            fetchIds.remove(id)?.let { key -> inFlight.remove(key)?.cancel() }
            NativeSurfaceMap.nativeReportFetchCancelled(handle, id)
        }
    }

    /** Spawn every admitted `start`; returns how many were launched. */
    private fun dispatchStarts(starts: JSONArray?, now: Long): Int {
        val list = starts ?: return 0
        var started = 0
        for (i in 0 until list.length()) {
            if (startFetchIfAdmitted(list.optJSONObject(i), now)) started++
        }
        return started
    }

    /** One plan `start`: parse, run the lane/backoff admission gate, and
     *  either launch the fetch or report it cancelled so the engine
     *  re-issues it when capacity frees up. */
    private fun startFetchIfAdmitted(obj: org.json.JSONObject?, now: Long): Boolean {
        val t = parsePlanStart(obj) ?: return false
        val lane = laneOf(t.key)
        val admitted = admitFetch(
            key = t.key,
            laneUsed = inFlight.keys.count { laneOf(it) == lane },
            laneCap = laneCap(lane),
            alreadyInFlight = inFlight.containsKey(t.key),
            retryAt = retryAt,
            now = now,
        )
        if (!admitted) {
            // Declined (lane full / backing off / duplicate) — hand it back.
            NativeSurfaceMap.nativeReportFetchCancelled(handle, t.id)
            return false
        }
        fetchIds[t.id] = t.key
        inFlight[t.key] = launchTileFetch(t)
        return true
    }

    private enum class FetchLane { RASTER, DEM, VECTOR }

    private fun laneOf(key: String): FetchLane = when {
        isDemKey(key) -> FetchLane.DEM
        isVectorKey(key) -> FetchLane.VECTOR
        else -> FetchLane.RASTER
    }

    // Vector (MVT) tiles keep their OWN small lane: the self-hosted basemap
    // MVT query is CPU-heavy (~0.2-1.5s/tile on the 2-core tileserver); in a
    // 32-wide shared lane they flooded the server past its timeout. DEM is
    // separate so raster churn can't starve the heightmap.
    private fun laneCap(lane: FetchLane): Int = when (lane) {
        FetchLane.RASTER -> RASTER_FETCH_LANE
        FetchLane.DEM -> DEM_FETCH_LANE
        FetchLane.VECTOR -> VECTOR_FETCH_LANE
    }

    /** A reconcile-key belongs to the DEM lane iff it's a terrain tile. Matches
     *  the `"__terrain"` layer name `parsePendingRaster` assigns DEM entries. */
    private fun isDemKey(key: String) = key.startsWith("__terrain/")

    /** A reconcile-key belongs to the vector lane iff its layer is one of the
     *  declared vector (MVT) layers — key is "<layer>/z/x/y" (see reconcile key). */
    private fun isVectorKey(key: String) = vectors.any { key.startsWith(it.id + "/") }

    private data class TileFetch(
        val id: Long,
        val layer: String,
        val z: Int,
        val x: Int,
        val y: Int,
        val key: String,
        val url: String,
        val terrain: Boolean = false,
        val vector: Boolean = false,
    )

    /** Turn one plan `start` entry into a fetchable request, or null.
     *  "terrain" entries use the DEM URL (3D heightmap) and ingest via the DEM
     *  path; "raster" entries use their layer's template. */
    private fun parsePlanStart(o: org.json.JSONObject?): TileFetch? {
        if (o == null) return null
        val id = o.optLong("id", -1L)
        if (id < 0) return null
        val kind = o.optString("kind")
        val layer = o.optString("layer")
        val template = when (kind) {
            "raster" -> rasters.firstOrNull { it.id == layer }?.tileUrlTemplate
            "vector" -> vectors.firstOrNull { it.id == layer }?.tileUrlTemplate
            "terrain" -> demUrl
            else -> null
        } ?: return null
        val z = o.optInt("z")
        val x = o.optInt("x")
        val y = o.optInt("y")
        val url = template.replace("{z}", "$z").replace("{x}", "$x").replace("{y}", "$y")
        return TileFetch(
            id, layer, z, x, y, "$layer/$z/$x/$y", url,
            terrain = kind == "terrain",
            vector = kind == "vector",
        )
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
                    // Genuine miss (HTTP error/timeout): report the attempt failed
                    // (the engine re-pends it) and back off locally so the next
                    // plans decline it until the window elapses.
                    retryAt[t.key] = SystemClock.uptimeMillis() + RETRY_BACKOFF_MS
                    if (handle != 0L) NativeSurfaceMap.nativeReportFetchFailed(handle, t.id)
                }
                handle == 0L -> Unit
                bytes.size > MAX_TILE_BYTES -> {
                    // Oversized payload (misrouted endpoint / error body / corrupt
                    // cache entry). Copying it into the native ingest queue OOM-aborts
                    // the process, so evict it and back off instead of ingesting.
                    withContext(Dispatchers.IO) { tileCache?.remove(t.layer, t.z, t.x, t.y) }
                    retryAt[t.key] = SystemClock.uptimeMillis() + RETRY_BACKOFF_MS
                    if (handle != 0L) NativeSurfaceMap.nativeReportFetchFailed(handle, t.id)
                    android.util.Log.w("TurbomapTiles", "tile ${t.key} oversize (${bytes.size}B); evicted + skipped")
                }
                (when {
                    t.terrain -> NativeSurfaceMap.nativeIngestTerrain(handle, t.z, t.x, t.y, bytes)
                    t.vector -> NativeSurfaceMap.nativeIngestVector(handle, t.layer, t.z, t.x, t.y, bytes)
                    else -> NativeSurfaceMap.nativeIngestRaster(handle, t.layer, t.z, t.x, t.y, bytes)
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
                    if (handle != 0L) NativeSurfaceMap.nativeReportFetchFailed(handle, t.id)
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
                fetchIds.remove(t.id)
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
        fetchIds.clear()
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
                    response.use { r ->
                        // Reject an over-cap body before `bytes()` reads the whole
                        // thing into memory (when the length is advertised; chunked
                        // responses fall through to the post-read guard at ingest).
                        val len = r.body?.contentLength() ?: -1L
                        when {
                            !r.isSuccessful -> null
                            len > MAX_TILE_BYTES -> null
                            else -> r.body?.bytes()
                        }
                    }
                }.getOrNull()
                if (cont.isActive) cont.resume(bytes)
            }
        })
    }

    private companion object {
        const val TIMEOUT_MS = 10_000
        const val BEARING_EPSILON = 0.01
        // Per-KIND fetch lanes. Raster (Kartverket CDN) and DEM (our tileserver)
        // are different hosts with opposite scaling, measured by tile_profiler:
        // the CDN scales cleanly to ~32; the DEM endpoint serves cached tiles
        // fast but renders cold ones CPU-bound (server caps concurrent renders,
        // 429-ing the excess). A single shared pool let raster churn starve DEM
        // (3D heightmap "never loaded"). Separate lanes fix that: each kind gets
        // its own budget and can't crowd out the other. DEM lane is small so a
        // cold region fills steadily (4-wide server renders) instead of a 429
        // storm; raster lane is wide so the basemap streams in fast.
        const val RASTER_FETCH_LANE = 32
        const val DEM_FETCH_LANE = 8

        /** Concurrent vector (MVT) tile fetches. Small on purpose: the self-hosted
         *  basemap MVT render is CPU-heavy on a 2-core tileserver with no tile
         *  cache, so flooding it (32-wide) timed every tile out. ~6 keeps renders
         *  within the 10s fetch timeout while water streams in progressively. */
        const val VECTOR_FETCH_LANE = 6
        // Safety re-pump cadence: catches retries + any edge a wake missed, and
        // keeps loading self-healing even with no gestures. Cheap (one JNI call
        // returning "[]" when idle).
        const val SAFETY_TICK_MS = 350L
        const val RETRY_BACKOFF_MS = 1500L
        /** Slice-1 cold-load trace window: log the structured per-frame trace
         *  under `TurbomapTrace` for this long after a fresh surface comes up,
         *  to capture first-load streaming order + jitter. */
        const val COLD_LOAD_TRACE_MS = 15_000L
        // Hard cap on a single tile payload. A 256–512px encoded raster/DEM tile
        // is well under 1 MB; anything past this is a misrouted endpoint, an
        // error page, or a corrupt cache entry. Reject it instead of copying it
        // into the native ingest queue — an unbounded payload there OOM-aborts
        // the process in scudo (`internal map failure`) on the fetch coroutine.
        const val MAX_TILE_BYTES = 16 * 1024 * 1024
        // Route/track tube radius in screen px (≈ a 16 px-wide path). Tunable.
        const val ROUTE_TUBE_RADIUS_PX = 8.0
        // Bounded wait for the render thread to release the surface on detach.
        const val DETACH_JOIN_MS = 350L
        const val STATS_LOG_INTERVAL_MS = 3000L
        // During an animation, pull tiles along the path at ~8 Hz, not every frame.
        const val ANIM_RECONCILE_MS = 120L
    }
}
