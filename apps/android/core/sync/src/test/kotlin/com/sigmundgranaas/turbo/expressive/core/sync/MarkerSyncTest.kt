package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerDao
import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerEntity
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

private class FakeMarkerDao : MarkerDao {
    val rows = MutableStateFlow<Map<String, MarkerEntity>>(emptyMap())
    override fun observeAll(): Flow<List<MarkerEntity>> = rows.map { m -> m.values.filter { it.deletedAtEpochMs == null } }
    override suspend fun byId(id: String): MarkerEntity? = rows.value[id]
    override suspend fun byRemoteId(remoteId: String): MarkerEntity? = rows.value.values.find { it.remoteId == remoteId }
    override suspend fun pendingSync(): List<MarkerEntity> = rows.value.values.filter { it.dirty }
    override suspend fun upsert(entity: MarkerEntity) { rows.value = rows.value + (entity.id to entity) }
    override suspend fun markSynced(id: String, remoteId: String, version: Long, updatedAt: Long) {
        rows.value[id]?.let { rows.value = rows.value + (id to it.copy(remoteId = remoteId, version = version, updatedAtEpochMs = updatedAt, dirty = false)) }
    }
    override suspend fun softDelete(id: String, ts: Long) {
        rows.value[id]?.let { rows.value = rows.value + (id to it.copy(deletedAtEpochMs = ts, dirty = true)) }
    }
    override suspend fun delete(id: String) { rows.value = rows.value - id }
}

private class FakeLocationRemote(
    var pullResult: LocationsDeltaDto = LocationsDeltaDto(),
) : LocationRemote {
    val created = mutableListOf<MarkerEntity>()
    val deleted = mutableListOf<String>()
    override suspend fun pull(since: String?) = pullResult
    override suspend fun create(row: MarkerEntity): RemoteRef { created += row; return RemoteRef("srv-new", 1, "2024-01-01T00:00:00Z") }
    override suspend fun update(row: MarkerEntity): LocationUpdateOutcome = LocationUpdateOutcome.Updated(RemoteRef("srv", 2, "2024-01-01T00:00:00Z"))
    override suspend fun delete(remoteId: String, version: Long) { deleted += remoteId }
}

private fun remoteMarker(id: String, name: String, lat: Double, lng: Double, updatedAt: String, version: Long) = LocationResponseDto(
    id = id,
    geometry = LocationGeometryDto(longitude = lng, latitude = lat),
    display = LocationDisplayDto(name = name, description = "note", icon = "peak"),
    updatedAt = updatedAt,
    version = version,
)

class MarkerSyncTest {

    @Test
    fun `parses a locations delta and maps a remote marker onto an entity`() {
        val dto = remoteMarker("srv-1", "Summit", 60.2, 10.5, "2024-01-01T11:00:00Z", 4)
        val entity = dto.toEntity("local-1")
        assertEquals("local-1", entity.id)
        assertEquals("srv-1", entity.remoteId)
        assertEquals("Summit", entity.name)
        assertEquals("peak", entity.kind)         // server icon → local kind
        assertEquals("note", entity.notes)        // server description → local notes
        assertEquals(60.2, entity.lat, 1e-9)
        assertEquals(10.5, entity.lng, 1e-9)
        assertFalse(entity.dirty)
    }

    @Test
    fun `a local marker serializes into a write request`() {
        val entity = MarkerEntity(id = "l1", name = "Camp", kind = "tent", lat = 61.0, lng = 9.0, colorArgb = 0xFFFF0000, notes = "by the lake")
        val req = entity.toWriteRequest()
        assertEquals(9.0, req.geometry.longitude, 1e-9)
        assertEquals(61.0, req.geometry.latitude, 1e-9)
        assertEquals("Camp", req.display.name)
        assertEquals("tent", req.display.icon)
        assertEquals("by the lake", req.display.description)
    }

    @Test
    fun `a fresh remote marker is inserted as clean`() = runTest {
        val dao = FakeMarkerDao()
        val remote = FakeLocationRemote(LocationsDeltaDto(items = listOf(remoteMarker("srv-1", "Summit", 60.2, 10.5, "2024-01-01T11:00:00Z", 4)), serverTime = "2024-02-01T00:00:00Z"))
        val cursor = MarkerSyncer(remote, dao).sync(null)
        val row = dao.rows.value.values.single()
        assertEquals("srv-1", row.remoteId)
        assertFalse(row.dirty)
        assertEquals("2024-02-01T00:00:00Z", cursor)
    }

    @Test
    fun `a dirty local marker is created on the server`() = runTest {
        val dao = FakeMarkerDao()
        dao.upsert(MarkerEntity(id = "l1", name = "New", kind = "tent", lat = 61.0, lng = 9.0, colorArgb = null, notes = null, dirty = true))
        val remote = FakeLocationRemote()
        MarkerSyncer(remote, dao).sync(null)
        assertEquals(1, remote.created.size)
        assertEquals("srv-new", dao.rows.value["l1"]!!.remoteId)
        assertFalse(dao.rows.value["l1"]!!.dirty)
    }

    @Test
    fun `a server tombstone purges the clean local marker`() = runTest {
        val dao = FakeMarkerDao()
        dao.upsert(remoteMarker("srv-1", "Doomed", 60.0, 10.0, "2024-01-01T10:00:00Z", 1).toEntity("l1"))
        val remote = FakeLocationRemote(LocationsDeltaDto(deleted = listOf(TombstoneDto("srv-1", "2024-02-01T00:00:00Z", 2))))
        MarkerSyncer(remote, dao).sync("x")
        assertNull(dao.rows.value["l1"])
    }

    @Test
    fun `a synced local deletion is pushed and purged`() = runTest {
        val dao = FakeMarkerDao()
        dao.upsert(remoteMarker("srv-1", "Bye", 60.0, 10.0, "2024-01-01T10:00:00Z", 3).toEntity("l1").copy(deletedAtEpochMs = 999L, dirty = true))
        val remote = FakeLocationRemote()
        MarkerSyncer(remote, dao).sync("x")
        assertTrue(remote.deleted.contains("srv-1"))
        assertNull(dao.rows.value["l1"])
    }
}
