package com.sigmundgranaas.turbo.expressive.core.sync

import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Pins the sharing wire contract against the .NET Sharing service DTOs. */
class SharingWireTest {

    private val json = Json { ignoreUnknownKeys = true; explicitNulls = false; encodeDefaults = true }

    @Test
    fun `parses a user profile`() {
        val dto = json.decodeFromString<UserProfileDto>(
            """{"userId":"u-1","friendCode":"AB12CD","createdAt":"2024-01-01T00:00:00Z"}""",
        )
        assertEquals("AB12CD", dto.friendCode)
        assertEquals("u-1", dto.userId)
    }

    @Test
    fun `parses a link grant`() {
        val dto = json.decodeFromString<LinkGrantDto>(
            """{"resourceId":"r-1","subjectId":"s-1","linkToken":"tok-xyz","role":"viewer","grantedAt":"2024-01-01T00:00:00Z","expiresAt":null}""",
        )
        assertEquals("tok-xyz", dto.linkToken)
        assertEquals("viewer", dto.role)
    }

    @Test
    fun `parses a link redemption`() {
        val dto = json.decodeFromString<LinkRedemptionDto>(
            """{"resourceId":"r-1","resourceType":"path","role":"viewer"}""",
        )
        assertEquals("path", dto.resourceType)
        assertEquals("r-1", dto.resourceId)
    }

    @Test
    fun `serializes a create-link request with defaults`() {
        val body = json.encodeToString(GrantAsLinkRequest(resourceId = "r-1"))
        assertTrue(body.contains("\"resourceId\":\"r-1\""))
        assertTrue(body.contains("\"role\":\"viewer\""))
    }
}
