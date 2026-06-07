package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.data.database.PathDao
import com.sigmundgranaas.turbo.expressive.core.data.database.PathEntity
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

private class FakePathDao : PathDao {
    val rows = MutableStateFlow<Map<String, PathEntity>>(emptyMap())
    override fun observeAll(): Flow<List<PathEntity>> =
        rows.map { m -> m.values.filter { it.deletedAtEpochMs == null }.sortedByDescending { e -> e.createdAtEpochMs } }
    override suspend fun byId(id: String): PathEntity? = rows.value[id]
    override suspend fun pendingSync(): List<PathEntity> = rows.value.values.filter { it.dirty }
    override suspend fun upsert(entity: PathEntity) { rows.value = rows.value + (entity.id to entity) }
    override suspend fun softDelete(id: String, ts: Long) {
        rows.value[id]?.let { rows.value = rows.value + (id to it.copy(deletedAtEpochMs = ts, dirty = true)) }
    }
    override suspend fun delete(id: String) { rows.value = rows.value - id }
}

class PathRepositoryTest {

    private val sample = SavedPath(
        id = "p-1",
        name = "Morning loop",
        path = GeoPath(
            points = listOf(LatLng(69.6480, 18.9560), LatLng(69.6560, 18.9700), LatLng(69.6620, 18.9820)),
            source = GeoPathSource.Recording,
            distanceM = 2143.0,
            ascentM = 120.0,
            descentM = 40.0,
            movingTimeSeconds = 1800,
            recordedAtEpochMs = 1_700_000_000_000L,
        ),
    )

    @Test
    fun `save then load round-trips geometry and metadata`() = runTest {
        val repo = RoomPathRepository(FakePathDao())
        repo.save(sample)
        val loaded = repo.byId("p-1")!!

        assertEquals("Morning loop", loaded.name)
        assertEquals(GeoPathSource.Recording, loaded.path.source)
        assertEquals(2143.0, loaded.path.distanceM, 1e-6)
        assertEquals(120.0, loaded.path.ascentM!!, 1e-6)
        assertEquals(1800, loaded.path.movingTimeSeconds)
        assertEquals(1_700_000_000_000L, loaded.path.recordedAtEpochMs)
        assertEquals(3, loaded.path.points.size)
        assertEquals(69.6620, loaded.path.points[2].lat, 1e-9)
        assertEquals(18.9820, loaded.path.points[2].lng, 1e-9)
    }

    @Test
    fun `save then load round-trips per-point elevations`() = runTest {
        val repo = RoomPathRepository(FakePathDao())
        val withElevation = sample.copy(
            id = "p-ele",
            path = sample.path.copy(elevations = listOf(10.0, null, 55.0)),
        )
        repo.save(withElevation)
        val loaded = repo.byId("p-ele")!!
        assertEquals(listOf(10.0, null, 55.0), loaded.path.elevations)
    }

    @Test
    fun `a track with no elevations loads back as null`() = runTest {
        val repo = RoomPathRepository(FakePathDao())
        repo.save(sample) // sample has no elevations
        assertNull(repo.byId("p-1")!!.path.elevations)
    }

    @Test
    fun `observeAll reflects saves and deletes`() = runTest {
        val repo = RoomPathRepository(FakePathDao())
        repo.save(sample)
        assertEquals(1, repo.observeAll().first().size)
        repo.delete("p-1")
        assertEquals(0, repo.observeAll().first().size)
        assertNull(repo.byId("p-1"))
    }

    @Test
    fun `a local save is marked dirty so the engine will push it`() = runTest {
        val dao = FakePathDao()
        RoomPathRepository(dao).save(sample)
        assertEquals(true, dao.rows.value["p-1"]!!.dirty)
        assertEquals(listOf("p-1"), dao.pendingSync().map { it.id })
    }

    @Test
    fun `editing a synced row preserves its remoteId and version`() = runTest {
        val dao = FakePathDao()
        // Simulate a row that has already synced (remoteId + version assigned by the server).
        dao.upsert(sample.toEntity().copy(remoteId = "srv-9", version = 4L, dirty = false))
        RoomPathRepository(dao).save(sample.copy(name = "Renamed"))
        val row = dao.rows.value["p-1"]!!
        assertEquals("srv-9", row.remoteId)
        assertEquals(4L, row.version)
        assertEquals("Renamed", row.name)
        assertEquals(true, row.dirty)
    }

    @Test
    fun `deleting a synced row tombstones it instead of purging`() = runTest {
        val dao = FakePathDao()
        dao.upsert(sample.toEntity().copy(remoteId = "srv-1", version = 1L, dirty = false))
        RoomPathRepository(dao).delete("p-1")
        val row = dao.rows.value["p-1"]!!
        assertEquals(true, row.deletedAtEpochMs != null) // still present as a tombstone
        assertEquals(true, row.dirty) // pending delete-push
        assertEquals(0, RoomPathRepository(dao).observeAll().first().size) // hidden from the UI
    }

    @Test
    fun `deleting a never-synced row purges it immediately`() = runTest {
        val dao = FakePathDao()
        val repo = RoomPathRepository(dao)
        repo.save(sample) // remoteId stays null
        repo.delete("p-1")
        assertNull(dao.rows.value["p-1"]) // gone, no tombstone needed
    }
}
