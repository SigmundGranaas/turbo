package com.sigmundgranaas.turbo.expressive.core.sync

import com.sigmundgranaas.turbo.expressive.core.data.database.PathDao
import com.sigmundgranaas.turbo.expressive.core.data.database.PathEntity
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

private class FakePathDao : PathDao {
    val rows = MutableStateFlow<Map<String, PathEntity>>(emptyMap())
    override fun observeAll(): Flow<List<PathEntity>> =
        rows.map { m -> m.values.filter { it.deletedAtEpochMs == null } }
    override suspend fun byId(id: String): PathEntity? = rows.value[id]
    override suspend fun byRemoteId(remoteId: String): PathEntity? = rows.value.values.find { it.remoteId == remoteId }
    override suspend fun pendingSync(): List<PathEntity> = rows.value.values.filter { it.dirty }
    override suspend fun upsert(entity: PathEntity) { rows.value = rows.value + (entity.id to entity) }
    override suspend fun markSynced(id: String, remoteId: String, version: Long, updatedAt: Long) {
        rows.value[id]?.let {
            rows.value = rows.value + (id to it.copy(remoteId = remoteId, version = version, updatedAtEpochMs = updatedAt, dirty = false))
        }
    }
    override suspend fun softDelete(id: String, ts: Long) {
        rows.value[id]?.let { rows.value = rows.value + (id to it.copy(deletedAtEpochMs = ts, dirty = true)) }
    }
    override suspend fun delete(id: String) { rows.value = rows.value - id }
}

private class FakeTrackRemote(
    var pullResult: TracksDeltaDto = TracksDeltaDto(),
    var updateResult: TrackUpdateOutcome = TrackUpdateOutcome.Updated(RemoteRef("srv", 2, "2024-01-01T00:00:00Z")),
) : TrackRemote {
    val created = mutableListOf<PathEntity>()
    val updated = mutableListOf<PathEntity>()
    val deleted = mutableListOf<String>()
    var nextCreateId = 1
    override suspend fun pull(since: String?): TracksDeltaDto = pullResult
    override suspend fun create(row: PathEntity): RemoteRef {
        created += row
        return RemoteRef("srv-new-${nextCreateId++}", 1, "2024-01-01T00:00:00Z")
    }
    override suspend fun update(row: PathEntity): TrackUpdateOutcome { updated += row; return updateResult }
    override suspend fun delete(remoteId: String, version: Long) { deleted += remoteId }
}

private fun remoteTrack(id: String, name: String, updatedAt: String, version: Long) = TrackResponseDto(
    id = id,
    geometry = TrackGeometryDto(points = listOf(WirePoint(10.5, 60.2))),
    metadata = TrackMetadataDto(name = name),
    stats = TrackStatsDto(distanceMeters = 100.0),
    updatedAt = updatedAt,
    version = version,
)

class TrackSyncerTest {

    @Test
    fun `a fresh remote track is inserted locally as clean`() = runTest {
        val dao = FakePathDao()
        val remote = FakeTrackRemote(pullResult = TracksDeltaDto(items = listOf(remoteTrack("srv-1", "Loop", "2024-01-01T11:00:00Z", 3)), serverTime = "2024-01-02T00:00:00Z"))
        val cursor = TrackSyncer(remote, dao).sync(since = null)

        val row = dao.rows.value.values.single()
        assertEquals("srv-1", row.remoteId)
        assertEquals("Loop", row.name)
        assertFalse(row.dirty)
        assertEquals(3L, row.version)
        assertEquals("2024-01-02T00:00:00Z", cursor)
    }

    @Test
    fun `a dirty local-only track is created on the server and marked synced`() = runTest {
        val dao = FakePathDao()
        dao.upsert(
            PathEntity(id = "local-1", name = "New", source = "Recording", points = "60.2,10.5", distanceM = 1.0, ascentM = null, descentM = null, durationSec = null, createdAtEpochMs = 1L, dirty = true),
        )
        val remote = FakeTrackRemote()
        TrackSyncer(remote, dao).sync(since = null)

        assertEquals(1, remote.created.size)
        val row = dao.rows.value["local-1"]!!
        assertEquals("srv-new-1", row.remoteId)
        assertFalse(row.dirty)
    }

    @Test
    fun `a newer server copy overwrites a clean local row`() = runTest {
        val dao = FakePathDao()
        dao.upsert(remoteTrack("srv-1", "Old name", "2024-01-01T10:00:00Z", 1).toEntity("local-1"))
        val remote = FakeTrackRemote(pullResult = TracksDeltaDto(items = listOf(remoteTrack("srv-1", "New name", "2024-01-01T12:00:00Z", 2))))
        TrackSyncer(remote, dao).sync(since = "x")

        assertEquals("New name", dao.rows.value["local-1"]!!.name)
        assertEquals(2L, dao.rows.value["local-1"]!!.version)
    }

    @Test
    fun `a server tombstone purges the clean local row`() = runTest {
        val dao = FakePathDao()
        dao.upsert(remoteTrack("srv-1", "Doomed", "2024-01-01T10:00:00Z", 1).toEntity("local-1"))
        val remote = FakeTrackRemote(pullResult = TracksDeltaDto(deleted = listOf(TombstoneDto("srv-1", "2024-01-02T00:00:00Z", 2))))
        TrackSyncer(remote, dao).sync(since = "x")

        assertNull(dao.rows.value["local-1"])
    }

    @Test
    fun `a 412 conflict on update adopts the server copy`() = runTest {
        val dao = FakePathDao()
        // a dirty synced row whose update will lose to the server
        val local = remoteTrack("srv-1", "My edit", "2024-01-01T11:00:00Z", 1).toEntity("local-1").copy(dirty = true)
        dao.upsert(local)
        val serverWins = remoteTrack("srv-1", "Server truth", "2024-01-01T12:00:00Z", 5)
        val remote = FakeTrackRemote(updateResult = TrackUpdateOutcome.Conflict(serverWins))
        TrackSyncer(remote, dao).sync(since = "x")

        val row = dao.rows.value["local-1"]!!
        assertEquals("Server truth", row.name)
        assertEquals(5L, row.version)
        assertFalse(row.dirty)
        assertNotNull(row.remoteId)
    }

    @Test
    fun `a synced local deletion is pushed and purged`() = runTest {
        val dao = FakePathDao()
        val tombstoned = remoteTrack("srv-1", "Bye", "2024-01-01T10:00:00Z", 4).toEntity("local-1").copy(deletedAtEpochMs = 999L, dirty = true)
        dao.upsert(tombstoned)
        val remote = FakeTrackRemote()
        TrackSyncer(remote, dao).sync(since = "x")

        assertTrue(remote.deleted.contains("srv-1"))
        assertNull(dao.rows.value["local-1"])
    }
}
