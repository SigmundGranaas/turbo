package com.sigmundgranaas.turbo.expressive.core.auth

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import io.ktor.http.isSuccess
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
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
        val response = client.get("$base/api/auth/oauth/google/url")
        if (!response.status.isSuccess()) throw AuthException("Couldn't start Google sign-in (${response.status.value})")
        response.body<OAuthUrlResponse>().authorizationUrl
    }.fold(
        onSuccess = { Outcome.Success(it) },
        onFailure = { Outcome.Failure(if (it is AuthException) it else AuthException(it.message ?: "Network error")) },
    )

    override suspend fun loginWithGoogle(code: String): Outcome<Account> =
        authCall("$base/api/auth/oauth/mobile-signin", MobileSignInRequest(provider = "Google", code = code))

    override suspend fun refresh(): Outcome<Unit> {
        val refresh = store.tokens()?.refreshToken ?: return Outcome.Failure(AuthException("Not signed in"))
        return when (val r = authCall("$base/api/auth/token/refresh", RefreshRequest(refresh))) {
            is Outcome.Success -> Outcome.Success(Unit)
            is Outcome.Failure -> r
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
