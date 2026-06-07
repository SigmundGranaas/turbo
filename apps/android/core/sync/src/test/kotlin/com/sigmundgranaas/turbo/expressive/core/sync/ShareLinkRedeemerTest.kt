package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionDao
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionEntity
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionItemEntity
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionWithCount
import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerDao
import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerEntity
import com.sigmundgranaas.turbo.expressive.core.data.database.PathDao
import com.sigmundgranaas.turbo.expressive.core.data.database.PathEntity
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

private class RedeemPathDao : PathDao {
    val rows = MutableStateFlow<Map<String, PathEntity>>(emptyMap())
    override fun observeAll(): Flow<List<PathEntity>> = rows.map { it.values.toList() }
    override suspend fun byId(id: String): PathEntity? = rows.value[id]
    override suspend fun byRemoteId(remoteId: String): PathEntity? = rows.value.values.find { it.remoteId == remoteId }
    override suspend fun pendingSync(): List<PathEntity> = emptyList()
    override suspend fun upsert(entity: PathEntity) { rows.value = rows.value + (entity.id to entity) }
    override suspend fun markSynced(id: String, remoteId: String, version: Long, updatedAt: Long) = Unit
    override suspend fun softDelete(id: String, ts: Long) = Unit
    override suspend fun delete(id: String) { rows.value = rows.value - id }
}

private object NoMarkerDao : MarkerDao {
    override fun observeAll(): Flow<List<MarkerEntity>> = flowOf(emptyList())
    override suspend fun byId(id: String): MarkerEntity? = null
    override suspend fun byRemoteId(remoteId: String): MarkerEntity? = null
    override suspend fun pendingSync(): List<MarkerEntity> = emptyList()
    override suspend fun upsert(entity: MarkerEntity) = Unit
    override suspend fun markSynced(id: String, remoteId: String, version: Long, updatedAt: Long) = Unit
    override suspend fun softDelete(id: String, ts: Long) = Unit
    override suspend fun delete(id: String) = Unit
}

private object NoCollectionDao : CollectionDao {
    override fun observeAll(): Flow<List<CollectionWithCount>> = flowOf(emptyList())
    override suspend fun byId(id: String): CollectionEntity? = null
    override suspend fun byRemoteId(remoteId: String): CollectionEntity? = null
    override suspend fun pendingSync(): List<CollectionEntity> = emptyList()
    override suspend fun upsert(entity: CollectionEntity) = Unit
    override suspend fun markSynced(id: String, remoteId: String, version: Long, updatedAt: Long) = Unit
    override suspend fun softDelete(id: String, ts: Long) = Unit
    override suspend fun delete(id: String) = Unit
    override suspend fun syncedCollections(): List<CollectionEntity> = emptyList()
    override suspend fun itemsForCollection(collectionId: String): List<CollectionItemEntity> = emptyList()
    override suspend fun clearItems(id: String) = Unit
    override suspend fun addItem(item: CollectionItemEntity) = Unit
    override suspend fun removeItem(collectionId: String, itemId: String, itemType: String) = Unit
    override suspend fun tombstoneItem(collectionId: String, itemId: String, itemType: String, ts: Long) = Unit
    override suspend fun markItemSynced(collectionId: String, itemId: String, itemType: String) = Unit
    override fun observeItemIds(collectionId: String, itemType: String): Flow<List<String>> = flowOf(emptyList())
    override fun observeCollectionsForItem(itemId: String, itemType: String): Flow<List<String>> = flowOf(emptyList())
}

private class RedeemSharing(private val redemption: Outcome<LinkRedemption>) : SharingRepository {
    var redeemed: String? = null
    override suspend fun friendCode() = Outcome.Success("turbo-x")
    override suspend fun createLink(resourceId: String, role: String) = Outcome.Success("u")
    override suspend fun redeemLink(token: String): Outcome<LinkRedemption> { redeemed = token; return redemption }
    override suspend fun sharedResources(since: String?) = Outcome.Success(ResourceSyncPageDto())
}

private class TrackByIdRemote(private val dto: TrackResponseDto?) : TrackRemote {
    override suspend fun pull(since: String?) = TracksDeltaDto()
    override suspend fun create(row: PathEntity) = RemoteRef("x", 1, null)
    override suspend fun update(row: PathEntity) = TrackUpdateOutcome.Gone
    override suspend fun delete(remoteId: String, version: Long) = Unit
    override suspend fun fetchById(remoteId: String): TrackResponseDto? = dto.takeIf { it?.id == remoteId }
}

private object NoLocationRemote : LocationRemote {
    override suspend fun pull(since: String?) = LocationsDeltaDto()
    override suspend fun create(row: MarkerEntity) = RemoteRef("x", 1, null)
    override suspend fun update(row: MarkerEntity) = LocationUpdateOutcome.Gone
    override suspend fun delete(remoteId: String, version: Long) = Unit
    override suspend fun fetchById(remoteId: String): LocationResponseDto? = null
}

private object NoCollectionRemote : CollectionRemote {
    override suspend fun pull(since: String?) = CollectionsDeltaDto()
    override suspend fun create(row: CollectionEntity) = RemoteRef("x", 1, null)
    override suspend fun update(row: CollectionEntity) = CollectionUpdateOutcome.Gone
    override suspend fun delete(remoteId: String, version: Long) = Unit
    override suspend fun fetchById(remoteId: String): CollectionResponseDto? = null
    override suspend fun addItem(collectionRemoteId: String, type: String, uuid: String) = Unit
    override suspend fun removeItem(collectionRemoteId: String, type: String, uuid: String) = Unit
}

class ShareLinkRedeemerTest {

    @Test
    fun `redeeming a path link fetches and inserts the shared track`() = runTest {
        val dao = RedeemPathDao()
        val server = TrackResponseDto(
            id = "srv-shared",
            geometry = TrackGeometryDto(points = listOf(WirePoint(10.5, 60.2))),
            metadata = TrackMetadataDto(name = "Shared loop"),
            updatedAt = "2024-01-01T00:00:00Z",
            version = 1,
        )
        val redeemer = ShareLinkRedeemer(
            sharing = RedeemSharing(Outcome.Success(LinkRedemption("srv-shared", "path", "viewer"))),
            tracks = TrackByIdRemote(server),
            locations = NoLocationRemote,
            collections = NoCollectionRemote,
            pathDao = dao,
            markerDao = NoMarkerDao,
            collectionDao = NoCollectionDao,
        )

        val result = redeemer.redeem("tok-123")

        assertEquals(Outcome.Success(LinkRedemption("srv-shared", "path", "viewer")), result)
        val row = dao.rows.value.values.single()
        assertEquals("srv-shared", row.remoteId)
        assertEquals("Shared loop", row.name)
        assertFalse(row.dirty) // shared resource lands clean (not re-pushed)
    }

    @Test
    fun `a failed redemption inserts nothing`() = runTest {
        val dao = RedeemPathDao()
        val redeemer = ShareLinkRedeemer(
            sharing = RedeemSharing(Outcome.Failure(RuntimeException("gone"))),
            tracks = TrackByIdRemote(null),
            locations = NoLocationRemote,
            collections = NoCollectionRemote,
            pathDao = dao,
            markerDao = NoMarkerDao,
            collectionDao = NoCollectionDao,
        )
        redeemer.redeem("tok-x")
        assertEquals(0, dao.rows.value.size)
    }
}
