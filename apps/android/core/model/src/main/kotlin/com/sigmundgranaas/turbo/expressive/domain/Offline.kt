package com.sigmundgranaas.turbo.expressive.domain

/** A lat/lng bounding box (WGS84). */
data class GeoBounds(
    val south: Double,
    val west: Double,
    val north: Double,
    val east: Double,
)

/** A downloaded (or downloading) offline map region. */
data class OfflineRegionInfo(
    val id: Long,
    val name: String,
    val complete: Boolean,
    /** Download progress 0..1 (1 when complete). */
    val progress: Float,
    val sizeBytes: Long,
)
