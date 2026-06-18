package com.sigmundgranaas.turbo.expressive.domain

/**
 * One timestep of gridded weather for the cloud overlay, in the shape the GPU
 * overlay consumes: two `gridW * gridH` byte planes — [precip] and [coverage],
 * each `0..255` — plus the instant it represents. [coverage] drives *where*
 * cloud is (cloud-area-fraction), [precip] drives how dark/rainy it gets.
 *
 * Built in `core:data` by sampling MET Norway's gridded forecast over the
 * viewport and resampling to the grid; the map overlay geo-registers it to the
 * lat/lng box the frames were sampled for. A plain class (the planes are arrays,
 * so value equality would be by reference anyway).
 */
class RadarGridFrame(
    val gridW: Int,
    val gridH: Int,
    val precip: ByteArray,
    val coverage: ByteArray,
    val epochMillis: Long,
)
