package com.sigmundgranaas.turbo.expressive.core.auth

import kotlinx.serialization.Serializable

/** POST /api/auth/auth/register */
@Serializable
internal data class RegisterRequest(val email: String, val password: String, val confirmPassword: String)

/** POST /api/auth/auth/login */
@Serializable
internal data class LoginRequest(val email: String, val password: String)

/** POST /api/auth/token/refresh and /token/revoke */
@Serializable
internal data class RefreshRequest(val refreshToken: String)

/** POST /api/auth/oauth/mobile-signin */
@Serializable
internal data class MobileSignInRequest(val provider: String, val code: String, val state: String? = null)

/** Shared response for register / login / refresh / mobile-signin. */
@Serializable
internal data class AuthResponse(
    val accessToken: String,
    val refreshToken: String,
    val accountId: String,
    val email: String,
)

/** GET /api/auth/oauth/{provider}/url */
@Serializable
internal data class OAuthUrlResponse(val authorizationUrl: String)

/** GET /api/auth/session/me — the server's view of the signed-in session. */
@Serializable
internal data class SessionMeResponse(
    val accountId: String = "",
    val email: String = "",
    val isActive: Boolean = true,
)

/** Error envelope: { errorCode, message }. */
@Serializable
internal data class ApiError(val errorCode: String? = null, val message: String? = null)
