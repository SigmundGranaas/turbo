package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.NtbPoi
import com.sigmundgranaas.turbo.expressive.domain.NtbPoiType
import com.sigmundgranaas.turbo.expressive.domain.NtbRoute
import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.parameter
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import javax.inject.Inject

/**
 * Nasjonal Turbase (ut.no / DNT) markers, served by the Turbo backend proxy
 * (`/api/places/ntb`). The proxy holds the api key and normalises the data, so
 * this client just does HTTP + DTO→domain mapping. Failures degrade to
 * empty/null so a flaky source never breaks the map.
 */
interface NasjonalTurbaseRepository {
    /** Cabins, places and trip markers within [bounds]. */
    suspend fun pois(bounds: GeoBounds): List<NtbPoi>

    /** A trip's full route (polyline + metadata), or null. */
    suspend fun route(id: String): NtbRoute?
}

class HttpNasjonalTurbaseRepository @Inject constructor(
    private val client: HttpClient,
) : NasjonalTurbaseRepository {

    override suspend fun pois(bounds: GeoBounds): List<NtbPoi> = try {
        val res: PoisResponseDto = client.get("$BASE_URL/pois") {
            parameter("minLat", bounds.south)
            parameter("minLon", bounds.west)
            parameter("maxLat", bounds.north)
            parameter("maxLon", bounds.east)
        }.body()
        res.pois.mapNotNull(NtbPoiDto::toDomain)
    } catch (_: Throwable) {
        emptyList()
    }

    override suspend fun route(id: String): NtbRoute? = try {
        client.get("$BASE_URL/route/$id").body<NtbRouteDto>().toDomain()
    } catch (_: Throwable) {
        null
    }

    companion object {
        const val BASE_URL = "https://kart-api.sandring.no/api/places/ntb"
    }
}

// ── Proxy DTOs (the proxy already normalised ut.no/DNT into this shape) ──

@Serializable
internal data class PoisResponseDto(val pois: List<NtbPoiDto> = emptyList())

@Serializable
internal data class NtbPoiDto(
    val id: String = "",
    val type: String = "place",
    val lat: Double? = null,
    val lng: Double? = null,
    val title: String = "",
    val summary: String? = null,
    val imageUrl: String? = null,
    val utUrl: String? = null,
)

@Serializable
internal data class NtbRouteDto(
    val id: String = "",
    val title: String = "",
    /** GeoJSON order [lng, lat]. */
    val points: List<List<Double>> = emptyList(),
    val description: String? = null,
    val distanceMeters: Double? = null,
    val grade: String? = null,
    val imageUrl: String? = null,
    val utUrl: String? = null,
)

internal fun NtbPoiDto.toDomain(): NtbPoi? {
    val la = lat ?: return null
    val ln = lng ?: return null
    return NtbPoi(
        id = id,
        type = type.toPoiType(),
        title = title,
        position = LatLng(la, ln),
        summary = summary,
        imageUrl = imageUrl,
        utUrl = utUrl,
    )
}

internal fun NtbRouteDto.toDomain(): NtbRoute = NtbRoute(
    id = id,
    title = title,
    points = points.mapNotNull { p ->
        // [lng, lat] → LatLng
        if (p.size >= 2) LatLng(p[1], p[0]) else null
    },
    description = description,
    distanceMeters = distanceMeters,
    grade = grade,
    imageUrl = imageUrl,
    utUrl = utUrl,
)

private fun String.toPoiType(): NtbPoiType = when (this) {
    "cabin" -> NtbPoiType.Cabin
    "trip" -> NtbPoiType.Trip
    else -> NtbPoiType.Place
}

/**
 * Offline stand-in used in DEBUG (the backend isn't reachable from the
 * emulator). Returns a couple of demo POIs near the viewport centre so the
 * overlay, info sheet and route reveal can be driven without a network.
 */
class SyntheticNasjonalTurbaseRepository @Inject constructor() : NasjonalTurbaseRepository {
    override suspend fun pois(bounds: GeoBounds): List<NtbPoi> {
        val cLat = (bounds.south + bounds.north) / 2
        val cLng = (bounds.west + bounds.east) / 2
        return listOf(
            NtbPoi(
                id = "demo-cabin",
                type = NtbPoiType.Cabin,
                title = "Demo cabin (DNT)",
                position = LatLng(cLat + 0.01, cLng + 0.01),
                summary = "A self-served DNT cabin. Synthetic debug data.",
                utUrl = "https://ut.no/hytte/demo",
            ),
            NtbPoi(
                id = "demo-trip",
                type = NtbPoiType.Trip,
                title = "Demo trip (UT.no)",
                position = LatLng(cLat - 0.01, cLng - 0.01),
                summary = "A marked trip suggestion. Synthetic debug data.",
                utUrl = "https://ut.no/turforslag/demo",
            ),
        )
    }

    override suspend fun route(id: String): NtbRoute? {
        if (id != "demo-trip") return null
        return NtbRoute(
            id = id,
            title = "Demo trip (UT.no)",
            points = listOf(
                LatLng(59.90, 10.70),
                LatLng(59.905, 10.715),
                LatLng(59.912, 10.72),
                LatLng(59.92, 10.74),
            ),
            description = "A short demo route used to drive the reveal animation offline.",
            distanceMeters = 4200.0,
            grade = "Middels",
            utUrl = "https://ut.no/turforslag/demo",
        )
    }
}
