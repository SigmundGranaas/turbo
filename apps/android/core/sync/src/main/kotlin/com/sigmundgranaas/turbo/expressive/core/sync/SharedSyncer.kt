package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import javax.inject.Inject

/**
 * Pulls the "shared with me" delta (GET /api/sharing/resources/sync?since=) and
 * mirrors it locally: live envelopes are fetched + adopted (read-only-by-convention),
 * deleted ones (grant revoked / removed server-side) are dropped. Runs as a normal
 * [DomainSyncer] under its own cursor.
 */
class SharedSyncer @Inject constructor(
    private val sharing: SharingRepository,
    private val redeemer: ShareLinkRedeemer,
) : DomainSyncer {

    override val cursorKey = "shared"

    override suspend fun sync(since: String?): String? {
        val page = (sharing.sharedResources(since) as? Outcome.Success)?.value ?: return null
        page.items.forEach { env ->
            if (env.deleted) redeemer.purge(env.id, env.type) else redeemer.adopt(env.id, env.type)
        }
        return page.serverTime
    }
}
