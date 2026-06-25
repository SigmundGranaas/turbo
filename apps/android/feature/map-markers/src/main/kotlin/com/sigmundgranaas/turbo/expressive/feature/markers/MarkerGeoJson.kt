package com.sigmundgranaas.turbo.expressive.feature.markers

import com.sigmundgranaas.turbo.expressive.domain.Marker

/**
 * Serialises [Marker]s to a GeoJSON FeatureCollection of Points. Coordinates are
 * GeoJSON order [lng, lat]; name / kind / notes ride along in each feature's
 * `properties`. Pure string building (no JSON dependency in this module).
 */
internal object MarkerGeoJson {
    fun encode(markers: List<Marker>): String {
        val features = markers.joinToString(",") { m ->
            val props = buildString {
                append("\"name\":").append(str(m.name))
                append(",\"kind\":").append(str(m.kind.key))
                m.notes?.takeIf { it.isNotBlank() }?.let { append(",\"notes\":").append(str(it)) }
            }
            "{\"type\":\"Feature\",\"properties\":{$props}," +
                "\"geometry\":{\"type\":\"Point\",\"coordinates\":[${m.position.lng},${m.position.lat}]}}"
        }
        return "{\"type\":\"FeatureCollection\",\"features\":[$features]}"
    }

    /** A safe .geojson filename stem from a marker/collection name. */
    fun fileName(name: String): String {
        val stem = name.trim().ifBlank { "markers" }.replace(Regex("[^A-Za-z0-9-_]+"), "_").take(40)
        return "$stem.geojson"
    }

    private fun str(s: String): String = "\"" + s
        .replace("\\", "\\\\")
        .replace("\"", "\\\"")
        .replace("\n", "\\n") + "\""
}
