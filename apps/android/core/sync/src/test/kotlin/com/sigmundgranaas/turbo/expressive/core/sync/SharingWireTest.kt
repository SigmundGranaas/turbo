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

    @Test
    fun `parses a list of friendships`() {
        val list = json.decodeFromString<List<FriendshipDto>>(
            """[{"otherUserId":"u-2","initiatorId":"u-1","status":"pending","createdAt":"2024-01-01T00:00:00Z","acceptedAt":null},
                {"otherUserId":"u-3","initiatorId":"u-3","status":"accepted","createdAt":"2024-01-02T00:00:00Z","acceptedAt":"2024-01-03T00:00:00Z"}]""",
        )
        assertEquals(2, list.size)
        assertEquals("pending", list[0].status)
        assertEquals("u-3", list[1].otherUserId)
    }

    @Test
    fun `parses a user lookup and tolerates extra fields`() {
        val dto = json.decodeFromString<UserLookupResponse>("""{"userId":"u-9","friendCode":"ZZ99"}""")
        assertEquals("u-9", dto.userId)
    }

    @Test
    fun `parses a group with members`() {
        val dto = json.decodeFromString<GroupDto>(
            """{"id":"g-1","ownerId":"u-1","name":"Tindetur","members":[
                {"userId":"u-1","role":"owner","joinedAt":"2024-01-01T00:00:00Z"},
                {"userId":"u-2","role":"member","joinedAt":null}]}""",
        )
        assertEquals("Tindetur", dto.name)
        assertEquals(2, dto.members.size)
        assertEquals("owner", dto.members[0].role)
    }

    @Test
    fun `serializes friendship + group requests`() {
        assertTrue(json.encodeToString(FriendshipActionRequest("u-2")).contains("\"otherUserId\":\"u-2\""))
        assertTrue(json.encodeToString(CreateGroupRequest("Tindetur")).contains("\"name\":\"Tindetur\""))
        assertTrue(json.encodeToString(GroupMemberRequest("u-2")).contains("\"userId\":\"u-2\""))
    }
}
