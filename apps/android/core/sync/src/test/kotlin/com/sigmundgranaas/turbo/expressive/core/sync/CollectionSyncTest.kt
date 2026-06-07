package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionDao
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionEntity
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionItemEntity
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionWithCount
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
    override suspend fun clearItems(id: String) = Unit
    override suspend fun addItem(item: CollectionItemEntity) = Unit
    override suspend fun removeItem(collectionId: String, itemId: String, itemType: String) = Unit
    override fun observeItemIds(collectionId: String, itemType: String): Flow<List<String>> = flowOf(emptyList())
    override fun observeCollectionsForItem(itemId: String, itemType: String): Flow<List<String>> = flowOf(emptyList())
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
        CollectionSyncer(remote, dao).sync(null)
        assertEquals(1, remote.created.size)
        assertEquals("#112233", remote.created[0].toWriteRequest().colorHex)
        assertEquals("srv-new", dao.rows.value["c1"]!!.remoteId)
        assertFalse(dao.rows.value["c1"]!!.dirty)
    }

    @Test
    fun `a 412 conflict adopts the server collection`() = runTest {
        val dao = FakeCollectionDao()
        dao.upsert(CollectionResponseDto(id = "srv-1", name = "Mine", version = 1, updatedAt = "2024-01-01T10:00:00Z").toEntity("c1", 1L).copy(dirty = true))
        val server = CollectionResponseDto(id = "srv-1", name = "Server truth", colorHex = "#00FF00", version = 7, updatedAt = "2024-01-01T12:00:00Z")
        val remote = FakeCollectionRemote(updateResult = CollectionUpdateOutcome.Conflict(server))
        CollectionSyncer(remote, dao).sync("x")
        val row = dao.rows.value["c1"]!!
        assertEquals("Server truth", row.name)
        assertEquals(7L, row.version)
        assertFalse(row.dirty)
    }
}
