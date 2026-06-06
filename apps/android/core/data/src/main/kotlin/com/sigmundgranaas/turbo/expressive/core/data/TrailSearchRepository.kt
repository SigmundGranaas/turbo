package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SearchHit
import io.ktor.client.HttpClient
import io.ktor.client.request.get
import io.ktor.client.request.header
import io.ktor.client.request.parameter
import io.ktor.client.statement.bodyAsText
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.doubleOrNull
import javax.inject.Inject

/**
 * Named-trail search over Geonorge's friluftsruter2 WFS (the Nasjonalturbase
 * foot-route layer). Mirrors the Flutter `TrailSearchService`: a CQL `ILIKE`
 * filter on the route name, surfacing the first vertex of each route as its
 * position so it can be shown as a search hit.
 */
interface TrailSearchRepository {
    suspend fun search(query: String): Outcome<List<SearchHit>>
}

class GeonorgeTrailSearchRepository @Inject constructor(
    private val client: HttpClient,
) : TrailSearchRepository {

    override suspend fun search(query: String): Outcome<List<SearchHit>> = try {
        val escaped = query.trim().replace("'", "''")
        val raw = client
            .get("https://wfs.geonorge.no/skwms1/wfs.friluftsruter2") {
                parameter("SERVICE", "WFS")
                parameter("VERSION", "2.0.0")
                parameter("REQUEST", "GetFeature")
                parameter("TYPENAMES", "fotrute")
                parameter("OUTPUTFORMAT", "application/json")
                parameter("SRSNAME", "urn:ogc:def:crs:EPSG::4326")
                parameter("COUNT", 10)
                parameter("CQL_FILTER", "navn ILIKE '%$escaped%'")
                header("User-Agent", USER_AGENT)
            }
            .bodyAsText()
        Outcome.Success(TrailWfs.parse(raw))
    } catch (t: Throwable) {
        Outcome.Failure(t)
    }

    private companion object {
        const val USER_AGENT = "turbo-expressive/0.1 github.com/SigmundGranaas/turbo"
    }
}

/** Pure GeoJSON-FeatureCollection → [SearchHit] parsing, isolated for testability. */
internal object TrailWfs {
    private val json = Json { ignoreUnknownKeys = true }

    fun parse(body: String): List<SearchHit> =
        runCatching { json.decodeFromString<TrailFeatureCollection>(body) }
            .getOrNull()
            ?.features
            ?.mapNotNull(TrailFeature::toHit)
            .orEmpty()
}

@Serializable
private data class TrailFeatureCollection(val features: List<TrailFeature> = emptyList())

@Serializable
private data class TrailFeature(
    val properties: TrailProperties? = null,
    val geometry: TrailGeometry? = null,
)

@Serializable
private data class TrailProperties(
    val navn: String? = null,
    val rutenummer: String? = null,
    val merkemetode: String? = null,
)

@Serializable
private data class TrailGeometry(
    val type: String? = null,
    val coordinates: JsonElement? = null,
)

private fun TrailFeature.toHit(): SearchHit? {
    val name = properties?.navn?.takeIf(String::isNotBlank) ?: return null
    val firstVertex = geometry?.coordinates?.let(::firstLngLat) ?: return null
    val desc = listOfNotNull(
        "Trail",
        properties.rutenummer?.takeIf(String::isNotBlank),
        properties.merkemetode?.takeIf(String::isNotBlank),
    ).joinToString(" · ")
    return SearchHit(name = name, description = desc, position = firstVertex)
}

/**
 * GeoJSON coordinates are [lng, lat] and arbitrarily nested (Point → pair,
 * LineString → list of pairs, MultiLineString → list of lists). Descend into the
 * first child until a `[lng, lat]` numeric pair is reached, then swap to [LatLng].
 */
private fun firstLngLat(element: JsonElement): LatLng? {
    var node: JsonElement = element
    repeat(MAX_DEPTH) {
        val arr = node as? JsonArray ?: return null
        val head = arr.firstOrNull() ?: return null
        if (head is JsonPrimitive) {
            val lng = (arr.getOrNull(0) as? JsonPrimitive)?.doubleOrNull ?: return null
            val lat = (arr.getOrNull(1) as? JsonPrimitive)?.doubleOrNull ?: return null
            return LatLng(lat, lng)
        }
        node = head
    }
    return null
}

private const val MAX_DEPTH = 6
