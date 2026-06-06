package com.sigmundgranaas.turbo.expressive.feature.recording

import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/** A track parsed from an imported file: an optional name plus its geometry. */
data class ParsedTrack(val name: String?, val geo: GeoPath)

/**
 * Reads GPX, KML and GeoJSON track files into a [ParsedTrack], auto-detecting
 * the format from the content. Mirrors the export side ([serialize]); the parse
 * is pure so it can be unit-tested without a file picker. Returns null when no
 * usable line geometry is found.
 */
internal object TrackImport {
    private val json = Json { ignoreUnknownKeys = true }

    fun parse(body: String): ParsedTrack? {
        val trimmed = body.trimStart()
        return when {
            trimmed.startsWith("{") -> parseGeoJson(trimmed)
            trimmed.contains("<gpx", ignoreCase = true) -> parseGpx(body)
            trimmed.contains("<kml", ignoreCase = true) -> parseKml(body)
            else -> null
        }
    }

    private fun parseGpx(body: String): ParsedTrack? {
        // trkpt / rtept carry the line; wpt are standalone points we ignore for a track.
        val pointRegex = Regex(
            """<(?:trkpt|rtept)\b[^>]*\blat="([-\d.]+)"[^>]*\blon="([-\d.]+)"[^>]*?>(.*?)</(?:trkpt|rtept)>|""" +
                """<(?:trkpt|rtept)\b[^>]*\blat="([-\d.]+)"[^>]*\blon="([-\d.]+)"[^>]*/>""",
            RegexOption.DOT_MATCHES_ALL,
        )
        val points = mutableListOf<LatLng>()
        val elevations = mutableListOf<Double?>()
        pointRegex.findAll(body).forEach { m ->
            val lat = (m.groupValues[1].ifEmpty { m.groupValues[4] }).toDoubleOrNull() ?: return@forEach
            val lon = (m.groupValues[2].ifEmpty { m.groupValues[5] }).toDoubleOrNull() ?: return@forEach
            points += LatLng(lat, lon)
            elevations += Regex("""<ele>([-\d.]+)</ele>""").find(m.groupValues[3])?.groupValues?.get(1)?.toDoubleOrNull()
        }
        if (points.size < 2) return null
        return ParsedTrack(firstTag(body, "name"), geoOf(points, elevations))
    }

    private fun parseKml(body: String): ParsedTrack? {
        val points = mutableListOf<LatLng>()
        val elevations = mutableListOf<Double?>()
        Regex("""<coordinates>(.*?)</coordinates>""", RegexOption.DOT_MATCHES_ALL).findAll(body).forEach { block ->
            block.groupValues[1].trim().split(Regex("\\s+")).forEach { tuple ->
                val parts = tuple.split(",")
                val lon = parts.getOrNull(0)?.toDoubleOrNull()
                val lat = parts.getOrNull(1)?.toDoubleOrNull()
                if (lat != null && lon != null) {
                    points += LatLng(lat, lon)
                    elevations += parts.getOrNull(2)?.toDoubleOrNull()
                }
            }
        }
        if (points.size < 2) return null
        return ParsedTrack(firstTag(body, "name"), geoOf(points, elevations))
    }

    private fun parseGeoJson(body: String): ParsedTrack? {
        val root = runCatching { json.parseToJsonElement(body).jsonObject }.getOrNull() ?: return null
        // Unwrap FeatureCollection → first Feature, Feature → geometry.
        val name = root["features"]?.let { (it as? JsonArray) }
            ?.firstOrNull()?.jsonObject?.get("properties")?.jsonObject?.get("name")?.jsonPrimitive?.contentOrNull
            ?: root["properties"]?.jsonObject?.get("name")?.jsonPrimitive?.contentOrNull
        val geometry = findGeometry(root) ?: return null
        val coordsEl = geometry["coordinates"] ?: return null
        val type = geometry["type"]?.jsonPrimitive?.contentOrNull
        val lines = when (type) {
            "LineString" -> listOf(coordsEl as? JsonArray ?: return null)
            "MultiLineString" -> (coordsEl as? JsonArray)?.mapNotNull { it as? JsonArray } ?: return null
            else -> return null
        }
        val points = mutableListOf<LatLng>()
        val elevations = mutableListOf<Double?>()
        lines.forEach { line ->
            line.forEach { pair ->
                val arr = pair as? JsonArray ?: return@forEach
                val lon = (arr.getOrNull(0) as? JsonPrimitive)?.doubleOrNull
                val lat = (arr.getOrNull(1) as? JsonPrimitive)?.doubleOrNull
                if (lat != null && lon != null) {
                    points += LatLng(lat, lon)
                    elevations += (arr.getOrNull(2) as? JsonPrimitive)?.doubleOrNull
                }
            }
        }
        if (points.size < 2) return null
        return ParsedTrack(name, geoOf(points, elevations))
    }

    private fun findGeometry(root: JsonObject): JsonObject? = when {
        root["type"]?.jsonPrimitive?.contentOrNull == "FeatureCollection" ->
            (root["features"] as? JsonArray)?.firstOrNull()?.jsonObject?.get("geometry")?.jsonObject
        root["type"]?.jsonPrimitive?.contentOrNull == "Feature" -> root["geometry"]?.jsonObject
        root.containsKey("coordinates") -> root
        else -> null
    }

    private fun geoOf(points: List<LatLng>, elevations: List<Double?>): GeoPath {
        val hasEle = elevations.any { it != null }
        return GeoPath.fromPoints(points, GeoPathSource.Saved, if (hasEle) elevations else null)
    }

    private fun firstTag(body: String, tag: String): String? =
        Regex("<$tag>(.*?)</$tag>", RegexOption.DOT_MATCHES_ALL).find(body)
            ?.groupValues?.get(1)?.trim()?.takeIf { it.isNotEmpty() }
}
