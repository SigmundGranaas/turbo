package com.sigmundgranaas.turbo.expressive.core.auth

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import kotlinx.coroutines.flow.StateFlow

/** Auth failures carry a user-facing message (server's, or a fallback). */
class AuthException(message: String) : Exception(message)

/**
 * Owns the session: email/password + Google sign-in, token refresh, sign-out, and
 * the observable [state]. Calls the unauthenticated /api/auth endpoints with a
 * dedicated client; protected data endpoints get a separate authenticated client
 * (added with the sync layer).
 */
interface AuthRepository {
    val state: StateFlow<AuthState>

    /** Load any persisted session into [state]; call once at startup. */
    suspend fun restore()

    suspend fun register(email: String, password: String): Outcome<Account>
    suspend fun login(email: String, password: String): Outcome<Account>

    /** The Google OAuth consent URL to open (the flow returns a code to finish with). */
    suspend fun googleAuthUrl(): Outcome<String>

    /** Finish Google sign-in with the authorization code from the OAuth flow. */
    suspend fun loginWithGoogle(code: String): Outcome<Account>

    /** Refresh the access token using the stored refresh token. */
    suspend fun refresh(): Outcome<Unit>

    suspend fun logout()

    /** Current access token for authenticated calls, or null when signed out. */
    suspend fun accessToken(): String?
}
