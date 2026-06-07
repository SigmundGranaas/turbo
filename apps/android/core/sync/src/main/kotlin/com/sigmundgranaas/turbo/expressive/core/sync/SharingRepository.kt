package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.auth.AuthConfig
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import io.ktor.client.call.body
import io.ktor.client.request.parameter
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.HttpMethod
import io.ktor.http.contentType
import io.ktor.http.isSuccess
import javax.inject.Inject

/** A redeemed share link: which resource the current user just gained access to, and the role. */
data class LinkRedemption(val resourceId: String, val resourceType: String, val role: String)

/**
 * Authenticated access to the backend Sharing service. Reuses the sync layer's
 * [AuthorizedHttp] (token + refresh-on-401). Web base for shareable URLs differs
 * from the API base.
 */
interface SharingRepository {
    /** The user's shareable friend code (e.g. "turbo-AB12CD"). */
    suspend fun friendCode(): Outcome<String>

    /** Create a share link for a resource; returns a shareable URL containing the link token. */
    suspend fun createLink(resourceId: String, role: String = ROLE_VIEWER): Outcome<String>

    /** Redeem a share-link token, granting the current user access to the resource. */
    suspend fun redeemLink(token: String): Outcome<LinkRedemption>

    /** Delta of resources shared with the current user (since the last cursor). */
    suspend fun sharedResources(since: String?): Outcome<ResourceSyncPageDto>

    // ── Friends + groups (default to "unsupported" so test fakes need not override) ──
    suspend fun friendships(): Outcome<List<FriendshipDto>> = unsupported()
    suspend fun requestFriendship(otherUserId: String): Outcome<Unit> = unsupported()
    suspend fun acceptFriendship(otherUserId: String): Outcome<Unit> = unsupported()
    suspend fun removeFriendship(otherUserId: String): Outcome<Unit> = unsupported()
    /** Resolve a friend code (with or without the "turbo-" prefix) to a user id. */
    suspend fun lookupUser(code: String): Outcome<String> = unsupported()
    suspend fun groups(): Outcome<List<GroupDto>> = unsupported()
    suspend fun createGroup(name: String): Outcome<GroupDto> = unsupported()
    suspend fun addGroupMember(groupId: String, userId: String): Outcome<Unit> = unsupported()
    suspend fun removeGroupMember(groupId: String, userId: String): Outcome<Unit> = unsupported()

    companion object {
        const val ROLE_VIEWER = "viewer"
        private fun <T> unsupported(): Outcome<T> = Outcome.Failure(UnsupportedOperationException())
    }
}

class KtorSharingRepository @Inject constructor(
    private val http: AuthorizedHttp,
) : SharingRepository {
    private val base = "${AuthConfig.BASE_URL}/api/sharing"

    override suspend fun friendCode(): Outcome<String> = runCatching {
        val resp = http.request("$base/me/profile") { method = HttpMethod.Get }
        check(resp.status.isSuccess()) { "profile ${resp.status}" }
        val dto: UserProfileDto = resp.body()
        "$FRIEND_CODE_PREFIX${dto.friendCode}"
    }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    override suspend fun createLink(resourceId: String, role: String): Outcome<String> = runCatching {
        val resp = http.request("$base/grants/links") {
            method = HttpMethod.Post
            contentType(ContentType.Application.Json)
            setBody(GrantAsLinkRequest(resourceId = resourceId, role = role))
        }
        check(resp.status.isSuccess()) { "createLink ${resp.status}" }
        val dto: LinkGrantDto = resp.body()
        "$WEB_BASE_URL/link/${dto.linkToken}"
    }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    override suspend fun redeemLink(token: String): Outcome<LinkRedemption> = runCatching {
        val resp = http.request("$base/grants/links/$token/redeem") { method = HttpMethod.Post }
        check(resp.status.isSuccess()) { "redeem ${resp.status}" }
        val dto: LinkRedemptionDto = resp.body()
        LinkRedemption(dto.resourceId, dto.resourceType, dto.role)
    }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    override suspend fun sharedResources(since: String?): Outcome<ResourceSyncPageDto> = runCatching {
        val resp = http.request("$base/resources/sync") {
            method = HttpMethod.Get
            if (!since.isNullOrBlank()) parameter("since", since)
        }
        check(resp.status.isSuccess()) { "resources/sync ${resp.status}" }
        resp.body<ResourceSyncPageDto>()
    }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    override suspend fun friendships(): Outcome<List<FriendshipDto>> = runCatching {
        val resp = http.request("$base/friendships") { method = HttpMethod.Get }
        check(resp.status.isSuccess()) { "friendships ${resp.status}" }
        resp.body<List<FriendshipDto>>()
    }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    override suspend fun requestFriendship(otherUserId: String): Outcome<Unit> =
        friendshipAction("request", otherUserId)

    override suspend fun acceptFriendship(otherUserId: String): Outcome<Unit> =
        friendshipAction("accept", otherUserId)

    private suspend fun friendshipAction(action: String, otherUserId: String): Outcome<Unit> = runCatching {
        val resp = http.request("$base/friendships/$action") {
            method = HttpMethod.Post
            contentType(ContentType.Application.Json)
            setBody(FriendshipActionRequest(otherUserId))
        }
        check(resp.status.isSuccess()) { "friendships/$action ${resp.status}" }
        Unit
    }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    override suspend fun removeFriendship(otherUserId: String): Outcome<Unit> = runCatching {
        val resp = http.request("$base/friendships/$otherUserId") { method = HttpMethod.Delete }
        check(resp.status.isSuccess()) { "friendships delete ${resp.status}" }
        Unit
    }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    override suspend fun lookupUser(code: String): Outcome<String> = runCatching {
        val bare = code.trim().removePrefix(FRIEND_CODE_PREFIX)
        val resp = http.request("$base/users/lookup") {
            method = HttpMethod.Get
            parameter("code", bare)
        }
        check(resp.status.isSuccess()) { "users/lookup ${resp.status}" }
        resp.body<UserLookupResponse>().userId
    }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    override suspend fun groups(): Outcome<List<GroupDto>> = runCatching {
        val resp = http.request("$base/groups") { method = HttpMethod.Get }
        check(resp.status.isSuccess()) { "groups ${resp.status}" }
        resp.body<List<GroupDto>>()
    }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    override suspend fun createGroup(name: String): Outcome<GroupDto> = runCatching {
        val resp = http.request("$base/groups") {
            method = HttpMethod.Post
            contentType(ContentType.Application.Json)
            setBody(CreateGroupRequest(name))
        }
        check(resp.status.isSuccess()) { "createGroup ${resp.status}" }
        resp.body<GroupDto>()
    }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    override suspend fun addGroupMember(groupId: String, userId: String): Outcome<Unit> = runCatching {
        val resp = http.request("$base/groups/$groupId/members") {
            method = HttpMethod.Post
            contentType(ContentType.Application.Json)
            setBody(GroupMemberRequest(userId))
        }
        check(resp.status.isSuccess()) { "addGroupMember ${resp.status}" }
        Unit
    }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    override suspend fun removeGroupMember(groupId: String, userId: String): Outcome<Unit> = runCatching {
        val resp = http.request("$base/groups/$groupId/members/$userId") { method = HttpMethod.Delete }
        check(resp.status.isSuccess()) { "removeGroupMember ${resp.status}" }
        Unit
    }.fold({ Outcome.Success(it) }, { Outcome.Failure(it) })

    private companion object {
        const val FRIEND_CODE_PREFIX = "turbo-"
        // Where shared links resolve (the web app / App Link host). Provisional path until P4 deep-link wiring.
        const val WEB_BASE_URL = "https://kart.sandring.no"
    }
}
