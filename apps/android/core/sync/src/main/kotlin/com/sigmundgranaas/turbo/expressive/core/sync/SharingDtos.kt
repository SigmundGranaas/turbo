package com.sigmundgranaas.turbo.expressive.core.sync

import kotlinx.serialization.Serializable

/** GET /api/sharing/me/profile — the current user's stable friend code. */
@Serializable
data class UserProfileDto(
    val userId: String = "",
    val friendCode: String = "",
    val createdAt: String? = null,
)

/** POST /api/sharing/grants/links body. */
@Serializable
data class GrantAsLinkRequest(
    val resourceId: String,
    val role: String = "viewer",
    val expiresAt: String? = null,
)

/** POST /api/sharing/grants/links response — the share-link grant. */
@Serializable
data class LinkGrantDto(
    val resourceId: String = "",
    val subjectId: String = "",
    val linkToken: String = "",
    val role: String = "viewer",
    val grantedAt: String? = null,
    val expiresAt: String? = null,
)

/** POST /api/sharing/grants/links/{token}/redeem response. */
@Serializable
data class LinkRedemptionDto(
    val resourceId: String = "",
    val resourceType: String = "",
    val role: String = "",
)
