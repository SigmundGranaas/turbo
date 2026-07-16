package com.sigmundgranaas.turbo.expressive.core.auth

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import io.ktor.client.HttpClient
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpStatusCode
import io.ktor.http.headersOf
import io.ktor.serialization.kotlinx.json.json
import kotlinx.coroutines.async
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** In-memory [AuthTokenStore] so the repository's persistence + signed-in flips are observable. */
private class FakeTokenStore(seed: Pair<AuthTokens, Account>? = null) : AuthTokenStore {
    private var saved: Pair<AuthTokens, Account>? = seed
    var cleared = false
        private set

    override suspend fun save(tokens: AuthTokens, account: Account) { saved = tokens to account }
    override suspend fun tokens(): AuthTokens? = saved?.first
    override suspend fun account(): Account? = saved?.second
    override suspend fun clear() { saved = null; cleared = true }
}

/**
 * Drives [KtorAuthRepository] against a mock Ktor engine (configured exactly like the
 * real @AuthClient — JSON, no expectSuccess) and an in-memory token store, so the
 * session logic — persist-on-success, fail-without-saving, signed-in/out transitions —
 * is covered without a network or Android Keystore.
 */
class KtorAuthRepositoryTest {

    private val authBody =
        """{"accessToken":"acc-jwt","refreshToken":"ref-opaque","accountId":"a-1","email":"hiker@x.no"}"""

    private fun client(status: HttpStatusCode, body: String) = HttpClient(
        MockEngine { respond(body, status, headersOf(HttpHeaders.ContentType, "application/json")) },
    ) {
        expectSuccess = false
        install(ContentNegotiation) { json(Json { ignoreUnknownKeys = true }) }
    }

    @Test
    fun `login success persists tokens and flips state to signed-in`() = runTest {
        val store = FakeTokenStore()
        val repo = KtorAuthRepository(client(HttpStatusCode.OK, authBody), store)

        val outcome = repo.login("hiker@x.no", "pw")

        assertTrue(outcome is Outcome.Success)
        assertEquals("hiker@x.no", (outcome as Outcome.Success).value.email)
        assertEquals("acc-jwt", store.tokens()?.accessToken)
        assertEquals("ref-opaque", store.tokens()?.refreshToken)
        assertEquals(AuthState.SignedIn(Account("a-1", "hiker@x.no")), repo.state.value)
    }

    @Test
    fun `login failure surfaces the server message and saves nothing`() = runTest {
        val store = FakeTokenStore()
        val repo = KtorAuthRepository(client(HttpStatusCode.Unauthorized, """{"message":"Invalid credentials"}"""), store)

        val outcome = repo.login("hiker@x.no", "wrong")

        assertTrue(outcome is Outcome.Failure)
        assertEquals("Invalid credentials", (outcome as Outcome.Failure).error.message)
        assertNull(store.tokens())
        assertEquals(AuthState.Unknown, repo.state.value) // never flipped to signed-in
    }

    @Test
    fun `register success signs in the new account`() = runTest {
        val store = FakeTokenStore()
        val repo = KtorAuthRepository(client(HttpStatusCode.OK, authBody), store)

        val outcome = repo.register("hiker@x.no", "pw")

        assertTrue(outcome is Outcome.Success)
        assertEquals(AuthState.SignedIn(Account("a-1", "hiker@x.no")), repo.state.value)
    }

    @Test
    fun `restore reflects the persisted session`() = runTest {
        val signedIn = KtorAuthRepository(
            client(HttpStatusCode.OK, authBody),
            FakeTokenStore(AuthTokens("a", "r") to Account("a-1", "hiker@x.no")),
        )
        signedIn.restore()
        assertEquals(AuthState.SignedIn(Account("a-1", "hiker@x.no")), signedIn.state.value)

        val fresh = KtorAuthRepository(client(HttpStatusCode.OK, authBody), FakeTokenStore())
        fresh.restore()
        assertEquals(AuthState.SignedOut, fresh.state.value)
    }

    @Test
    fun `logout clears the store and signs out`() = runTest {
        val store = FakeTokenStore(AuthTokens("a", "r") to Account("a-1", "hiker@x.no"))
        val repo = KtorAuthRepository(client(HttpStatusCode.OK, "{}"), store)

        repo.logout()

        assertTrue(store.cleared)
        assertNull(store.tokens())
        assertEquals(AuthState.SignedOut, repo.state.value)
    }

    @Test
    fun `refresh without a stored token fails fast`() = runTest {
        val repo = KtorAuthRepository(client(HttpStatusCode.OK, authBody), FakeTokenStore())
        val outcome = repo.refresh()
        assertTrue(outcome is Outcome.Failure)
        assertEquals("Not signed in", (outcome as Outcome.Failure).error.message)
    }

    @Test
    fun `a server-rejected refresh token signs the user out`() = runTest {
        val store = FakeTokenStore(AuthTokens("a", "dead-refresh") to Account("a-1", "hiker@x.no"))
        val repo = KtorAuthRepository(client(HttpStatusCode.Unauthorized, """{"message":"revoked"}"""), store)
        repo.restore()

        val outcome = repo.refresh()

        assertTrue(outcome is Outcome.Failure)
        assertTrue(store.cleared)
        assertEquals(AuthState.SignedOut, repo.state.value) // stuck-signed-in bug: fixed
    }

    @Test
    fun `a transport failure during refresh keeps the session`() = runTest {
        val store = FakeTokenStore(AuthTokens("a", "r") to Account("a-1", "hiker@x.no"))
        val offline = HttpClient(MockEngine { throw java.io.IOException("offline") }) {
            expectSuccess = false
            install(ContentNegotiation) { json(Json { ignoreUnknownKeys = true }) }
        }
        val repo = KtorAuthRepository(offline, store)
        repo.restore()

        val outcome = repo.refresh()

        assertTrue(outcome is Outcome.Failure)
        assertTrue(!store.cleared) // offline must NOT log the user out
        assertEquals(AuthState.SignedIn(Account("a-1", "hiker@x.no")), repo.state.value)
    }

    @Test
    fun `concurrent refreshes share one network call`() = runTest {
        var calls = 0
        val store = FakeTokenStore(AuthTokens("stale", "r-1") to Account("a-1", "hiker@x.no"))
        val engine = MockEngine {
            calls++
            kotlinx.coroutines.delay(50) // hold the first refresh so the second queues on the mutex
            respond(authBody, HttpStatusCode.OK, headersOf(HttpHeaders.ContentType, "application/json"))
        }
        val repo = KtorAuthRepository(
            HttpClient(engine) {
                expectSuccess = false
                install(ContentNegotiation) { json(Json { ignoreUnknownKeys = true }) }
            },
            store,
        )

        val a = async { repo.refresh() }
        val b = async { repo.refresh() }
        assertTrue(a.await() is Outcome.Success)
        assertTrue(b.await() is Outcome.Success)

        // Refresh tokens rotate: a second concurrent refresh with the consumed
        // token would have killed the session — it must reuse the first result.
        assertEquals(1, calls)
        assertEquals("acc-jwt", store.tokens()?.accessToken)
    }

    @Test
    fun `a refresh response without account fields keeps the persisted identity`() = runTest {
        val store = FakeTokenStore(AuthTokens("a", "r") to Account("a-1", "hiker@x.no"))
        val tokensOnly = """{"accessToken":"new-acc","refreshToken":"new-ref","accountId":"","email":""}"""
        val repo = KtorAuthRepository(client(HttpStatusCode.OK, tokensOnly), store)
        repo.restore()

        assertTrue(repo.refresh() is Outcome.Success)

        assertEquals("new-acc", store.tokens()?.accessToken)
        assertEquals(Account("a-1", "hiker@x.no"), store.account()) // not blanked
        assertEquals(AuthState.SignedIn(Account("a-1", "hiker@x.no")), repo.state.value)
    }

    /** Routes /session/me and /token/refresh separately so validation flows are drivable. */
    private fun routedClient(
        me: () -> Pair<HttpStatusCode, String>,
        refresh: () -> Pair<HttpStatusCode, String> = { HttpStatusCode.OK to authBody },
    ) = HttpClient(
        MockEngine { request ->
            val (status, body) = when {
                request.url.encodedPath.endsWith("/session/me") -> me()
                request.url.encodedPath.endsWith("/token/refresh") -> refresh()
                else -> HttpStatusCode.NotFound to "{}"
            }
            respond(body, status, headersOf(HttpHeaders.ContentType, "application/json"))
        },
    ) {
        expectSuccess = false
        install(ContentNegotiation) { json(Json { ignoreUnknownKeys = true }) }
    }

    @Test
    fun `validateSession signs out when the server refuses the session after a refresh`() = runTest {
        val store = FakeTokenStore(AuthTokens("a", "dead") to Account("a-1", "hiker@x.no"))
        val repo = KtorAuthRepository(
            routedClient(
                me = { HttpStatusCode.Unauthorized to "{}" },
                refresh = { HttpStatusCode.Unauthorized to """{"message":"revoked"}""" },
            ),
            store,
        )
        repo.restore()

        repo.validateSession()

        assertEquals(AuthState.SignedOut, repo.state.value)
        assertTrue(store.cleared)
    }

    @Test
    fun `validateSession re-syncs a changed email`() = runTest {
        val store = FakeTokenStore(AuthTokens("a", "r") to Account("a-1", "old@x.no"))
        val repo = KtorAuthRepository(
            routedClient(me = { HttpStatusCode.OK to """{"accountId":"a-1","email":"new@x.no","isActive":true}""" }),
            store,
        )
        repo.restore()

        repo.validateSession()

        assertEquals(AuthState.SignedIn(Account("a-1", "new@x.no")), repo.state.value)
        assertEquals("new@x.no", store.account()?.email)
    }

    @Test
    fun `validateSession keeps the session on transport errors`() = runTest {
        val store = FakeTokenStore(AuthTokens("a", "r") to Account("a-1", "hiker@x.no"))
        val offline = HttpClient(MockEngine { throw java.io.IOException("offline") }) {
            expectSuccess = false
            install(ContentNegotiation) { json(Json { ignoreUnknownKeys = true }) }
        }
        val repo = KtorAuthRepository(offline, store)
        repo.restore()

        repo.validateSession()

        assertEquals(AuthState.SignedIn(Account("a-1", "hiker@x.no")), repo.state.value)
        assertTrue(!store.cleared)
    }

    @Test
    fun `accessToken returns the stored access token`() = runTest {
        val repo = KtorAuthRepository(
            client(HttpStatusCode.OK, authBody),
            FakeTokenStore(AuthTokens("acc-jwt", "ref") to Account("a-1", "hiker@x.no")),
        )
        assertEquals("acc-jwt", repo.accessToken())
    }

    @Test
    fun `googleAuthUrl returns the consent url`() = runTest {
        val repo = KtorAuthRepository(
            client(HttpStatusCode.OK, """{"authorizationUrl":"https://accounts.google.com/o/oauth2/v2/auth?x=1"}"""),
            FakeTokenStore(),
        )
        val outcome = repo.googleAuthUrl()
        assertTrue(outcome is Outcome.Success)
        assertEquals("https://accounts.google.com/o/oauth2/v2/auth?x=1", (outcome as Outcome.Success).value)
    }
}
