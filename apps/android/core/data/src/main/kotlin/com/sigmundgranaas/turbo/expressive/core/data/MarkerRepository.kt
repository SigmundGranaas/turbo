package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerDao
import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerEntity
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.MarkerKind
import com.sigmundgranaas.turbo.expressive.domain.WeatherSnapshot
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import javax.inject.Inject

/**
 * Offline-first source of truth for user markers. Backed by Room; starts empty —
 * markers are created by the user or arrive via sync (no seeded sample content).
 */
interface MarkerRepository {
    fun observeAll(): Flow<List<Marker>>
    suspend fun upsert(marker: Marker)
    suspend fun delete(id: String)
}

class RoomMarkerRepository @Inject constructor(
    private val dao: MarkerDao,
) : MarkerRepository {
    override fun observeAll(): Flow<List<Marker>> =
        dao.observeAll().map { rows -> rows.map(MarkerEntity::toDomain) }

    override suspend fun upsert(marker: Marker) {
        val existing = dao.byId(marker.id)
        dao.upsert(
            marker.toEntity().copy(
                remoteId = existing?.remoteId,
                version = existing?.version,
                updatedAtEpochMs = System.currentTimeMillis(),
                deletedAtEpochMs = null,
                dirty = true,
                readOnly = existing?.readOnly ?: false,
            ),
        )
    }

    override suspend fun delete(id: String) {
        // Synced rows become tombstones so the engine can push the delete; never-synced rows are purged.
        if (dao.byId(id)?.remoteId != null) {
            dao.softDelete(id, System.currentTimeMillis())
        } else {
            dao.delete(id)
        }
    }
}

internal fun MarkerEntity.toDomain(): Marker = Marker(
    id = id,
    name = name,
    kind = ActivityKindId.fromKey(kind) ?: ActivityKindId.Mountain,
    position = LatLng(lat, lng),
    colorArgb = colorArgb,
    notes = notes,
    // Unknown discriminators (older rows, forward-compat) degrade to a plain pin.
    markerKind = runCatching { MarkerKind.valueOf(markerKind) }.getOrDefault(MarkerKind.Standard),
    forecast = cachedForecast?.let(WeatherSnapshotCodec::decode),
    forecastFetchedAtEpochMs = forecastFetchedAtEpochMs,
)

internal fun Marker.toEntity(): MarkerEntity = MarkerEntity(
    id = id,
    name = name,
    kind = kind.key,
    lat = position.lat,
    lng = position.lng,
    colorArgb = colorArgb,
    notes = notes,
    markerKind = markerKind.name,
    cachedForecast = forecast?.let(WeatherSnapshotCodec::encode),
    forecastFetchedAtEpochMs = forecastFetchedAtEpochMs,
)

/**
 * JSON codec for a weather pin's cached [WeatherSnapshot] at the DB-string boundary. The
 * pure domain stays serialization-free; this private DTO mirror carries the fields on the
 * wire-to-Room hop. Corrupt/legacy JSON decodes to `null` (the pin re-fetches) rather than crash.
 */
internal object WeatherSnapshotCodec {
    @Serializable
    private data class Dto(
        val temperatureC: Double? = null,
        val symbolCode: String? = null,
        val windSpeedMs: Double? = null,
        val windFromDeg: Double? = null,
        val precipitationMm: Double? = null,
        val waveHeightM: Double? = null,
        val waveFromDeg: Double? = null,
        val seaTemperatureC: Double? = null,
    )

    private val json = Json { ignoreUnknownKeys = true }

    fun encode(snap: WeatherSnapshot): String = json.encodeToString(
        Dto(
            temperatureC = snap.temperatureC,
            symbolCode = snap.symbolCode,
            windSpeedMs = snap.windSpeedMs,
            windFromDeg = snap.windFromDeg,
            precipitationMm = snap.precipitationMm,
            waveHeightM = snap.waveHeightM,
            waveFromDeg = snap.waveFromDeg,
            seaTemperatureC = snap.seaTemperatureC,
        ),
    )

    fun decode(raw: String): WeatherSnapshot? = runCatching {
        val d = json.decodeFromString<Dto>(raw)
        WeatherSnapshot(
            temperatureC = d.temperatureC,
            symbolCode = d.symbolCode,
            windSpeedMs = d.windSpeedMs,
            windFromDeg = d.windFromDeg,
            precipitationMm = d.precipitationMm,
            waveHeightM = d.waveHeightM,
            waveFromDeg = d.waveFromDeg,
            seaTemperatureC = d.seaTemperatureC,
        )
    }.getOrNull()
}
