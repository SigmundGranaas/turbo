package com.sigmundgranaas.turbo.expressive.feature.recording

import com.sigmundgranaas.turbo.expressive.domain.SavedPath

/**
 * Serialises a [SavedPath] to GPX 1.1 — the universal track-exchange format that
 * Garmin, Strava, Komoot, Gaia, etc. all import. Per-point `<ele>` is emitted when
 * the track captured altitude; otherwise points are plain lat/lon.
 */
internal fun pathToGpx(path: SavedPath): String {
    val points = path.path.points
    val elevations = path.path.elevations
    val sb = StringBuilder(256 + points.size * 64)
    sb.append("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")
    sb.append("<gpx version=\"1.1\" creator=\"Turbo\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n")
    sb.append("  <trk>\n")
    sb.append("    <name>").append(escapeXml(path.name)).append("</name>\n")
    sb.append("    <trkseg>\n")
    points.forEachIndexed { i, p ->
        val ele = elevations?.getOrNull(i)
        sb.append("      <trkpt lat=\"").append(p.lat).append("\" lon=\"").append(p.lng).append("\">")
        if (ele != null) sb.append("<ele>").append(ele).append("</ele>")
        sb.append("</trkpt>\n")
    }
    sb.append("    </trkseg>\n")
    sb.append("  </trk>\n")
    sb.append("</gpx>\n")
    return sb.toString()
}

/** A safe filename stem derived from the track name (for the shared .gpx file). */
internal fun gpxFileName(name: String): String {
    val stem = name.trim().ifBlank { "track" }.replace(Regex("[^A-Za-z0-9-_]+"), "_").take(40)
    return "$stem.gpx"
}

private fun escapeXml(s: String): String = s
    .replace("&", "&amp;")
    .replace("<", "&lt;")
    .replace(">", "&gt;")
    .replace("\"", "&quot;")
