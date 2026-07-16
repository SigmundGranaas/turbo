package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import javax.inject.Inject
import kotlinx.serialization.Serializable

/**
 * Per-vertex terrain elevations from the tileserver's DEM (`POST /v1/elev/samples`)
 * — the elevation-backfill primitive. Self-hosted (the same DEM the 3D terrain
 * renders), so no external rate limits; one round-trip per ≤[MAX_POINTS] chunk.
 */
interface ElevationRepository {
    /** Elevation (m) per input point, in order; null entries where the DEM has no
     *  coverage. Fails as an [Outcome.Failure] on transport errors. */
    suspend fun sample(points: List<LatLng>): Outcome<List<Double?>>

    companion object {
        /** The server caps a request at 4096 points; chunk above this. */
        const val MAX_POINTS = 4096
    }
}

class TileserverElevationRepository @Inject constructor(
    private val client: HttpClient,
) : ElevationRepository {

    override suspend fun sample(points: List<LatLng>): Outcome<List<Double?>> = try {
        val out = ArrayList<Double?>(points.size)
        points.chunked(ElevationRepository.MAX_POINTS).forEach { chunk ->
            val resp: SamplesResponse = client.post("$BASE/v1/elev/samples") {
                contentType(ContentType.Application.Json)
                setBody(SamplesRequest(points = chunk.map { listOf(it.lng, it.lat) }))
            }.body()
            out += resp.elevM.map { it?.toDouble() }
        }
        Outcome.Success(out)
    } catch (t: Throwable) {
        Outcome.Failure(t)
    }

    private companion object {
        const val BASE = "https://kart-api.sandring.no"
    }
}

@Serializable
private data class SamplesRequest(val points: List<List<Double>>)

@Serializable
private data class SamplesResponse(
    @kotlinx.serialization.SerialName("elev_m") val elevM: List<Double?> = emptyList(),
)
