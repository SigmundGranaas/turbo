package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SearchHit
import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.parameter
import javax.inject.Inject
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Street-address search backed by the public Kartverket Adresser API — the same
 * backend the old Flutter composite search used. Direct Geonorge calls for now;
 * the long-term home is a Matrikkel ingest into the tileserver search index.
 */
interface AddressSearchRepository {
    suspend fun search(query: String): Outcome<List<SearchHit>>
}

/** Municipality search backed by the public Kartverket Kommuneinfo API. */
interface KommuneSearchRepository {
    suspend fun search(query: String): Outcome<List<SearchHit>>
}

class GeonorgeAddressRepository @Inject constructor(
    private val client: HttpClient,
) : AddressSearchRepository {

    override suspend fun search(query: String): Outcome<List<SearchHit>> = try {
        val response: AdresseResponse = client
            .get("https://ws.geonorge.no/adresser/v1/sok") {
                parameter("sok", query)
                parameter("treffPerSide", 5)
            }
            .body()
        Outcome.Success(response.adresser.mapNotNull(AdresseDto::toHit))
    } catch (t: Throwable) {
        Outcome.Failure(t)
    }
}

class GeonorgeKommuneRepository @Inject constructor(
    private val client: HttpClient,
) : KommuneSearchRepository {

    /** Two-step: `/sok` finds matching municipalities by name; the detail call
     *  supplies the centre point (`punktIOmrade`) + county the list view shows.
     *  Capped at the top [MAX_KOMMUNER] hits so a broad query costs ≤ 4 calls. */
    override suspend fun search(query: String): Outcome<List<SearchHit>> = try {
        val response: KommuneSokResponse = client
            .get("https://ws.geonorge.no/kommuneinfo/v1/sok") {
                parameter("knavn", query)
            }
            .body()
        val hits = coroutineScope {
            response.kommuner.take(MAX_KOMMUNER).map { k ->
                async {
                    val nr = k.kommunenummer ?: return@async null
                    runCatching {
                        val detail: KommuneDetailDto = client
                            .get("https://ws.geonorge.no/kommuneinfo/v1/kommuner/$nr") {
                                parameter("utkoordsys", 4258)
                            }
                            .body()
                        detail.toHit()
                    }.getOrNull()
                }
            }.awaitAll().filterNotNull()
        }
        Outcome.Success(hits)
    } catch (t: Throwable) {
        Outcome.Failure(t)
    }

    private companion object {
        const val MAX_KOMMUNER = 3
    }
}

// ── Adresser wire ─────────────────────────────────────────────────────────

@Serializable
private data class AdresseResponse(val adresser: List<AdresseDto> = emptyList())

@Serializable
private data class AdresseDto(
    val adressetekst: String? = null,
    val postnummer: String? = null,
    val poststed: String? = null,
    val representasjonspunkt: AdressePunktDto? = null,
)

@Serializable
private data class AdressePunktDto(val lat: Double? = null, val lon: Double? = null)

private fun AdresseDto.toHit(): SearchHit? {
    val text = adressetekst?.takeIf { it.isNotBlank() } ?: return null
    val lat = representasjonspunkt?.lat ?: return null
    val lon = representasjonspunkt.lon ?: return null
    val secondary = listOfNotNull(postnummer, poststed).joinToString(" ")
    return SearchHit(name = text, description = secondary, position = LatLng(lat, lon))
}

// ── Kommuneinfo wire ──────────────────────────────────────────────────────

@Serializable
private data class KommuneSokResponse(val kommuner: List<KommuneSokDto> = emptyList())

@Serializable
private data class KommuneSokDto(
    @SerialName("kommunenavnNorsk") val kommunenavnNorsk: String? = null,
    val kommunenavn: String? = null,
    val kommunenummer: String? = null,
)

@Serializable
private data class KommuneDetailDto(
    @SerialName("kommunenavnNorsk") val kommunenavnNorsk: String? = null,
    val kommunenavn: String? = null,
    val fylkesnavn: String? = null,
    val punktIOmrade: PunktIOmradeDto? = null,
)

@Serializable
private data class PunktIOmradeDto(val coordinates: List<Double> = emptyList())

private fun KommuneDetailDto.toHit(): SearchHit? {
    val name = (kommunenavnNorsk ?: kommunenavn)?.takeIf { it.isNotBlank() } ?: return null
    // GeoJSON point order: [lon, lat].
    val lon = punktIOmrade?.coordinates?.getOrNull(0) ?: return null
    val lat = punktIOmrade.coordinates.getOrNull(1) ?: return null
    return SearchHit(name = name, description = fylkesnavn.orEmpty(), position = LatLng(lat, lon))
}
