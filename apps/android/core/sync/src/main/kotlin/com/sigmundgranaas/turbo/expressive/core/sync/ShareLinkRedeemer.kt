package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionDao
import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerDao
import com.sigmundgranaas.turbo.expressive.core.data.database.PathDao
import java.util.UUID
import javax.inject.Inject

/**
 * Redeems an incoming share-link token and pulls the now-visible resource into the
 * local DB as a clean (dirty=false) row, mapping the server id to a stable local id.
 * Shared resources land read-only-by-convention — they sync down but the engine
 * won't push them back unless the user edits (a read-only flag is a follow-on).
 */
class ShareLinkRedeemer @Inject constructor(
    private val sharing: SharingRepository,
    private val tracks: TrackRemote,
    private val locations: LocationRemote,
    private val collections: CollectionRemote,
    private val pathDao: PathDao,
    private val markerDao: MarkerDao,
    private val collectionDao: CollectionDao,
) {
    /** Redeem [token]; on success the granted resource is fetched + inserted locally. */
    suspend fun redeem(token: String): Outcome<LinkRedemption> {
        val outcome = sharing.redeemLink(token)
        (outcome as? Outcome.Success)?.value?.let { adopt(it) }
        return outcome
    }

    private suspend fun adopt(redemption: LinkRedemption) {
        when (redemption.resourceType.lowercase()) {
            "path", "track" -> tracks.fetchById(redemption.resourceId)?.let { dto ->
                val localId = pathDao.byRemoteId(dto.id)?.id ?: UUID.randomUUID().toString()
                pathDao.upsert(dto.toEntity(localId))
            }
            "marker", "location" -> locations.fetchById(redemption.resourceId)?.let { dto ->
                val localId = markerDao.byRemoteId(dto.id)?.id ?: UUID.randomUUID().toString()
                markerDao.upsert(dto.toEntity(localId))
            }
            "collection" -> collections.fetchById(redemption.resourceId)?.let { dto ->
                val existing = collectionDao.byRemoteId(dto.id)
                collectionDao.upsert(dto.toEntity(existing?.id ?: UUID.randomUUID().toString(), existing?.createdAtEpochMs))
            }
        }
    }
}
