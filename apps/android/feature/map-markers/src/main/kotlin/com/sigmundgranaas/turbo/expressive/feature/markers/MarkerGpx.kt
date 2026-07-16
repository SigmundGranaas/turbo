package com.sigmundgranaas.turbo.expressive.feature.markers

import com.sigmundgranaas.turbo.expressive.domain.Marker

/**
 * Serialises [Marker]s to GPX 1.1 `<wpt>` waypoints — the universal waypoint
 * format Garmin/Gaia/Locus import, complementing [MarkerGeoJson] the same way
 * the track export offers both. Name → `<name>`, notes → `<desc>`, kind →
 * `<sym>`. Pure string building, mirroring [MarkerGeoJson].
 */
object MarkerGpx {
    fun encode(markers: List<Marker>): String {
        val sb = StringBuilder(256 + markers.size * 96)
        sb.append("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")
        sb.append("<gpx version=\"1.1\" creator=\"Turbo\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n")
        markers.forEach { m ->
            sb.append("  <wpt lat=\"").append(m.position.lat).append("\" lon=\"").append(m.position.lng).append("\">\n")
            sb.append("    <name>").append(xml(m.name)).append("</name>\n")
            m.notes?.takeIf { it.isNotBlank() }?.let { sb.append("    <desc>").append(xml(it)).append("</desc>\n") }
            sb.append("    <sym>").append(xml(m.kind.key)).append("</sym>\n")
            sb.append("  </wpt>\n")
        }
        sb.append("</gpx>\n")
        return sb.toString()
    }

    /** A safe .gpx filename stem from a marker/collection name. */
    fun fileName(name: String): String {
        val stem = name.trim().ifBlank { "markers" }.replace(Regex("[^A-Za-z0-9-_]+"), "_").take(40)
        return "$stem.gpx"
    }

    private fun xml(s: String): String = s
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
}
