package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.auth.AuthRepository
import io.ktor.client.HttpClient
import io.ktor.client.request.HttpRequestBuilder
import io.ktor.client.request.header
import io.ktor.client.request.request
import io.ktor.client.statement.HttpResponse
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpStatusCode
import javax.inject.Inject
import javax.inject.Qualifier

/** Marks the HttpClient for authenticated app-data (sync) calls — distinct from the public map clients. */
@Qualifier
@Retention(AnnotationRetention.RUNTIME)
annotation class SyncClient

/**
 * Issues requests with the current access token attached, transparently refreshing
 * it once on a 401 and retrying. One refresh attempt per call — a second 401 means
 * the refresh token is dead and the response (still 401) propagates so the caller
 * can surface it.
 */
class AuthorizedHttp @Inject constructor(
    @param:SyncClient private val client: HttpClient,
    private val auth: AuthRepository,
) {
    /**
     * [urlString] must be the absolute URL (passed positionally so Ktor parses scheme+host,
     * exactly like the auth client's `get("$base/…")`); setting it inside the builder block
     * leaves the host as the default localhost.
     */
    suspend fun request(urlString: String, build: HttpRequestBuilder.() -> Unit): HttpResponse {
        val first = client.request(urlString) {
            build()
            bearer(auth.accessToken())
        }
        if (first.status != HttpStatusCode.Unauthorized) return first

        auth.refresh()
        return client.request(urlString) {
            build()
            bearer(auth.accessToken())
        }
    }

    private fun HttpRequestBuilder.bearer(token: String?) {
        if (token != null) header(HttpHeaders.Authorization, "Bearer $token")
    }
}
