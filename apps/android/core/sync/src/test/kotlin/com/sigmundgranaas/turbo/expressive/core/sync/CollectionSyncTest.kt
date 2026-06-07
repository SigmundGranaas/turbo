package com.sigmundgranaas.turbo.expressive.core.sync

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

private class FakeCollectionDao : CollectionDao {
    val rows = MutableStateFlow<Map<String, CollectionEntity>>(emptyMap())
    override fun observeAll(): Flow<List<CollectionWithCount>> = rows.map { m ->
        m.values.filter { it.deletedAtEpochMs == null }.map { CollectionWithCount(it.id, it.name, it.colorArgb, it.icon, 0) }
    }
    override suspend fun byId(id: String): CollectionEntity? = rows.value[id]
    override suspend fun byRemoteId(remoteId: String): CollectionEntity? = rows.value.values.find { it.remoteId == remoteId }
    override suspend fun pendingSync(): List<CollectionEntity> = rows.value.values.filter { it.dirty }
    override suspend fun upsert(entity: CollectionEntity) { rows.value = rows.value + (entity.id to entity) }
    override suspend fun markSynced(id: String, remoteId: String, version: Long, updatedAt: Long) {
        rows.value[id]?.let { rows.value = rows.value + (id to it.copy(remoteId = remoteId, version = version, updatedAtEpochMs = updatedAt, dirty = false)) }
    }
    override suspend fun softDelete(id: String, ts: Long) {
        rows.value[id]?.let { rows.value = rows.value + (id to it.copy(deletedAtEpochMs = ts, dirty = true)) }
    }
    override suspend fun delete(id: String) { rows.value = rows.value - id }
    override suspend fun syncedCollections(): List<CollectionEntity> = rows.value.values.filter { it.remoteId != null && it.deletedAtEpochMs == null }
    val items = mutableListOf<CollectionItemEntity>()
    override suspend fun itemsForCollection(collectionId: String): List<CollectionItemEntity> = items.filter { it.collectionId == collectionId }
    override suspend fun clearItems(id: String) { items.removeAll { it.collectionId == id } }
    override suspend fun addItem(item: CollectionItemEntity) { if (items.none { it.collectionId == item.collectionId && it.itemId == item.itemId && it.itemType == item.itemType }) items += item }
    override suspend fun removeItem(collectionId: String, itemId: String, itemType: String) = Unit
    override fun observeItemIds(collectionId: String, itemType: String): Flow<List<String>> = flowOf(emptyList())
    override fun observeCollectionsForItem(itemId: String, itemType: String): Flow<List<String>> = flowOf(emptyList())
}

private class FakeItemPathDao(private val byRemote: Map<String, String> = emptyMap(), private val byLocal: Map<String, String> = emptyMap()) : PathDao {
    override fun observeAll(): Flow<List<PathEntity>> = flowOf(emptyList())
    override suspend fun byId(id: String): PathEntity? = byLocal[id]?.let { pathRow(id, it) }
    override suspend fun byRemoteId(remoteId: String): PathEntity? = byRemote[remoteId]?.let { pathRow(it, remoteId) }
    override suspend fun pendingSync(): List<PathEntity> = emptyList()
    override suspend fun upsert(entity: PathEntity) = Unit
    override suspend fun markSynced(id: String, remoteId: String, version: Long, updatedAt: Long) = Unit
    override suspend fun softDelete(id: String, ts: Long) = Unit
    override suspend fun delete(id: String) = Unit
    private fun pathRow(local: String, remote: String) = PathEntity(id = local, name = "t", source = "Saved", points = "", distanceM = 0.0, ascentM = null, descentM = null, durationSec = null, createdAtEpochMs = 0L, remoteId = remote)
}

private object NoItemMarkerDao : MarkerDao {
    override fun observeAll(): Flow<List<MarkerEntity>> = flowOf(emptyList())
    override suspend fun byId(id: String): MarkerEntity? = null
    override suspend fun byRemoteId(remoteId: String): MarkerEntity? = null
    override suspend fun pendingSync(): List<MarkerEntity> = emptyList()
    override suspend fun upsert(entity: MarkerEntity) = Unit
    override suspend fun markSynced(id: String, remoteId: String, version: Long, updatedAt: Long) = Unit
    override suspend fun softDelete(id: String, ts: Long) = Unit
    override suspend fun delete(id: String) = Unit
}

private class FakeCollectionRemote(
    var pullResult: CollectionsDeltaDto = CollectionsDeltaDto(),
    var updateResult: CollectionUpdateOutcome = CollectionUpdateOutcome.Updated(RemoteRef("srv", 2, "2024-01-01T00:00:00Z")),
) : CollectionRemote {
    val created = mutableListOf<CollectionEntity>()
    override suspend fun pull(since: String?) = pullResult
    override suspend fun create(row: CollectionEntity): RemoteRef { created += row; return RemoteRef("srv-new", 1, "2024-01-01T00:00:00Z") }
    override suspend fun update(row: CollectionEntity) = updateResult
    override suspend fun delete(remoteId: String, version: Long) = Unit
    override suspend fun fetchById(remoteId: String): CollectionResponseDto? = pullResult.items.find { it.id == remoteId }
    val addedItems = mutableListOf<Triple<String, String, String>>()
    override suspend fun addItem(collectionRemoteId: String, type: String, uuid: String) { addedItems += Triple(collectionRemoteId, type, uuid) }
}

class CollectionSyncTest {

    @Test
    fun `colour round-trips through hex`() {
        assertEquals("#FF5733", 0xFFFF5733.toColorHex())
        assertEquals(0xFFFF5733L, "#FF5733".parseColorArgb())
    }

    @Test
    fun `a remote collection maps onto an entity`() {
        val dto = CollectionResponseDto(id = "srv-1", name = "Peaks", colorHex = "#3366CC", iconKey = "folder", updatedAt = "2024-01-01T11:00:00Z", version = 2)
        val e = dto.toEntity("local-1", existingCreatedAt = 5L)
        assertEquals("local-1", e.id)
        assertEquals("srv-1", e.remoteId)
        assertEquals("Peaks", e.name)
        assertEquals(0xFF3366CCL, e.colorArgb)
        assertEquals("folder", e.icon)
        assertFalse(e.dirty)
    }

    @Test
    fun `a dirty local collection is created on the server`() = runTest {
        val dao = FakeCollectionDao()
        dao.upsert(CollectionEntity(id = "c1", name = "Trips", colorArgb = 0xFF112233, icon = "star", createdAtEpochMs = 1L, dirty = true))
        val remote = FakeCollectionRemote()
        CollectionSyncer(remote, dao, FakeItemPathDao(), NoItemMarkerDao).sync(null)
        assertEquals(1, remote.created.size)
        assertEquals("#112233", remote.created[0].toWriteRequest().colorHex)
        assertEquals("srv-new", dao.rows.value["c1"]!!.remoteId)
        assertFalse(dao.rows.value["c1"]!!.dirty)
    }

    @Test
    fun `a synced collection pushes its track membership by resolving the resource id`() = runTest {
        val dao = FakeCollectionDao()
        // a synced collection with one local Path member
        dao.upsert(CollectionEntity(id = "c1", name = "Trips", colorArgb = null, icon = null, createdAtEpochMs = 1L, remoteId = "srv-c1", version = 1, dirty = false))
        dao.addItem(CollectionItemEntity("c1", "local-path-1", "Path"))
        // the path resolves to a server id
        val pathDao = FakeItemPathDao(byLocal = mapOf("local-path-1" to "srv-path-9"))
        val remote = FakeCollectionRemote()

        CollectionSyncer(remote, dao, pathDao, NoItemMarkerDao).sync("x")

        assertEquals(listOf(Triple("srv-c1", "track", "srv-path-9")), remote.addedItems)
    }

    @Test
    fun `an incoming server membership is adopted locally`() = runTest {
        val dao = FakeCollectionDao()
        val pathDao = FakeItemPathDao(byRemote = mapOf("srv-path-9" to "local-path-1"))
        val remote = FakeCollectionRemote(
            pullResult = CollectionsDeltaDto(
                items = listOf(
                    CollectionResponseDto(
                        id = "srv-c1", name = "Shared", updatedAt = "2024-01-01T00:00:00Z", version = 1,
                        items = listOf(CollectionItemRefDto(type = "track", uuid = "srv-path-9")),
                    ),
                ),
            ),
        )

        CollectionSyncer(remote, dao, pathDao, NoItemMarkerDao).sync(null)

        val localCollectionId = dao.byRemoteId("srv-c1")!!.id
        assertEquals(listOf(CollectionItemEntity(localCollectionId, "local-path-1", "Path")), dao.items.toList())
    }

    @Test
    fun `a 412 conflict adopts the server collection`() = runTest {
        val dao = FakeCollectionDao()
        dao.upsert(CollectionResponseDto(id = "srv-1", name = "Mine", version = 1, updatedAt = "2024-01-01T10:00:00Z").toEntity("c1", 1L).copy(dirty = true))
        val server = CollectionResponseDto(id = "srv-1", name = "Server truth", colorHex = "#00FF00", version = 7, updatedAt = "2024-01-01T12:00:00Z")
        val remote = FakeCollectionRemote(updateResult = CollectionUpdateOutcome.Conflict(server))
        CollectionSyncer(remote, dao, FakeItemPathDao(), NoItemMarkerDao).sync("x")
        val row = dao.rows.value["c1"]!!
        assertEquals("Server truth", row.name)
        assertEquals(7L, row.version)
        assertFalse(row.dirty)
    }
}
