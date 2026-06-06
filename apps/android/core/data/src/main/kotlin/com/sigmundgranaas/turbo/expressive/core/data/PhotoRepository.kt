package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.data.database.PhotoDao
import com.sigmundgranaas.turbo.expressive.core.data.database.PhotoEntity
import com.sigmundgranaas.turbo.expressive.domain.Photo
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject

/** Local store for geotagged photos, optionally attached to a marker. */
interface PhotoRepository {
    fun observeAll(): Flow<List<Photo>>
    fun observeForMarker(markerId: String): Flow<List<Photo>>
    suspend fun add(photo: Photo)
    suspend fun delete(id: String)
}

class RoomPhotoRepository @Inject constructor(
    private val dao: PhotoDao,
) : PhotoRepository {

    override fun observeAll(): Flow<List<Photo>> = dao.observeAll().map { it.map(PhotoEntity::toDomain) }

    override fun observeForMarker(markerId: String): Flow<List<Photo>> =
        dao.observeForMarker(markerId).map { it.map(PhotoEntity::toDomain) }

    override suspend fun add(photo: Photo) = dao.upsert(photo.toEntity())

    override suspend fun delete(id: String) = dao.delete(id)
}

private fun PhotoEntity.toDomain() = Photo(id, markerId, lat, lng, uri, capturedAtEpochMs)

private fun Photo.toEntity() = PhotoEntity(id, markerId, lat, lng, uri, capturedAtEpochMs)
