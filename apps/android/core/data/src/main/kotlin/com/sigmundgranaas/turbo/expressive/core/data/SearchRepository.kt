package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SearchHit
import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.parameter
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import javax.inject.Inject

/** Place search backed by the public Kartverket stedsnavn (place-name) API. */
interface SearchRepository {
    suspend fun search(query: String): Outcome<List<SearchHit>>
}

class KartverketSearchRepository @Inject constructor(
    private val client: HttpClient,
) : SearchRepository {

    override suspend fun search(query: String): Outcome<List<SearchHit>> = try {
        val response: StedsnavnResponse = client
            .get("https://api.kartverket.no/stedsnavn/v1/navn") {
                parameter("sok", "$query*")
                parameter("fuzzy", true)
                parameter("utkoordsys", 4326)
                parameter("treffPerSide", 8)
                parameter("side", 1)
            }
            .body()
        Outcome.Success(response.navn.mapNotNull(NavnDto::toHit))
    } catch (t: Throwable) {
        Outcome.Failure(t)
    }
}

@Serializable
private data class StedsnavnResponse(val navn: List<NavnDto> = emptyList())

@Serializable
private data class NavnDto(
    @SerialName("skrivemåte") val name: String? = null,
    @SerialName("navneobjekttype") val type: String? = null,
    @SerialName("kommuner") val kommuner: List<KommuneDto> = emptyList(),
    @SerialName("representasjonspunkt") val point: PunktDto? = null,
)

@Serializable
private data class KommuneDto(@SerialName("kommunenavn") val name: String? = null)

@Serializable
private data class PunktDto(
    @SerialName("nord") val nord: Double? = null,
    @SerialName("øst") val ost: Double? = null,
)

private fun NavnDto.toHit(): SearchHit? {
    val n = name ?: return null
    val lat = point?.nord ?: return null
    val lng = point?.ost ?: return null
    val desc = listOfNotNull(type, kommuner.firstOrNull()?.name).joinToString(" · ")
    return SearchHit(name = n, description = desc, position = LatLng(lat, lng))
}
