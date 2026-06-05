package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerDao
import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerEntity
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject

/**
 * Offline-first source of truth for user markers. Backed by Room; the DB is
 * seeded from [com.sigmundgranaas.turbo.expressive.domain.SampleData] on first
 * create so the map is never empty.
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

    override suspend fun upsert(marker: Marker) = dao.upsert(marker.toEntity())
    override suspend fun delete(id: String) = dao.delete(id)
}

internal fun MarkerEntity.toDomain(): Marker = Marker(
    id = id,
    name = name,
    kind = ActivityKindId.fromKey(kind) ?: ActivityKindId.Mountain,
    position = LatLng(lat, lng),
    colorArgb = colorArgb,
)

internal fun Marker.toEntity(): MarkerEntity = MarkerEntity(
    id = id,
    name = name,
    kind = kind.key,
    lat = position.lat,
    lng = position.lng,
    colorArgb = colorArgb,
)
