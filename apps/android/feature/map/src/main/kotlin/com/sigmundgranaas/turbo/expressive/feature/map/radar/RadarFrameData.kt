package com.sigmundgranaas.turbo.expressive.feature.map.radar

/**
 * One radar timestep ready to hand to the GPU overlay: two `gridW * gridH`
 * byte planes — [precip] and [coverage], each `0..255` — plus the instant it
 * represents. This is the normalised shape every [RadarDataSource] produces,
 * whatever raster it decoded (MET radar reflectivity, nowcast, cloud cover).
 *
 * A plain class (not `data class`) because the planes are arrays — value
 * equality would be by reference anyway and isn't needed.
 */
class RadarFrameData(
    val gridW: Int,
    val gridH: Int,
    val precip: ByteArray,
    val coverage: ByteArray,
    val epochMillis: Long,
)
