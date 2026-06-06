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

/**
 * Serialises a [SavedPath] to a GeoJSON Feature (LineString). Coordinates are
 * GeoJSON order [lng, lat(, ele)]; track metadata rides along in `properties`.
 */
internal fun pathToGeoJson(path: SavedPath): String {
    val points = path.path.points
    val elevations = path.path.elevations
    val coords = points.mapIndexed { i, p ->
        val ele = elevations?.getOrNull(i)
        if (ele != null) "[${p.lng},${p.lat},$ele]" else "[${p.lng},${p.lat}]"
    }.joinToString(",")
    return buildString {
        append("{\"type\":\"Feature\",")
        append("\"properties\":{\"name\":").append(jsonString(path.name)).append("},")
        append("\"geometry\":{\"type\":\"LineString\",\"coordinates\":[").append(coords).append("]}}")
    }
}

/**
 * Serialises a [SavedPath] to a KML 2.2 Placemark (LineString) — the format
 * Google Earth and many GIS tools expect. Coordinates are `lng,lat,ele` tuples.
 */
internal fun pathToKml(path: SavedPath): String {
    val points = path.path.points
    val elevations = path.path.elevations
    val coords = points.mapIndexed { i, p ->
        val ele = elevations?.getOrNull(i) ?: 0.0
        "${p.lng},${p.lat},$ele"
    }.joinToString(" ")
    return buildString {
        append("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")
        append("<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n  <Document>\n    <Placemark>\n")
        append("      <name>").append(escapeXml(path.name)).append("</name>\n")
        append("      <LineString><coordinates>").append(coords).append("</coordinates></LineString>\n")
        append("    </Placemark>\n  </Document>\n</kml>\n")
    }
}

/** Track-exchange formats the app can export a [SavedPath] to. */
internal enum class ExportFormat(val label: String, val extension: String, val mimeType: String) {
    Gpx("GPX", "gpx", "application/gpx+xml"),
    GeoJson("GeoJSON", "geojson", "application/geo+json"),
    Kml("KML", "kml", "application/vnd.google-earth.kml+xml"),
}

internal fun serialize(path: SavedPath, format: ExportFormat): String = when (format) {
    ExportFormat.Gpx -> pathToGpx(path)
    ExportFormat.GeoJson -> pathToGeoJson(path)
    ExportFormat.Kml -> pathToKml(path)
}

/** A safe filename derived from the track name for the given [format]. */
internal fun exportFileName(name: String, format: ExportFormat): String {
    val stem = name.trim().ifBlank { "track" }.replace(Regex("[^A-Za-z0-9-_]+"), "_").take(40)
    return "$stem.${format.extension}"
}

/** A safe filename stem derived from the track name (for the shared .gpx file). */
internal fun gpxFileName(name: String): String = exportFileName(name, ExportFormat.Gpx)

private fun escapeXml(s: String): String = s
    .replace("&", "&amp;")
    .replace("<", "&lt;")
    .replace(">", "&gt;")
    .replace("\"", "&quot;")

private fun jsonString(s: String): String = "\"" + s
    .replace("\\", "\\\\")
    .replace("\"", "\\\"")
    .replace("\n", "\\n") + "\""
