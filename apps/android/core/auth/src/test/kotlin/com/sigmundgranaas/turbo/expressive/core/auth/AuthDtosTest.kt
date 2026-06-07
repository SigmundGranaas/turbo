package com.sigmundgranaas.turbo.expressive.core.auth

import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Pins the auth wire contract to what the backend actually returns
 * ({ accessToken, refreshToken, accountId, email }) and tolerates extra fields,
 * so a server response shape change is caught here rather than at runtime.
 */
class AuthDtosTest {
    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun `parses the shared auth response`() {
        val body = """
            {
              "accessToken": "jwt-access",
              "refreshToken": "opaque-refresh",
              "accountId": "11111111-2222-3333-4444-555555555555",
              "email": "hiker@example.com",
              "extraServerField": "ignored"
            }
        """.trimIndent()

        val parsed = json.decodeFromString<AuthResponse>(body)

        assertEquals("jwt-access", parsed.accessToken)
        assertEquals("opaque-refresh", parsed.refreshToken)
        assertEquals("11111111-2222-3333-4444-555555555555", parsed.accountId)
        assertEquals("hiker@example.com", parsed.email)
    }

    @Test
    fun `serializes login + mobile-signin requests with the field names the API expects`() {
        assertEquals(
            """{"email":"a@b.no","password":"pw"}""",
            json.encodeToString(LoginRequest("a@b.no", "pw")),
        )
        assertEquals(
            """{"provider":"Google","code":"auth-code"}""",
            json.encodeToString(MobileSignInRequest(provider = "Google", code = "auth-code")),
        )
    }
}
