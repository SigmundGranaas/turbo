package com.sigmundgranaas.turbo.expressive.domain

/** How the app picks light vs dark colours. */
enum class ThemeMode { System, Light, Dark }

/**
 * Which router answers a route request.
 *
 * [Auto] is the shipped behaviour and the only one an ordinary user
 * should ever be on: server first, phone behind it. The two overrides
 * exist because "does on-device routing work on real hardware" cannot
 * be answered by a router that decides for itself which engine to use —
 * under [Auto] the device path only runs when the server is absent or
 * slow, so a tester on a good connection would never reach it, and a
 * tester on a bad one could not tell whether they had.
 *
 * They are deliberately settings and not build flags: the first
 * measurement has to happen on the published release APK, on a real
 * phone, over a real network. A debug build would answer a different
 * question — one where R8 has not run and the ABI split has not
 * happened.
 */
enum class RouteEngine {
    /** Server first, device behind it. The shipped default. */
    Auto,

    /** Always the phone. Fails rather than falling back, so a failure is visible. */
    Device,

    /** Always the server, even with a pack downloaded. The control case. */
    Server,
}

/** Persisted user preferences (DataStore-backed). */
data class UserSettings(
    val compassOrientation: Boolean = true,
    val followLocation: Boolean = false,
    val metricUnits: Boolean = true,
    val themeMode: ThemeMode = ThemeMode.System,
    /** When off, the cloud sync engine is paused even while signed in. */
    val cloudSyncEnabled: Boolean = true,
    /** When on, offline map downloads only run on un-metered (Wi-Fi) networks. */
    val downloadOverWifiOnly: Boolean = false,
    /** The last-selected base map, restored on launch. */
    val baseLayer: BaseLayer = BaseLayer.Norgeskart,
    /**
     * The map camera the user last left the app at, so reopening returns there
     * instead of the Norway-wide fallback. `null` until the map has been moved
     * at least once (first-ever launch).
     */
    val lastCameraLat: Double? = null,
    val lastCameraLng: Double? = null,
    val lastCameraZoom: Double? = null,
    /** My-position dot colour ("#RRGGBB"); null = the default blue. */
    val locationDotColorHex: String? = null,
    /** Whether the my-position dot grows a heading beam when the fix has a course. */
    val showHeadingBeam: Boolean = true,
    /** User-added XYZ basemaps ("add your own map URL"), in add order. */
    val customTileSources: List<CustomTileSource> = emptyList(),
    /** When set (and present in [customTileSources]), that custom source is the
     *  active basemap instead of [baseLayer]. Cleared by picking a built-in. */
    val selectedCustomSourceId: String? = null,
    /** Tunable map-gesture feel (Settings → Gestures). See the map overhaul spec. */
    val gestures: GestureSettings = GestureSettings(),
    /** Compass "Lock rotation": when on, gesture bearing changes are suppressed in both 2D and
     *  3D (pitch stays free). Toggled from the compass long-press menu or Settings → Gestures. */
    val rotationLocked: Boolean = false,
    /** Experimental features gated behind an explicit opt-in (off by default): the
     *  Trails and Clouds map layers only appear in the layers sheet when enabled. */
    val experimentalTrails: Boolean = false,
    val experimentalClouds: Boolean = false,
    /**
     * Which router answers. [RouteEngine.Auto] unless someone is
     * deliberately measuring — see [RouteEngine].
     */
    val routeEngine: RouteEngine = RouteEngine.Auto,
    /**
     * Where routing packs are downloaded from; `null` uses
     * [RoutingPack.DEFAULT_SOURCE].
     *
     * A setting for the same reason [routeEngine] is one. The pack is
     * the input to the measurement M1 exists to take, and the host that
     * cuts packs is the one piece of the stack that can be down for a
     * month at a time. Baking the host in at build time would mean the
     * published APK — the only build the measurement is valid on — could
     * not be pointed anywhere else without cutting a new release.
     */
    val packSourceUrl: String? = null,
)

/**
 * Renderer-agnostic mirror of the gesture tunables (the turbomap `GestureConfig`
 * lives in the Android-only module; this domain type is what persists + flows to
 * the UI). Defaults match the shipped feel. See the map overhaul spec, Phase 0.
 */
data class GestureSettings(
    val longPressMs: Long = 500L,
    val movementGuardDp: Float = 18f,
    val rotationGateDeg: Float = 10f,
    val flingHalfLifeMs: Long = 300L,
)
