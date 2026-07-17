package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import com.sigmundgranaas.turbo.expressive.domain.RouteStreamEvent
import io.ktor.client.HttpClient
import io.ktor.client.request.accept
import io.ktor.client.request.preparePost
import io.ktor.client.request.setBody
import io.ktor.client.statement.bodyAsChannel
import io.ktor.http.ContentType
import io.ktor.http.contentType
import io.ktor.utils.io.readUTF8Line
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import javax.inject.Inject

/**
 * Turbo pathfinder client. [planStream] streams the solver's best-path snapshots
 * (progress) and then the final result (or failure) from the public, SSE
 * endpoint — so the UI can animate the route as it solves.
 */
interface RouteRepository {
    fun planStream(
        points: List<LatLng>,
        preset: RoutePreset = RoutePreset.Balanced,
        profile: String = "foot",
        roundTrip: Boolean = false,
    ): Flow<RouteStreamEvent>
}

class HttpRouteRepository @Inject constructor(
    private val client: HttpClient,
) : RouteRepository {

    override fun planStream(
        points: List<LatLng>,
        preset: RoutePreset,
        profile: String,
        roundTrip: Boolean,
    ): Flow<RouteStreamEvent> = flow {
        client.preparePost("$BASE_URL/plan/stream") {
            contentType(ContentType.Application.Json)
            accept(ContentType.Text.EventStream)
            setBody(RouteSse.encodeRequest(points, preset, profile, roundTrip))
        }.execute { response ->
            val channel = response.bodyAsChannel()
            var event: String? = null
            while (!channel.isClosedForRead) {
                val line = channel.readUTF8Line() ?: break
                when {
                    line.isEmpty() -> event = null
                    line.startsWith("event:") -> event = line.removePrefix("event:").trim()
                    line.startsWith("data:") -> RouteSse.parse(event, line.removePrefix("data:").trim())?.let { emit(it) }
                }
            }
        }
    }.flowOn(Dispatchers.IO)

    private companion object {
        const val BASE_URL = "https://kart-api.sandring.no/api/route"
    }
}
