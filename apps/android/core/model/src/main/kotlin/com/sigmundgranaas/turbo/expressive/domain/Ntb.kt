package com.sigmundgranaas.turbo.expressive.domain

/** What kind of Nasjonal Turbase (ut.no / DNT) object a marker represents. */
enum class NtbPoiType { Cabin, Trip, Place }

/**
 * A lightweight Nasjonal Turbase marker (cabin / trip / place) as served by the
 * backend proxy (`/api/places/ntb`). The proxy already normalised it, so this is
 * a thin value type. A [Trip] additionally has a route polyline fetched lazily
 * via [NasjonalTurbaseRepository.route].
 */
data class NtbPoi(
    val id: String,
    val type: NtbPoiType,
    val title: String,
    val position: LatLng,
    val summary: String? = null,
    val imageUrl: String? = null,
    /** Best link back to ut.no (proxy-provided), or null. */
    val utUrl: String? = null,
) {
    val hasRoute: Boolean get() = type == NtbPoiType.Trip
}

/** A trip's full detail: the route polyline to reveal plus sheet metadata. */
data class NtbRoute(
    val id: String,
    val title: String,
    val points: List<LatLng>,
    val description: String? = null,
    val distanceMeters: Double? = null,
    val grade: String? = null,
    val imageUrl: String? = null,
    val utUrl: String? = null,
) {
    val hasGeometry: Boolean get() = points.size >= 2
}
