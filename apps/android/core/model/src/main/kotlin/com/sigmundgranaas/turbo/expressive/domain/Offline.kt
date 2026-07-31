package com.sigmundgranaas.turbo.expressive.domain

/** A lat/lng bounding box (WGS84). */
data class GeoBounds(
    val south: Double,
    val west: Double,
    val north: Double,
    val east: Double,
)

/** Lifecycle of an offline region as the user sees it. */
enum class OfflineStatus {
    /** Tiles are actively downloading (or queued). */
    Downloading,

    /** All required tiles are present. */
    Complete,

    /** Download halted on purpose (e.g. waiting for Wi-Fi) — resumable. */
    Paused,

    /** Download stopped on an error (network, server, tile-limit) — retryable. */
    Failed,
}

/**
 * A downloaded (or downloading) offline map region. Carries enough metadata to
 * render the region meaningfully — which base map and overlays it covers, where
 * it is, and when it was made — not just a name + size.
 */
data class OfflineRegionInfo(
    val id: Long,
    val name: String,
    val status: OfflineStatus,
    /** Download progress 0..1 (1 when complete). */
    val progress: Float,
    val sizeBytes: Long,
    val tileCount: Long = 0L,
    val base: BaseLayer = BaseLayer.Norgeskart,
    val overlays: Set<OverlayId> = emptySet(),
    /** The downloaded extent, when known (null for legacy regions). */
    val bounds: GeoBounds? = null,
    val minZoom: Double = 0.0,
    val maxZoom: Double = 0.0,
    val createdAtEpochMs: Long = 0L,
    /** Human-readable reason when [status] is [OfflineStatus.Failed]. */
    val errorReason: String? = null,
) {
    /** Back-compat convenience for call sites that only care about completion. */
    val complete: Boolean get() = status == OfflineStatus.Complete
}

/**
 * Everything needed to start an offline download: the named area, which base map
 * + overlays to cache, and the zoom span. Replaces the old positional
 * `download(name, base, bounds, min, max)` so overlays + naming travel together.
 */
data class DownloadSpec(
    val name: String,
    val base: BaseLayer,
    val bounds: GeoBounds,
    val minZoom: Double,
    val maxZoom: Double,
    val overlays: Set<OverlayId> = emptySet(),
    /**
     * Also download the routing pack for this region, so routes can be
     * planned here without signal.
     *
     * Default on. The pack is a fraction of the tiles it rides with —
     * measured at 2.6 MB against roughly 6 MB of raster for the same
     * 13 x 13 km — and a checkbox would ask the user to trade that for a
     * capability they cannot evaluate in the abstract. One region, one
     * download, one size.
     */
    val includeRouting: Boolean = true,
)

/**
 * The detail-vs-size trade-off for an offline download: how many zoom levels
 * past the current camera to cache. Standard matches the historical default.
 */
enum class DetailLevel(val zoomSpan: Double) {
    Standard(4.0),
    Detailed(6.0),
}

/** A pre-download estimate of how big a [DownloadSpec] will be. */
data class OfflineEstimate(
    val tiles: Long,
    /** Total download size: map tiles plus the routing pack. */
    val bytes: Long,
    /** The routing pack's share of [bytes]. 0 when routing is excluded. */
    val packBytes: Long = 0L,
    /** False when the area is too large to download (tile limit / span guard). */
    val withinLimits: Boolean = true,
)
