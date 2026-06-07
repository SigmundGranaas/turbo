package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.auth.AuthConfig
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import io.ktor.client.call.body
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

    companion object {
        const val ROLE_VIEWER = "viewer"
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

    private companion object {
        const val FRIEND_CODE_PREFIX = "turbo-"
        // Where shared links resolve (the web app / App Link host). Provisional path until P4 deep-link wiring.
        const val WEB_BASE_URL = "https://kart.sandring.no"
    }
}
