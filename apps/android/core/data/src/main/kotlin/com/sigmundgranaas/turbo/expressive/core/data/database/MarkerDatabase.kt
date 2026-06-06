package com.sigmundgranaas.turbo.expressive.core.data.database

import androidx.room.Dao
import androidx.room.Database
import androidx.room.Entity
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.PrimaryKey
import androidx.room.Query
import androidx.room.RoomDatabase
import kotlinx.coroutines.flow.Flow

@Entity(tableName = "marker")
data class MarkerEntity(
    @PrimaryKey val id: String,
    val name: String,
    val kind: String,
    val lat: Double,
    val lng: Double,
    val colorArgb: Long?,
    val notes: String? = null,
)

@Dao
interface MarkerDao {
    @Query("SELECT * FROM marker ORDER BY name")
    fun observeAll(): Flow<List<MarkerEntity>>

    @Query("SELECT * FROM marker WHERE id = :id")
    suspend fun byId(id: String): MarkerEntity?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(entity: MarkerEntity)

    @Query("DELETE FROM marker WHERE id = :id")
    suspend fun delete(id: String)
}

@Entity(tableName = "path")
data class PathEntity(
    @PrimaryKey val id: String,
    val name: String,
    val source: String,
    /** Points encoded as "lat,lng;lat,lng;…". */
    val points: String,
    val distanceM: Double,
    val ascentM: Double?,
    val descentM: Double?,
    val durationSec: Int?,
    val createdAtEpochMs: Long,
    /** Per-point altitude (m), parallel to [points], encoded as ";"-joined; empty = none. */
    val elevations: String? = null,
    /** Activity kind tag (ActivityKindId.name), null when untagged. */
    val activityKind: String? = null,
)

@Dao
interface PathDao {
    @Query("SELECT * FROM path ORDER BY createdAtEpochMs DESC")
    fun observeAll(): Flow<List<PathEntity>>

    @Query("SELECT * FROM path WHERE id = :id")
    suspend fun byId(id: String): PathEntity?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(entity: PathEntity)

    @Query("DELETE FROM path WHERE id = :id")
    suspend fun delete(id: String)
}

@Entity(tableName = "collection")
data class CollectionEntity(
    @PrimaryKey val id: String,
    val name: String,
    val colorArgb: Long?,
    val icon: String?,
    val createdAtEpochMs: Long,
)

/** Membership row linking an entity (marker/path) to a collection. */
@Entity(tableName = "collection_item", primaryKeys = ["collectionId", "itemId", "itemType"])
data class CollectionItemEntity(
    val collectionId: String,
    val itemId: String,
    val itemType: String,
)

/** Projection of a collection plus its current membership count. */
data class CollectionWithCount(
    val id: String,
    val name: String,
    val colorArgb: Long?,
    val icon: String?,
    val itemCount: Int,
)

@Dao
interface CollectionDao {
    @Query(
        "SELECT c.id, c.name, c.colorArgb, c.icon, " +
            "(SELECT COUNT(*) FROM collection_item ci WHERE ci.collectionId = c.id) AS itemCount " +
            "FROM collection c ORDER BY c.name",
    )
    fun observeAll(): Flow<List<CollectionWithCount>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(entity: CollectionEntity)

    @Query("DELETE FROM collection WHERE id = :id")
    suspend fun delete(id: String)

    @Query("DELETE FROM collection_item WHERE collectionId = :id")
    suspend fun clearItems(id: String)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun addItem(item: CollectionItemEntity)

    @Query("DELETE FROM collection_item WHERE collectionId = :collectionId AND itemId = :itemId AND itemType = :itemType")
    suspend fun removeItem(collectionId: String, itemId: String, itemType: String)

    @Query("SELECT itemId FROM collection_item WHERE collectionId = :collectionId AND itemType = :itemType")
    fun observeItemIds(collectionId: String, itemType: String): Flow<List<String>>

    @Query("SELECT collectionId FROM collection_item WHERE itemId = :itemId AND itemType = :itemType")
    fun observeCollectionsForItem(itemId: String, itemType: String): Flow<List<String>>
}

@Entity(tableName = "photo")
data class PhotoEntity(
    @PrimaryKey val id: String,
    val markerId: String?,
    val lat: Double,
    val lng: Double,
    val uri: String,
    val capturedAtEpochMs: Long,
)

@Dao
interface PhotoDao {
    @Query("SELECT * FROM photo ORDER BY capturedAtEpochMs DESC")
    fun observeAll(): Flow<List<PhotoEntity>>

    @Query("SELECT * FROM photo WHERE markerId = :markerId ORDER BY capturedAtEpochMs DESC")
    fun observeForMarker(markerId: String): Flow<List<PhotoEntity>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(entity: PhotoEntity)

    @Query("DELETE FROM photo WHERE id = :id")
    suspend fun delete(id: String)
}

@Database(
    entities = [
        MarkerEntity::class, PathEntity::class,
        CollectionEntity::class, CollectionItemEntity::class, PhotoEntity::class,
    ],
    version = 7,
    exportSchema = false,
)
abstract class TurboDatabase : RoomDatabase() {
    abstract fun markerDao(): MarkerDao
    abstract fun pathDao(): PathDao
    abstract fun collectionDao(): CollectionDao
    abstract fun photoDao(): PhotoDao
}
