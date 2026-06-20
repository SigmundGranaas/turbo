package com.sigmundgranaas.turbo.expressive.domain

/**
 * Capability for engines that can light the 3D terrain by a movable sun —
 * driving the analytic atmosphere/sky colour, the terrain shading, and the
 * cast shadows (a peak shadowing the valley behind it).
 *
 * Implemented by the wgpu engine; MapLibre does not, so "sun mode" UI keys off
 * `engine as? TerrainSunOverlay` and renders nothing when absent (the call site
 * stays a one-liner, exactly like [WeatherCloudOverlay]).
 */
interface TerrainSunOverlay {
    /**
     * Place the sun at a real UTC instant ([unixSeconds], seconds since the
     * epoch). The engine solves the solar position at the camera's location, so
     * the light + sky colour + shadow direction track the time of day. A
     * negative value reverts to the fixed default sun.
     */
    fun setSunTime(unixSeconds: Double)

    /**
     * Terrain cast-shadow strength in `[0,1]`: 0 disables them (zero per-frame
     * cost), higher = darker occlusion. Only affects 3D terrain.
     */
    fun setTerrainShadows(strength: Float)
}
