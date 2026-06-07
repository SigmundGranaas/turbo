package com.sigmundgranaas.turbo.expressive.core.auth

import javax.inject.Qualifier

/** Marks the HttpClient that talks to the Turbo auth/app API (vs. the public map clients). */
@Qualifier
@Retention(AnnotationRetention.RUNTIME)
annotation class AuthClient

/** The signed-in user. */
data class Account(val id: String, val email: String)

/** JWT pair from the auth backend: short-lived access + long-lived refresh. */
data class AuthTokens(val accessToken: String, val refreshToken: String)

/**
 * The app's authentication state. [Unknown] until the persisted session is
 * restored at startup, then [SignedIn] or [SignedOut].
 */
sealed interface AuthState {
    data object Unknown : AuthState
    data object SignedOut : AuthState
    data class SignedIn(val account: Account) : AuthState
}

/** Where the Turbo app API lives. The routing/MET/Kartverket clients stay separate + public. */
object AuthConfig {
    const val BASE_URL = "https://kart-api.sandring.no"
}
