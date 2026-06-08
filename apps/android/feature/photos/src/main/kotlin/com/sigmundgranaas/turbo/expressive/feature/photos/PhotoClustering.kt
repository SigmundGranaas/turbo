package com.sigmundgranaas.turbo.expressive.feature.photos

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Photo

/** A group of geotagged photos that sit close together on the map. */
data class PhotoCluster(
    val id: String,
    val center: LatLng,
    val photos: List<Photo>,
) {
    val count: Int get() = photos.size
    /** The newest photo's URI is the cover thumbnail. */
    val coverUri: String? get() = photos.maxByOrNull { it.capturedAtEpochMs }?.uri
    /** Photos newest-first, for the grid + viewer. */
    val ordered: List<Photo> get() = photos.sortedByDescending { it.capturedAtEpochMs }
}

/**
 * Group [photos] into clusters by snapping each to a lat/lng grid of [gridDeg]
 * degrees (~90 m at the default), then averaging each cell to a centroid. Pure +
 * deterministic so it can be unit-tested; the grid coarsens as the user zooms out
 * via [gridDeg]. Order is stable (by cell key) for predictable rendering.
 */
fun clusterPhotos(photos: List<Photo>, gridDeg: Double = 0.0008): List<PhotoCluster> {
    if (photos.isEmpty()) return emptyList()
    val step = gridDeg.coerceAtLeast(1e-6)
    return photos
        .groupBy { Math.round(it.lat / step) to Math.round(it.lng / step) }
        .toSortedMap(compareBy({ it.first }, { it.second }))
        .map { (cell, group) ->
            val lat = group.sumOf { it.lat } / group.size
            val lng = group.sumOf { it.lng } / group.size
            PhotoCluster(id = "pc-${cell.first}_${cell.second}", center = LatLng(lat, lng), photos = group)
        }
}
