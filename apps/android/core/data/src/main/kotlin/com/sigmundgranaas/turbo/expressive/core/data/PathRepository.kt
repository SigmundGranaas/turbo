package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.data.database.PathDao
import com.sigmundgranaas.turbo.expressive.core.data.database.PathEntity
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject

/** Offline-first store of recorded tracks / saved routes. */
interface PathRepository {
    fun observeAll(): Flow<List<SavedPath>>
    suspend fun byId(id: String): SavedPath?
    suspend fun save(path: SavedPath)
    suspend fun delete(id: String)
    /** The server-assigned id once this track has synced, or null if it hasn't been uploaded yet. */
    suspend fun remoteId(id: String): String?
}

class RoomPathRepository @Inject constructor(
    private val dao: PathDao,
) : PathRepository {
    override fun observeAll(): Flow<List<SavedPath>> =
        dao.observeAll().map { rows -> rows.map(PathEntity::toDomain) }

    override suspend fun byId(id: String): SavedPath? = dao.byId(id)?.toDomain()

    override suspend fun save(path: SavedPath) {
        val existing = dao.byId(path.id)
        dao.upsert(
            path.toEntity().copy(
                remoteId = existing?.remoteId,
                version = existing?.version,
                updatedAtEpochMs = System.currentTimeMillis(),
                deletedAtEpochMs = null,
                dirty = true,
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

    override suspend fun remoteId(id: String): String? = dao.byId(id)?.remoteId
}

private fun encodePoints(points: List<LatLng>): String =
    points.joinToString(";") { "${it.lat},${it.lng}" }

private fun decodePoints(encoded: String): List<LatLng> =
    if (encoded.isBlank()) {
        emptyList()
    } else {
        encoded.split(";").mapNotNull { pair ->
            val parts = pair.split(",")
            val lat = parts.getOrNull(0)?.toDoubleOrNull()
            val lng = parts.getOrNull(1)?.toDoubleOrNull()
            if (lat != null && lng != null) LatLng(lat, lng) else null
        }
    }

/** Decode ";"-joined elevations, keeping them parallel to [pointCount]; null if absent. */
private fun decodeElevations(encoded: String?, pointCount: Int): List<Double?>? {
    val parts = encoded?.takeIf { it.isNotBlank() }?.split(";") ?: return null
    if (parts.size != pointCount) return null
    val values = parts.map { it.toDoubleOrNull() }
    return if (values.any { it != null }) values else null
}

internal fun PathEntity.toDomain(): SavedPath = SavedPath(
    id = id,
    name = name,
    activityKind = activityKind?.let { k -> runCatching { com.sigmundgranaas.turbo.expressive.domain.ActivityKindId.valueOf(k) }.getOrNull() },
    path = decodePoints(points).let { pts ->
        GeoPath(
            points = pts,
            source = runCatching { GeoPathSource.valueOf(source) }.getOrDefault(GeoPathSource.Saved),
            elevations = decodeElevations(elevations, pts.size),
            distanceM = distanceM,
            ascentM = ascentM,
            descentM = descentM,
            movingTimeSeconds = durationSec,
            recordedAtEpochMs = createdAtEpochMs,
        )
    },
)

internal fun SavedPath.toEntity(): PathEntity = PathEntity(
    id = id,
    name = name,
    source = path.source.name,
    points = encodePoints(path.points),
    distanceM = path.distanceM,
    ascentM = path.ascentM,
    descentM = path.descentM,
    durationSec = path.movingTimeSeconds,
    createdAtEpochMs = path.recordedAtEpochMs ?: 0L,
    elevations = path.elevations?.joinToString(";") { it?.toString().orEmpty() },
    activityKind = activityKind?.name,
)
