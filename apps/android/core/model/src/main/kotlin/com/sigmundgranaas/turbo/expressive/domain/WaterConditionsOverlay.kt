package com.sigmundgranaas.turbo.expressive.domain

/**
 * Capability for engines that render a live water surface driven by the MET
 * wave/wind forecast — wave direction + ferocity, whitecaps when the sea turns
 * extreme, and shoreline foam.
 *
 * Implemented by the wgpu engine; MapLibre does not, so the call site keys off
 * `engine as? WaterConditionsOverlay` and does nothing when absent — exactly
 * like [TerrainSunOverlay] and [WeatherCloudOverlay].
 */
interface WaterConditionsOverlay {
    /**
     * Update the sea state from the latest forecast at the point of interest.
     * Every parameter is optional — pass `null` for anything MET doesn't provide
     * (it drops fields inland / at the series tail); all-`null` ⇒ a calm default.
     *
     * @param waveFromDeg significant-wave bearing the swell comes *from* (compass°)
     * @param waveHeightM significant wave height in metres (ferocity)
     * @param windSpeedMs 10 m wind speed in m/s (roughens the surface, whitecaps)
     * @param windFromDeg wind bearing it blows *from* (compass°), a fallback for direction
     */
    fun setWaterConditions(
        waveFromDeg: Float?,
        waveHeightM: Float?,
        windSpeedMs: Float?,
        windFromDeg: Float?,
    )
}
