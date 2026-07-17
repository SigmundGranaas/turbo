package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerDao
import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerEntity
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.MarkerKind
import com.sigmundgranaas.turbo.expressive.domain.WeatherSnapshot
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

private class FakeMarkerDao : MarkerDao {
    val rows = MutableStateFlow<Map<String, MarkerEntity>>(emptyMap())
    override fun observeAll(): Flow<List<MarkerEntity>> =
        rows.map { m -> m.values.filter { it.deletedAtEpochMs == null }.sortedBy { it.name } }
    override suspend fun byId(id: String): MarkerEntity? = rows.value[id]
    override suspend fun byRemoteId(remoteId: String): MarkerEntity? = rows.value.values.find { it.remoteId == remoteId }
    override suspend fun pendingSync(): List<MarkerEntity> = rows.value.values.filter { it.dirty && !it.readOnly }
    override suspend fun upsert(entity: MarkerEntity) { rows.value = rows.value + (entity.id to entity) }
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

class MarkerRepositoryTest {

    private val position = LatLng(69.6489, 18.9551)

    @Test
    fun `a weather pin round-trips as a marker kind carrying its cached forecast`() = runTest {
        val repo = RoomMarkerRepository(FakeMarkerDao())
        val pin = Marker(
            id = "w-1",
            name = "Weather pin",
            kind = ActivityKindId.Viewpoint,
            position = position,
            markerKind = MarkerKind.WeatherPin,
            forecast = WeatherSnapshot(
                temperatureC = -3.5,
                symbolCode = "snow",
                windSpeedMs = 9.0,
                windFromDeg = 340.0,
                precipitationMm = 0.8,
                waveHeightM = 1.4,
                waveFromDeg = 300.0,
                seaTemperatureC = 5.5,
            ),
            forecastFetchedAtEpochMs = 1_700_000_000_000L,
        )
        repo.upsert(pin)
        val loaded = repo.observeAll().first().single()

        assertEquals(MarkerKind.WeatherPin, loaded.markerKind)
        assertEquals(1_700_000_000_000L, loaded.forecastFetchedAtEpochMs)
        val f = loaded.forecast!!
        assertEquals(-3.5, f.temperatureC!!, 1e-9)
        assertEquals("snow", f.symbolCode)
        assertEquals(9.0, f.windSpeedMs!!, 1e-9)
        assertEquals(0.8, f.precipitationMm!!, 1e-9)
        assertEquals(1.4, f.waveHeightM!!, 1e-9)
        assertEquals(5.5, f.seaTemperatureC!!, 1e-9)
    }

    @Test
    fun `a plain marker round-trips as Standard with no cached forecast`() = runTest {
        val repo = RoomMarkerRepository(FakeMarkerDao())
        repo.upsert(Marker(id = "m-1", name = "Camp", kind = ActivityKindId.Camping, position = position))
        val loaded = repo.observeAll().first().single()
        assertEquals(MarkerKind.Standard, loaded.markerKind)
        assertNull(loaded.forecast)
        assertNull(loaded.forecastFetchedAtEpochMs)
    }
}
