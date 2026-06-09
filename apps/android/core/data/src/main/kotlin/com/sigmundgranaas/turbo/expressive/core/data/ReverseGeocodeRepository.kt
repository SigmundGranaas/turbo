package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.LocationDescription
import com.sigmundgranaas.turbo.expressive.domain.PlaceQualifier
import io.ktor.client.HttpClient
import io.ktor.client.request.get
import io.ktor.client.request.header
import io.ktor.client.request.parameter
import io.ktor.client.statement.bodyAsText
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.util.concurrent.ConcurrentHashMap
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Turns a tapped coordinate into a human description ("On Galdhøpiggen, 2469 m" /
 * "In Lom") by composing Kartverket backends: the nearest tiered place name
 * (stedsnavn), a nearby address, the kommune fallback, and an elevation lookup
 * (høydedata). Results are cached per ~250 m grid cell. Mirrors the Flutter
 * `KartverketReverseGeocoder` cascade; the protected-area (Naturbase) layer is
 * not yet ported.
 */
interface ReverseGeocodeRepository {
    suspend fun describe(point: LatLng): Outcome<LocationDescription>
}

@Singleton
class KartverketReverseGeocodeRepository @Inject constructor(
    private val client: HttpClient,
) : ReverseGeocodeRepository {

    private val cache = ConcurrentHashMap<Long, LocationDescription>()

    override suspend fun describe(point: LatLng): Outcome<LocationDescription> {
        val cellKey = ReverseGeocode.cellKey(point)
        cache[cellKey]?.let { return Outcome.Success(it) }
        return try {
            coroutineScope {
                // The sources are independent — fetch concurrently.
                val nameDeferred = async { runCatching { fetchNearestName(point) }.getOrNull() }
                val protectedDeferred = async { runCatching { fetchProtectedArea(point) }.getOrNull() }
                val addressDeferred = async { runCatching { fetchAddress(point) }.getOrNull() }
                val kommuneDeferred = async { runCatching { fetchKommune(point) }.getOrNull() }
                val elevationDeferred = async { runCatching { fetchElevation(point) }.getOrNull() }

                val name = nameDeferred.await()
                val protectedArea = protectedDeferred.await()
                val address = addressDeferred.await()
                val kommune = kommuneDeferred.await()
                val elevation = elevationDeferred.await()

                val description = ReverseGeocode.compose(name, protectedArea, address, kommune, elevation)
                    ?: return@coroutineScope Outcome.Failure(IllegalStateException("No description"))
                cache[cellKey] = description
                Outcome.Success(description)
            }
        } catch (t: Throwable) {
            Outcome.Failure(t)
        }
    }

    private suspend fun fetchNearestName(point: LatLng): ReverseGeocode.NearbyName? {
        val raw = client
            .get("https://ws.geonorge.no/stedsnavn/v1/punkt") {
                parameter("nord", "%.5f".format(point.lat))
                parameter("ost", "%.5f".format(point.lng))
                parameter("koordsys", 4258)
                parameter("radius", 1000)
                parameter("treffPerSide", 25)
                parameter("navnestatus", "hovednavn")
                header("User-Agent", USER_AGENT)
            }
            .bodyAsText()
        return ReverseGeocode.pickNearestName(ReverseGeocode.parseStedsnavn(raw))
    }

    /**
     * Miljødirektoratet's Vern (Naturbase) ArcGIS Identify — national parks, nature
     * reserves, landscape-protected areas. Stedsnavn doesn't carry these polygons, so
     * without this the geocoder can't say "In Jotunheimen nasjonalpark". Layer scope is
     * the long-shipped `all:1`; out of coverage → null.
     */
    private suspend fun fetchProtectedArea(point: LatLng): ReverseGeocode.ProtectedArea? {
        val half = 0.005 // ~500 m at Nordic latitudes
        val geometry = """{"x":${point.lng},"y":${point.lat},"spatialReference":{"wkid":4326}}"""
        val extent = """{"xmin":${point.lng - half},"ymin":${point.lat - half},""" +
            """"xmax":${point.lng + half},"ymax":${point.lat + half},"spatialReference":{"wkid":4326}}"""
        val raw = client
            .get("https://kart.miljodirektoratet.no/arcgis/rest/services/vern/MapServer/identify") {
                parameter("geometry", geometry)
                parameter("geometryType", "esriGeometryPoint")
                parameter("sr", 4326)
                parameter("tolerance", 1)
                parameter("layers", "all:1")
                parameter("mapExtent", extent)
                parameter("imageDisplay", "400,400,96")
                parameter("returnGeometry", false)
                parameter("f", "json")
                header("User-Agent", USER_AGENT)
            }
            .bodyAsText()
        return ReverseGeocode.parseProtectedArea(raw)
    }

    private suspend fun fetchAddress(point: LatLng): String? {
        val raw = client
            .get("https://ws.geonorge.no/adresser/v1/punktsok") {
                parameter("lat", "%.5f".format(point.lat))
                parameter("lon", "%.5f".format(point.lng))
                parameter("radius", 200)
                parameter("treffPerSide", 1)
                header("User-Agent", USER_AGENT)
            }
            .bodyAsText()
        return ReverseGeocode.parseAddress(raw)
    }

    private suspend fun fetchKommune(point: LatLng): ReverseGeocode.Kommune? {
        val raw = client
            .get("https://ws.geonorge.no/kommuneinfo/v1/punkt") {
                parameter("nord", "%.5f".format(point.lat))
                parameter("ost", "%.5f".format(point.lng))
                parameter("koordsys", 4258)
                header("User-Agent", USER_AGENT)
            }
            .bodyAsText()
        return ReverseGeocode.parseKommune(raw)
    }

    private suspend fun fetchElevation(point: LatLng): Double? {
        val raw = client
            .get("https://ws.geonorge.no/hoydedata/v1/punkt") {
                parameter("nord", "%.5f".format(point.lat))
                parameter("ost", "%.5f".format(point.lng))
                parameter("koordsys", 4258)
                parameter("geojson", false)
                header("User-Agent", USER_AGENT)
            }
            .bodyAsText()
        return ReverseGeocode.parseElevation(raw)
    }

    private companion object {
        const val USER_AGENT = "turbo-expressive/0.1 github.com/SigmundGranaas/turbo"
    }
}

/**
 * Pure parsing + composition for reverse-geocoding, isolated from networking so
 * the tier/qualifier logic and the priority cascade can be unit-tested.
 */
internal object ReverseGeocode {
    private val json = Json { ignoreUnknownKeys = true }

    /** A candidate place name near the queried point. */
    data class NearbyName(val name: String, val type: String, val distanceM: Double)

    data class Kommune(val name: String, val fylke: String?)

    /** A Naturbase protected area containing the point (national park / reserve / …). */
    data class ProtectedArea(val name: String, val kind: String?)

    /** ~250 m grid cell (0.0025°): quantize lat/lng and pack into a stable key. */
    fun cellKey(point: LatLng): Long {
        val qLat = Math.round(point.lat * GRID).toInt()
        val qLng = Math.round(point.lng * GRID).toInt()
        return (qLat.toLong() shl 32) xor (qLng.toLong() and 0xFFFFFFFFL)
    }

    fun parseStedsnavn(body: String): List<NearbyName> =
        runCatching {
            json.parseToJsonElement(body).jsonObject["navn"]?.jsonArray.orEmpty().mapNotNull { el ->
                val obj = el.jsonObject
                val name = obj["skrivemåte"]?.jsonPrimitive?.contentOrNull?.takeIf(String::isNotBlank)
                    ?: return@mapNotNull null
                val type = obj["navneobjekttype"]?.jsonPrimitive?.contentOrNull.orEmpty()
                val dist = obj["meterFraPunkt"]?.jsonPrimitive?.doubleOrNull ?: Double.MAX_VALUE
                NearbyName(name, type, dist)
            }
        }.getOrNull().orEmpty()

    /**
     * Choose the most descriptive nearby name: peaks/waters very close win as a
     * precise "On"/"At"; settlements within a wider radius become "In"; anything
     * else within range is a loose "Near". Ranks by (category, distance).
     */
    fun pickNearestName(candidates: List<NearbyName>): NearbyName? =
        candidates
            .mapNotNull { c -> categoryOf(c)?.let { it to c } }
            .filter { (cat, c) -> c.distanceM <= cat.radiusM }
            .minByOrNull { (cat, c) -> cat.ordinal * RANK_SPREAD + c.distanceM }
            ?.second

    fun qualifierFor(name: NearbyName): PlaceQualifier =
        when (categoryOf(name)) {
            Category.Peak -> PlaceQualifier.On
            Category.Settlement -> PlaceQualifier.In
            Category.Water -> PlaceQualifier.At
            else -> PlaceQualifier.Near
        }

    fun parseKommune(body: String): Kommune? = runCatching {
        val obj = json.parseToJsonElement(body).jsonObject
        val name = obj["kommunenavn"]?.jsonPrimitive?.contentOrNull?.takeIf(String::isNotBlank) ?: return null
        Kommune(name, obj["fylkesnavn"]?.jsonPrimitive?.contentOrNull?.takeIf(String::isNotBlank))
    }.getOrNull()

    /**
     * Parse an ArcGIS Identify response from the Vern MapServer into the best
     * containing area: national park → nature reserve → anything else. Area *codes*
     * ("VV00002858") are rejected so they never surface as a title.
     */
    fun parseProtectedArea(body: String): ProtectedArea? = runCatching {
        val results = json.parseToJsonElement(body).jsonObject["results"]?.jsonArray.orEmpty()
        val areas = results.mapNotNull { el ->
            val obj = el.jsonObject
            val name = readAreaName(obj) ?: return@mapNotNull null
            ProtectedArea(name, readAreaKind(obj))
        }
        pickArea(areas)
    }.getOrNull()

    fun pickArea(areas: List<ProtectedArea>): ProtectedArea? {
        if (areas.isEmpty()) return null
        return areas.firstOrNull { it.kind?.lowercase()?.contains("nasjonalpark") == true }
            ?: areas.firstOrNull { it.kind?.lowercase()?.contains("naturreservat") == true }
            ?: areas.first()
    }

    /** ArcGIS puts the display value in `value` or a named attribute; Naturbase uses
     *  navn / områdenavn variants. Probe named keys first, fall back to `value`. */
    private fun readAreaName(result: JsonObject): String? {
        val attrs = result["attributes"] as? JsonObject
        val keys = listOf("navn", "Navn", "NAVN", "områdenavn", "OMRÅDENAVN", "omradenavn", "Name", "NAME")
        val candidates = keys.mapNotNull { attrs?.get(it) } + listOfNotNull(result["value"])
        return candidates.firstNotNullOfOrNull { raw ->
            (raw as? JsonPrimitive)?.contentOrNull?.trim()
                ?.takeIf { it.isNotEmpty() && it != "Null" && !looksLikeNaturbaseCode(it) }
        }
    }

    /** `verneform` is Naturbase's protection-class label; fall back to the layer name. */
    private fun readAreaKind(result: JsonObject): String? {
        val attrs = result["attributes"] as? JsonObject
        for (k in listOf("verneform", "Verneform", "VERNEFORM")) {
            val v = (attrs?.get(k) as? JsonPrimitive)?.contentOrNull?.trim()
            if (!v.isNullOrEmpty() && v != "Null") return v
        }
        return (result["layerName"] as? JsonPrimitive)?.contentOrNull?.trim()?.takeIf(String::isNotBlank)
    }

    /** Naturbase area codes like "VV00002858" / "LVO00567" — uppercase prefix + digits. */
    fun looksLikeNaturbaseCode(s: String): Boolean = Regex("""^[A-Z]{1,5}\d{3,}$""").matches(s)

    fun parseAddress(body: String): String? = runCatching {
        val first = json.parseToJsonElement(body).jsonObject["adresser"]?.jsonArray?.firstOrNull()?.jsonObject
            ?: return null
        first["adressetekst"]?.jsonPrimitive?.contentOrNull?.takeIf(String::isNotBlank)
    }.getOrNull()

    fun parseElevation(body: String): Double? = runCatching {
        val z = json.parseToJsonElement(body).jsonObject["punkter"]?.jsonArray?.firstOrNull()
            ?.jsonObject?.get("z")?.jsonPrimitive?.doubleOrNull ?: return null
        z.takeIf { it.isFinite() && it > MIN_ELEV && it < MAX_ELEV }
    }.getOrNull()

    /**
     * Cascade (mirrors the Flutter geocoder): a *precise* nearby toponym (peak/water)
     * wins; otherwise a containing protected area (national park / reserve) — more
     * informative than a distant settlement; then any nearby named place; then a
     * human-readable address; then the kommune. A bare gårds-/bruksnummer ("155/1/73")
     * is skipped in favour of the kommune when one is known.
     */
    fun compose(
        name: NearbyName?,
        protectedArea: ProtectedArea?,
        address: String?,
        kommune: Kommune?,
        elevationM: Double?,
    ): LocationDescription? {
        val usableAddress = address?.takeUnless { looksLikeParcelCode(it) }
        val precise = name?.takeIf {
            qualifierFor(it) == PlaceQualifier.On || qualifierFor(it) == PlaceQualifier.At
        }
        return when {
            precise != null -> LocationDescription(
                title = precise.name,
                qualifier = qualifierFor(precise),
                secondary = precise.type.takeIf(String::isNotBlank) ?: kommune?.name,
                elevationM = elevationM,
            )
            protectedArea != null -> LocationDescription(
                title = protectedArea.name,
                qualifier = PlaceQualifier.In,
                secondary = protectedArea.kind ?: kommune?.name,
                elevationM = elevationM,
            )
            name != null -> LocationDescription(
                title = name.name,
                qualifier = qualifierFor(name),
                secondary = name.type.takeIf(String::isNotBlank) ?: kommune?.name,
                elevationM = elevationM,
            )
            usableAddress != null -> LocationDescription(
                title = usableAddress,
                qualifier = PlaceQualifier.Near,
                secondary = kommune?.name,
                elevationM = elevationM,
            )
            kommune != null -> LocationDescription(
                title = kommune.name,
                qualifier = PlaceQualifier.In,
                secondary = kommune.fylke,
                elevationM = elevationM,
            )
            // Last resort: a parcel-code address with no kommune is still better than nothing.
            address != null -> LocationDescription(address, PlaceQualifier.Near, null, elevationM)
            else -> null
        }
    }

    /** True for digits-and-separators-only strings like "155/1/73" (a gnr/bnr parcel code). */
    fun looksLikeParcelCode(s: String): Boolean = Regex("""^[0-9]+([/ -][0-9]+)+$""").matches(s.trim())

    private enum class Category(val radiusM: Double) {
        Peak(600.0), Water(150.0), Settlement(2500.0), Other(400.0)
    }

    private fun categoryOf(name: NearbyName): Category? {
        val t = name.type.lowercase()
        return when {
            PEAK_TYPES.any { t.contains(it) } -> Category.Peak
            WATER_TYPES.any { t.contains(it) } -> Category.Water
            SETTLEMENT_TYPES.any { t.contains(it) } -> Category.Settlement
            t.isNotBlank() -> Category.Other
            else -> null
        }
    }

    private const val GRID = 400.0
    private const val RANK_SPREAD = 100_000.0
    private const val MIN_ELEV = -1000.0
    private const val MAX_ELEV = 9000.0
    private val PEAK_TYPES = listOf("fjell", "topp", "ås", "haug", "nut", "tind", "egg", "berg")
    private val WATER_TYPES = listOf("vann", "vatn", "innsjø", "elv", "tjern", "bekk", "fjord")
    private val SETTLEMENT_TYPES = listOf("by", "tettsted", "tettbebyggelse", "bygd", "grend")
}
