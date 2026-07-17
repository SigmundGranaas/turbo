package com.sigmundgranaas.turbo.expressive.domain

/** A simple WGS84 coordinate, independent of any map SDK type. */
data class LatLng(val lat: Double, val lng: Double)

/**
 * The 18 outdoor-activity types — the soul of the product. The Norwegian [key]
 * is the stable identifier. This is a PURE type (no Compose): the localized
 * display label and icon/tint visuals live in `:core:designsystem`
 * (`ActivityKindId.labelRes` / `.icon` / `.tint`).
 */
enum class ActivityKindId(val key: String) {
    Mountain("Fjell"),
    Park("Park"),
    Beach("Strand"),
    Forest("Skog"),
    Hiking("Vandring"),
    Kayaking("Kajakk"),
    Biking("Sykkel"),
    Cabin("Hytte"),
    Parking("Parkering"),
    Camping("Camping"),
    Swimming("Badeplass"),
    Diving("Dykking"),
    Viewpoint("Utkikkspunkt"),
    Restaurant("Restaurant"),
    Cafe("Kafé"),
    Accommodation("Overnatting"),
    Fishing("Fiskeplass"),
    Skiing("Ski");

    companion object {
        fun fromKey(key: String): ActivityKindId? = entries.firstOrNull { it.key == key }
    }
}

/**
 * What role a [Marker] plays on the map. [Standard] is a plain user pin (its icon comes
 * from the [ActivityKindId]); [WeatherPin] is a live, cached weather node — the very same
 * persisted marker, distinguished only by this discriminator plus its cached forecast.
 * Keeping the activity [ActivityKindId] separate means a weather pin never pollutes the
 * 18-type activity picker.
 */
enum class MarkerKind { Standard, WeatherPin }

/** A user marker pinned on the map. */
data class Marker(
    val id: String,
    val name: String,
    val kind: ActivityKindId,
    val position: LatLng,
    /** Optional override tint (ARGB). Null → the kind's terracotta default. */
    val colorArgb: Long? = null,
    /** Free-text note the user attached to the pin. */
    val notes: String? = null,
    /** Plain pin vs live weather node. Defaults to [MarkerKind.Standard]. */
    val markerKind: MarkerKind = MarkerKind.Standard,
    /** Last MET forecast cached on a [MarkerKind.WeatherPin] (offline-safe render source); null otherwise. */
    val forecast: WeatherSnapshot? = null,
    /** Epoch ms the [forecast] was fetched — drives staleness (>1h) and the "updated Nh ago" cue. */
    val forecastFetchedAtEpochMs: Long? = null,
)

/** Base-map tile sources (matches the layer-selector in the design). */
enum class BaseLayer(val id: String, val title: String) {
    Norgeskart("topo", "Norgeskart"),
    Osm("osm", "OSM"),
    Satellite("gs", "Satellite");
}

/** Toggleable data overlays painted over the map. */
enum class OverlayId(val title: String, val subtitle: String) {
    Trails("Hiking trails", "Marked routes · Waymarked Trails"),
    Waves("Wave height", "Marine swell heatmap"),
    Wind("Wind", "Animated flow field"),
    Avalanche("Avalanche danger", "Varsom / NVE slopes");
}
