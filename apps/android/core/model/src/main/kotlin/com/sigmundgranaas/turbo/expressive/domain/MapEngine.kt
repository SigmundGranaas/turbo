package com.sigmundgranaas.turbo.expressive.domain

/**
 * The renderer-agnostic map control seam the app talks to — *not* a renderer.
 *
 * Feature code (the map screen, the locate/zoom rail, the offline downloader,
 * the route/measure tools) holds a [MapEngine] and never names the renderer
 * behind it. Two implementations exist: `MapLibreEngine` (`:core:map`, over
 * MapLibre's `MapLibreMap`) and `TurbomapMapEngine` (`:core:turbomap-android`,
 * over the wgpu/Rust engine via JNI). It lives here, in the renderer-agnostic
 * `:core:model`, so both can implement it without either depending on the
 * other — which is what makes A/B and shadow-parity testing expressible.
 *
 * **Scope today = the control plane** the app actually uses: camera moves,
 * projection (screen↔geographic), the visible box, and the overlay inset.
 * These map onto the Rust `MapEngine` contract
 * (`apps/turbomap/.../turbomap-engine`): [fromScreen]/[toScreen] are its
 * `unproject`/`project`, [flyTo]/[frameTo] its camera animation, etc.
 *
 * The **data plane** of that contract (`applyScene`, `pendingTiles`/
 * `ingestTile`, `hitTest`, `capabilities`) and explicit surface lifecycle are
 * deliberately *not* here yet (see
 * `docs/architecture/2026-06-android-renderer-swap-test-plan.md`, Stage E).
 * Keeping this interface free of any renderer type is what lets that happen
 * without touching feature code.
 */
interface MapEngine {
    /** Animate one zoom level in. */
    fun zoomIn()

    /** Animate one zoom level out. */
    fun zoomOut()

    /** Animate the camera to [target] at [zoom]. */
    fun flyTo(target: LatLng, zoom: Double)

    /** The current camera centre — a sensible route origin when there's no GPS fix. */
    fun center(): LatLng

    /** Geographic position under a screen pixel — used to capture freehand drawing. */
    fun fromScreen(xPx: Float, yPx: Float): LatLng

    /** Screen pixel for a geographic position — anchors on-map UI (e.g. the long-press menu). */
    fun toScreen(point: LatLng): Pair<Float, Float>

    /** The currently visible lat/lng box — the area to download for offline use. */
    fun visibleBounds(): GeoBounds

    /**
     * Reserve [bottomPx] at the bottom of the map for an overlay (the live sheet),
     * so "centre on me" keeps the user dot in the *visible* band above it instead of
     * hidden behind the sheet. Subsequent camera moves honour this padding.
     */
    fun setBottomInset(bottomPx: Int)

    /** Current camera zoom level. */
    fun zoom(): Double

    /** Current map bearing in degrees (0 = north up). */
    fun bearing(): Double

    /** Animate the map back to north-up (compass reset). */
    fun resetNorth()

    /** Frame the camera to fit [points] (e.g. a saved track being opened on the map). */
    fun frameTo(points: List<LatLng>, paddingPx: Int = 140)
}
