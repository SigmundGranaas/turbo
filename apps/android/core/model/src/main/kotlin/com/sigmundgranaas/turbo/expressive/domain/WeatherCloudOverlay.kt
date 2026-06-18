package com.sigmundgranaas.turbo.expressive.domain

/**
 * An opt-in capability for engines that can draw the procedural weather-cloud
 * overlay (precipitation + cloud coverage rendered as soft, GPU-shaded clouds,
 * darker = rainier). Kept *separate* from [MapEngine] because it's renderer-
 * specific: only `TurbomapMapEngine` (the wgpu/Rust engine) implements it;
 * MapLibre does not. Feature code discovers it with a cast:
 *
 * ```
 * (engine as? WeatherCloudOverlay)?.enableClouds(gridW, gridH)
 * ```
 *
 * The radar grid is two `gridW * gridH` byte planes — `precip` and `coverage`,
 * each `0..255` — the shape MET Norway's radar/nowcast + cloud-cover rasters
 * reduce to. Two timesteps are held (slot 0 = current, slot 1 = next); the
 * overlay crossfades between them by [setCloudTime]'s `blend`, which a time
 * slider scrubs forward or backward.
 */
interface WeatherCloudOverlay {
    /** Enable the overlay, allocating the radar data textures at this grid size. */
    fun enableClouds(gridW: Int, gridH: Int)

    /** Show/hide without discarding uploaded frames. */
    fun setCloudsVisible(visible: Boolean)

    /** Upload a frame into [slot] (0 = current, 1 = next) from two byte planes. */
    fun ingestRadarFrame(slot: Int, gridW: Int, gridH: Int, precip: ByteArray, coverage: ByteArray)

    /**
     * Set the animation clock ([timeSeconds], drives cloud drift) and the
     * slot-0→slot-1 crossfade ([blend], `0..1`).
     */
    fun setCloudTime(timeSeconds: Float, blend: Float)

    /**
     * Geo-register the radar to the lat/lng box it covers so the overlay is
     * **world-locked** — the clouds pan and zoom with the terrain instead of
     * staying fixed to the screen. Pass the bounds the frames were sampled for.
     * Default no-op for engines that only draw a screen-locked field.
     */
    fun setCloudGeoBounds(west: Double, south: Double, east: Double, north: Double) {}
}
