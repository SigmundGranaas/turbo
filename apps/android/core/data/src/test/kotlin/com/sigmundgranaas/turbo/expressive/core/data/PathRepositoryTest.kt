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
    private val rows = MutableStateFlow<Map<String, PathEntity>>(emptyMap())
    override fun observeAll(): Flow<List<PathEntity>> =
        rows.map { it.values.sortedByDescending { e -> e.createdAtEpochMs } }
    override suspend fun byId(id: String): PathEntity? = rows.value[id]
    override suspend fun upsert(entity: PathEntity) { rows.value = rows.value + (entity.id to entity) }
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
    fun `observeAll reflects saves and deletes`() = runTest {
        val repo = RoomPathRepository(FakePathDao())
        repo.save(sample)
        assertEquals(1, repo.observeAll().first().size)
        repo.delete("p-1")
        assertEquals(0, repo.observeAll().first().size)
        assertNull(repo.byId("p-1"))
    }
}
