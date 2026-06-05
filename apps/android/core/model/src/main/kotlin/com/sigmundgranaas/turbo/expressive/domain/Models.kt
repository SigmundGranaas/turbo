package com.sigmundgranaas.turbo.expressive.domain

/** A simple WGS84 coordinate, independent of any map SDK type. */
data class LatLng(val lat: Double, val lng: Double)

/**
 * The 18 outdoor-activity types — the soul of the product. The Norwegian [key]
 * is the stable identifier; only [label] is translated. This is a PURE type
 * (no Compose): icon/tint visuals live in `:core:designsystem`
 * (`ActivityKindId.icon` / `.tint`).
 */
enum class ActivityKindId(val key: String, val label: String) {
    Mountain("Fjell", "Mountain"),
    Park("Park", "Park"),
    Beach("Strand", "Beach"),
    Forest("Skog", "Forest"),
    Hiking("Vandring", "Hiking"),
    Kayaking("Kajakk", "Kayaking"),
    Biking("Sykkel", "Biking"),
    Cabin("Hytte", "Cabin"),
    Parking("Parkering", "Parking"),
    Camping("Camping", "Camping Spot"),
    Swimming("Badeplass", "Swimming Spot"),
    Diving("Dykking", "Diving"),
    Viewpoint("Utkikkspunkt", "Viewpoint"),
    Restaurant("Restaurant", "Restaurant"),
    Cafe("Kafé", "Café"),
    Accommodation("Overnatting", "Accommodation"),
    Fishing("Fiskeplass", "Fishing Spot"),
    Skiing("Ski", "Skiing");

    companion object {
        fun fromKey(key: String): ActivityKindId? = entries.firstOrNull { it.key == key }
    }
}

/** A user marker pinned on the map. */
data class Marker(
    val id: String,
    val name: String,
    val kind: ActivityKindId,
    val position: LatLng,
    /** Optional override tint (ARGB). Null → the kind's terracotta default. */
    val colorArgb: Long? = null,
)

/** Base-map tile sources (matches the layer-selector in the design). */
enum class BaseLayer(val id: String, val title: String) {
    Norgeskart("topo", "Norgeskart"),
    Osm("osm", "OSM"),
    Satellite("gs", "Satellite");
}

/** Toggleable data overlays painted over the map. */
enum class OverlayId(val title: String, val subtitle: String) {
    Waves("Wave height", "Marine swell heatmap"),
    Wind("Wind", "Animated flow field"),
    Avalanche("Avalanche danger", "Varsom / NVE slopes");
}
