package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.auth.Account
import com.sigmundgranaas.turbo.expressive.core.auth.AuthRepository
import com.sigmundgranaas.turbo.expressive.core.auth.AuthState
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import io.ktor.client.HttpClient
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.client.request.setBody
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpMethod
import io.ktor.http.headersOf
import io.ktor.utils.io.ByteReadChannel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Guards the regression where a `method`/`setBody` set inside [AuthorizedHttp.request]'s
 * config lambda was dropped — the lambda param clashed with `HttpRequestBuilder.build()`,
 * so every POST/PUT/DELETE silently degraded to a bodyless GET.
 */
class AuthorizedHttpTest {

    private class FakeAuth(private var token: String? = "tok") : AuthRepository {
        override val state: StateFlow<AuthState> = MutableStateFlow(AuthState.SignedOut)
        override suspend fun restore() = Unit
        override suspend fun register(email: String, password: String): Outcome<Account> = Outcome.Failure(RuntimeException())
        override suspend fun login(email: String, password: String): Outcome<Account> = Outcome.Failure(RuntimeException())
        override suspend fun googleAuthUrl(): Outcome<String> = Outcome.Failure(RuntimeException())
        override suspend fun loginWithGoogle(code: String): Outcome<Account> = Outcome.Failure(RuntimeException())
        override suspend fun refresh(): Outcome<Unit> = Outcome.Success(Unit)
        override suspend fun logout() = Unit
        override suspend fun accessToken(): String? = token
    }

    private fun clientRecording(record: (HttpMethod, String?, String?) -> Unit): HttpClient =
        HttpClient(
            MockEngine { req ->
                record(req.method, req.body.toString().let { (req.body as? io.ktor.http.content.TextContent)?.text }, req.headers[HttpHeaders.Authorization])
                respond(
                    content = ByteReadChannel("[]"),
                    headers = headersOf(HttpHeaders.ContentType, "application/json"),
                )
            },
        )

    @Test
    fun `method set in the config lambda is sent (POST is not degraded to GET)`() = runTest {
        var sentMethod: HttpMethod? = null
        var sentBody: String? = null
        val http = AuthorizedHttp(clientRecording { m, b, _ -> sentMethod = m; sentBody = b }, FakeAuth())

        http.request("https://example.test/api/sharing/groups") {
            method = HttpMethod.Post
            setBody("""{"name":"Tindetur"}""")
        }

        assertEquals(HttpMethod.Post, sentMethod)
        assertEquals("""{"name":"Tindetur"}""", sentBody)
    }

    @Test
    fun `the bearer token is attached`() = runTest {
        var auth: String? = null
        val http = AuthorizedHttp(
            HttpClient(
                MockEngine { req ->
                    auth = req.headers[HttpHeaders.Authorization]
                    respond(ByteReadChannel("[]"), headers = headersOf(HttpHeaders.ContentType, "application/json"))
                },
            ),
            FakeAuth(token = "abc123"),
        )
        http.request("https://example.test/api/sharing/groups") { method = HttpMethod.Get }
        assertEquals("Bearer abc123", auth)
    }
}
