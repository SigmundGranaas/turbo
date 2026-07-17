package com.sigmundgranaas.turbo.expressive.core.auth

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.header
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpStatusCode
import io.ktor.http.contentType
import io.ktor.http.isSuccess
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import javax.inject.Inject

class KtorAuthRepository @Inject constructor(
    @param:AuthClient private val client: HttpClient,
    private val store: AuthTokenStore,
) : AuthRepository {

    private val _state = MutableStateFlow<AuthState>(AuthState.Unknown)
    override val state: StateFlow<AuthState> = _state.asStateFlow()

    private val base = AuthConfig.BASE_URL

    override suspend fun restore() {
        _state.value = store.account()?.let { AuthState.SignedIn(it) } ?: AuthState.SignedOut
    }

    override suspend fun register(email: String, password: String): Outcome<Account> =
        authCall("$base/api/auth/auth/register", RegisterRequest(email, password, password))

    override suspend fun login(email: String, password: String): Outcome<Account> =
        authCall("$base/api/auth/auth/login", LoginRequest(email, password))

    override suspend fun googleAuthUrl(): Outcome<String> = runCatching {
        // mobile=true → the server hands back a consent URL whose redirect_uri is the
        // mobile-callback hop, which bounces the code into the app via turbo://oauth.
        // Without it the server bakes in the WEB callback and login dead-ends on the
        // web frontend (never returns to the app).
        val response = client.get("$base/api/auth/oauth/google/url?mobile=true")
        if (!response.status.isSuccess()) throw AuthException("Couldn't start Google sign-in (${response.status.value})")
        response.body<OAuthUrlResponse>().authorizationUrl
    }.fold(
        onSuccess = { Outcome.Success(it) },
        onFailure = { Outcome.Failure(if (it is AuthException) it else AuthException(it.message ?: "Network error")) },
    )

    override suspend fun loginWithGoogle(code: String): Outcome<Account> =
        authCall("$base/api/auth/oauth/mobile-signin", MobileSignInRequest(provider = "Google", code = code))

    private val refreshMutex = Mutex()

    override suspend fun refresh(): Outcome<Unit> {
        // Snapshot BEFORE the lock: if another caller rotates the tokens while
        // we wait, ours was a stale 401 and their fresh token is the answer.
        val staleAccess = store.tokens()?.accessToken
            ?: return Outcome.Failure(AuthException("Not signed in"))
        refreshMutex.withLock {
            val current = store.tokens() ?: return Outcome.Failure(AuthException("Not signed in"))
            // Single-flight: a concurrent refresh already succeeded — reuse it.
            // Refresh tokens rotate server-side, so a second refresh with the
            // now-consumed token would kill the session the first one just saved.
            if (current.accessToken != staleAccess) return Outcome.Success(Unit)

            val response = runCatching {
                client.post("$base/api/auth/token/refresh") {
                    contentType(ContentType.Application.Json)
                    setBody(RefreshRequest(current.refreshToken))
                }
            }.getOrElse {
                // Transport error (offline, timeout): the session may still be
                // fine — keep it and let the caller surface the sync failure.
                return Outcome.Failure(AuthException(it.message ?: "Network error"))
            }

            if (!response.status.isSuccess()) {
                // The SERVER rejected the refresh token — revoked or expired.
                // The session is dead: every authed call will 401 forever, so
                // force sign-out (mirrors the old app's auth-failure → logout).
                store.clear()
                _state.value = AuthState.SignedOut
                return Outcome.Failure(AuthException("Session expired — please sign in again"))
            }

            return runCatching {
                val auth = response.body<AuthResponse>()
                // The refresh response may omit the account fields — keep the
                // persisted identity rather than overwriting it with blanks.
                val persisted = store.account()
                val account = Account(
                    id = auth.accountId.ifBlank { persisted?.id.orEmpty() },
                    email = auth.email.ifBlank { persisted?.email.orEmpty() },
                )
                store.save(AuthTokens(auth.accessToken, auth.refreshToken), account)
                _state.value = AuthState.SignedIn(account)
                Outcome.Success(Unit)
            }.getOrElse { Outcome.Failure(AuthException(it.message ?: "Malformed refresh response")) }
        }
    }

    override suspend fun validateSession() {
        if (_state.value !is AuthState.SignedIn) return
        runCatching {
            var resp = me() ?: return
            if (resp.status == HttpStatusCode.Unauthorized) {
                // Access token stale — refresh once (a rejected refresh signs out
                // inside refresh()) and re-ask.
                if (refresh() is Outcome.Failure) return
                resp = me() ?: return
            }
            when {
                resp.status == HttpStatusCode.Unauthorized || resp.status == HttpStatusCode.Forbidden -> {
                    // Fresh tokens and the server still refuses the session
                    // (account disabled/deleted) — sign out.
                    store.clear()
                    _state.value = AuthState.SignedOut
                }
                resp.status.isSuccess() -> {
                    val me = resp.body<SessionMeResponse>()
                    val persisted = store.account() ?: return
                    if (me.email.isNotBlank() && me.email != persisted.email) {
                        val updated = persisted.copy(email = me.email)
                        store.tokens()?.let { store.save(it, updated) }
                        _state.value = AuthState.SignedIn(updated)
                    }
                }
                // Other statuses (5xx…): server trouble, not a verdict on the
                // session — keep the persisted state.
            }
        } // Transport errors: offline start keeps the session; swallow.
    }

    private suspend fun me(): io.ktor.client.statement.HttpResponse? {
        val token = store.tokens()?.accessToken ?: return null
        return client.get("$base/api/auth/session/me") {
            header(HttpHeaders.Authorization, "Bearer $token")
        }
    }

    override suspend fun logout() {
        runCatching {
            store.tokens()?.refreshToken?.let { token ->
                client.post("$base/api/auth/token/revoke") {
                    contentType(ContentType.Application.Json)
                    setBody(RefreshRequest(token))
                }
            }
        }
        store.clear()
        _state.value = AuthState.SignedOut
    }

    override suspend fun accessToken(): String? = store.tokens()?.accessToken

    /** POST [body], persist the returned tokens + account, and flip [state] to signed-in. */
    private suspend fun authCall(url: String, body: Any): Outcome<Account> = runCatching {
        val response = client.post(url) {
            contentType(ContentType.Application.Json)
            setBody(body)
        }
        if (!response.status.isSuccess()) {
            val err = runCatching { response.body<ApiError>() }.getOrNull()
            throw AuthException(err?.message ?: "Sign-in failed (${response.status.value})")
        }
        val auth = response.body<AuthResponse>()
        val account = Account(auth.accountId, auth.email)
        store.save(AuthTokens(auth.accessToken, auth.refreshToken), account)
        _state.value = AuthState.SignedIn(account)
        account
    }.fold(
        onSuccess = { Outcome.Success(it) },
        onFailure = { Outcome.Failure(if (it is AuthException) it else AuthException(it.message ?: "Network error")) },
    )
}
