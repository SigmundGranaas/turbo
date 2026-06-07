package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionDao
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionEntity
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionItemEntity
import com.sigmundgranaas.turbo.expressive.domain.CollectionItemType
import com.sigmundgranaas.turbo.expressive.domain.MapCollection
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject

/** Local CRUD + membership for user collections (folders of markers/tracks). */
interface CollectionRepository {
    fun observeAll(): Flow<List<MapCollection>>
    suspend fun upsert(collection: MapCollection)
    suspend fun delete(id: String)
    suspend fun addItem(collectionId: String, itemId: String, type: CollectionItemType)
    suspend fun removeItem(collectionId: String, itemId: String, type: CollectionItemType)
    fun observeItemIds(collectionId: String, type: CollectionItemType): Flow<List<String>>
    /** The ids of collections that currently contain the given item. */
    fun observeCollectionsForItem(itemId: String, type: CollectionItemType): Flow<List<String>>
}

class RoomCollectionRepository @Inject constructor(
    private val dao: CollectionDao,
) : CollectionRepository {

    override fun observeAll(): Flow<List<MapCollection>> = dao.observeAll().map { rows ->
        rows.map { MapCollection(it.id, it.name, it.colorArgb, it.icon, it.itemCount) }
    }

    override suspend fun upsert(collection: MapCollection) {
        val existing = dao.byId(collection.id)
        dao.upsert(
            CollectionEntity(
                id = collection.id,
                name = collection.name,
                colorArgb = collection.colorArgb,
                icon = collection.icon,
                createdAtEpochMs = existing?.createdAtEpochMs ?: System.currentTimeMillis(),
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
        dao.clearItems(id)
        // Synced rows become tombstones so the engine can push the delete; never-synced rows are purged.
        if (dao.byId(id)?.remoteId != null) {
            dao.softDelete(id, System.currentTimeMillis())
        } else {
            dao.delete(id)
        }
    }

    override suspend fun addItem(collectionId: String, itemId: String, type: CollectionItemType) {
        dao.addItem(CollectionItemEntity(collectionId, itemId, type.name))
    }

    override suspend fun removeItem(collectionId: String, itemId: String, type: CollectionItemType) {
        // In a synced collection, tombstone the membership so the engine can push the DELETE;
        // otherwise just drop it.
        if (dao.byId(collectionId)?.remoteId != null) {
            dao.tombstoneItem(collectionId, itemId, type.name, System.currentTimeMillis())
        } else {
            dao.removeItem(collectionId, itemId, type.name)
        }
    }

    override fun observeItemIds(collectionId: String, type: CollectionItemType): Flow<List<String>> =
        dao.observeItemIds(collectionId, type.name)

    override fun observeCollectionsForItem(itemId: String, type: CollectionItemType): Flow<List<String>> =
        dao.observeCollectionsForItem(itemId, type.name)
}
