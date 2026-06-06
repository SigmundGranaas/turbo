package com.sigmundgranaas.turbo.expressive.domain

/**
 * A geotagged photo stored locally. [uri] is a content/file URI string the app
 * can render; [markerId] links it to a marker when attached (null = standalone,
 * shown only on the geotagged photo map layer).
 */
data class Photo(
    val id: String,
    val markerId: String?,
    val lat: Double,
    val lng: Double,
    val uri: String,
    val capturedAtEpochMs: Long,
) {
    val position: LatLng get() = LatLng(lat, lng)
}
