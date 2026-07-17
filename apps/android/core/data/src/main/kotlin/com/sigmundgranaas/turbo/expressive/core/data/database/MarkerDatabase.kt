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

/**
 * Sync metadata carried by every cloud-syncable row. Local-only until signed in:
 * [remoteId]/[version]/[updatedAtEpochMs] stay null and [dirty] is harmless. The
 * sync engine (P3) populates them on push/pull.
 */
@Entity(tableName = "marker")
data class MarkerEntity(
    @PrimaryKey val id: String,
    val name: String,
    val kind: String,
    val lat: Double,
    val lng: Double,
    val colorArgb: Long?,
    val notes: String? = null,
    // ── weather-pin fields (see MIGRATION_11_12) ──
    /** [MarkerKind] name — `"Standard"` for a plain pin, `"WeatherPin"` for a live weather node. */
    val markerKind: String = "Standard",
    /** JSON-encoded cached forecast for a weather pin (offline-safe render source); null otherwise. */
    val cachedForecast: String? = null,
    /** Epoch ms the [cachedForecast] was fetched — staleness + "updated Nh ago". */
    val forecastFetchedAtEpochMs: Long? = null,
    // ── sync fields ──
    /** Server-assigned id once pushed (the local [id] stays stable). */
    val remoteId: String? = null,
    /** Server row version for optimistic concurrency (If-Match). */
    val version: Long? = null,
    /** Last-modified epoch ms — local edit time, replaced by the server's on sync. */
    val updatedAtEpochMs: Long? = null,
    /** Tombstone: set when soft-deleted; null = live. */
    val deletedAtEpochMs: Long? = null,
    /** True when there are local changes pending upload. */
    val dirty: Boolean = true,
    /** Shared-with-us resource (read-only by convention): never pushed back to the server. */
    val readOnly: Boolean = false,
)

@Dao
interface MarkerDao {
    @Query("SELECT * FROM marker WHERE deletedAtEpochMs IS NULL ORDER BY name")
    fun observeAll(): Flow<List<MarkerEntity>>

    @Query("SELECT * FROM marker WHERE id = :id")
    suspend fun byId(id: String): MarkerEntity?

    @Query("SELECT * FROM marker WHERE remoteId = :remoteId")
    suspend fun byRemoteId(remoteId: String): MarkerEntity?

    /** Rows with local changes to push (creates/updates and pending deletes); read-only rows excluded. */
    @Query("SELECT * FROM marker WHERE dirty = 1 AND readOnly = 0")
    suspend fun pendingSync(): List<MarkerEntity>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(entity: MarkerEntity)

    /** Mark a pushed row as synced: record the server id/version and clear the dirty flag. */
    @Query("UPDATE marker SET remoteId = :remoteId, version = :version, updatedAtEpochMs = :updatedAt, dirty = 0 WHERE id = :id")
    suspend fun markSynced(id: String, remoteId: String, version: Long, updatedAt: Long)

    /** Tombstone a synced row so the engine can push the delete. */
    @Query("UPDATE marker SET deletedAtEpochMs = :ts, dirty = 1 WHERE id = :id")
    suspend fun softDelete(id: String, ts: Long)

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
    /** When this track came from a Follow (D1): the planned guide it followed, encoded
     *  like [points] ("lat,lng;…"); null for plain recordings. */
    val plannedRoute: String? = null,
    /** Checkpoint splits recorded while following (D1), JSON-encoded; null/blank when none. */
    val phaseSplits: String? = null,
    /** Display colour "#RRGGBB"; null = default. Mirrors the wire `colorHex`. */
    val colorHex: String? = null,
    /** Track icon key; pass-through of the wire `iconKey` (web edits it). */
    val iconKey: String? = null,
    /** Line-style key (solid/dotted/dashed/dash_dot); wire pass-through. */
    val lineStyleKey: String? = null,
    // ── sync fields (see [MarkerEntity]) ──
    val remoteId: String? = null,
    val version: Long? = null,
    val updatedAtEpochMs: Long? = null,
    val deletedAtEpochMs: Long? = null,
    val dirty: Boolean = true,
    val readOnly: Boolean = false,
)

@Dao
interface PathDao {
    @Query("SELECT * FROM path WHERE deletedAtEpochMs IS NULL ORDER BY createdAtEpochMs DESC")
    fun observeAll(): Flow<List<PathEntity>>

    @Query("SELECT * FROM path WHERE id = :id")
    suspend fun byId(id: String): PathEntity?

    @Query("SELECT * FROM path WHERE remoteId = :remoteId")
    suspend fun byRemoteId(remoteId: String): PathEntity?

    @Query("SELECT * FROM path WHERE dirty = 1")
    suspend fun pendingSync(): List<PathEntity>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(entity: PathEntity)

    /** Mark a pushed row as synced: record the server id/version and clear the dirty flag. */
    @Query("UPDATE path SET remoteId = :remoteId, version = :version, updatedAtEpochMs = :updatedAt, dirty = 0 WHERE id = :id")
    suspend fun markSynced(id: String, remoteId: String, version: Long, updatedAt: Long)

    @Query("UPDATE path SET deletedAtEpochMs = :ts, dirty = 1 WHERE id = :id")
    suspend fun softDelete(id: String, ts: Long)

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
    // ── sync fields (see [MarkerEntity]) ──
    val remoteId: String? = null,
    val version: Long? = null,
    val updatedAtEpochMs: Long? = null,
    val deletedAtEpochMs: Long? = null,
    val dirty: Boolean = true,
    val readOnly: Boolean = false,
)

/** Membership row linking an entity (marker/path) to a collection. */
@Entity(tableName = "collection_item", primaryKeys = ["collectionId", "itemId", "itemType"])
data class CollectionItemEntity(
    val collectionId: String,
    val itemId: String,
    val itemType: String,
    /** True when this membership add/remove still needs pushing to the server. */
    val dirty: Boolean = true,
    /** Tombstone: set when removed from a synced collection; null = live. */
    val deletedAtEpochMs: Long? = null,
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
            "(SELECT COUNT(*) FROM collection_item ci WHERE ci.collectionId = c.id AND ci.deletedAtEpochMs IS NULL) AS itemCount " +
            "FROM collection c WHERE c.deletedAtEpochMs IS NULL ORDER BY c.name",
    )
    fun observeAll(): Flow<List<CollectionWithCount>>

    @Query("SELECT * FROM collection WHERE id = :id")
    suspend fun byId(id: String): CollectionEntity?

    @Query("SELECT * FROM collection WHERE remoteId = :remoteId")
    suspend fun byRemoteId(remoteId: String): CollectionEntity?

    /** Synced, live collections — used to push their membership to the server. */
    @Query("SELECT * FROM collection WHERE remoteId IS NOT NULL AND deletedAtEpochMs IS NULL")
    suspend fun syncedCollections(): List<CollectionEntity>

    /** Snapshot of a collection's membership rows (for sync). */
    @Query("SELECT * FROM collection_item WHERE collectionId = :collectionId")
    suspend fun itemsForCollection(collectionId: String): List<CollectionItemEntity>

    @Query("SELECT * FROM collection WHERE dirty = 1 AND readOnly = 0")
    suspend fun pendingSync(): List<CollectionEntity>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(entity: CollectionEntity)

    /** Mark a pushed row as synced: record the server id/version and clear the dirty flag. */
    @Query("UPDATE collection SET remoteId = :remoteId, version = :version, updatedAtEpochMs = :updatedAt, dirty = 0 WHERE id = :id")
    suspend fun markSynced(id: String, remoteId: String, version: Long, updatedAt: Long)

    @Query("UPDATE collection SET deletedAtEpochMs = :ts, dirty = 1 WHERE id = :id")
    suspend fun softDelete(id: String, ts: Long)

    @Query("DELETE FROM collection WHERE id = :id")
    suspend fun delete(id: String)

    @Query("DELETE FROM collection_item WHERE collectionId = :id")
    suspend fun clearItems(id: String)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun addItem(item: CollectionItemEntity)

    @Query("DELETE FROM collection_item WHERE collectionId = :collectionId AND itemId = :itemId AND itemType = :itemType")
    suspend fun removeItem(collectionId: String, itemId: String, itemType: String)

    /** Tombstone a membership of a synced collection so the engine pushes the DELETE. */
    @Query("UPDATE collection_item SET deletedAtEpochMs = :ts, dirty = 1 WHERE collectionId = :collectionId AND itemId = :itemId AND itemType = :itemType")
    suspend fun tombstoneItem(collectionId: String, itemId: String, itemType: String, ts: Long)

    /** Clear the dirty flag after a membership add/remove has been pushed. */
    @Query("UPDATE collection_item SET dirty = 0 WHERE collectionId = :collectionId AND itemId = :itemId AND itemType = :itemType")
    suspend fun markItemSynced(collectionId: String, itemId: String, itemType: String)

    @Query("SELECT itemId FROM collection_item WHERE collectionId = :collectionId AND itemType = :itemType AND deletedAtEpochMs IS NULL")
    fun observeItemIds(collectionId: String, itemType: String): Flow<List<String>>

    @Query("SELECT collectionId FROM collection_item WHERE itemId = :itemId AND itemType = :itemType AND deletedAtEpochMs IS NULL")
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
    version = 12,
    exportSchema = false,
)
abstract class TurboDatabase : RoomDatabase() {
    abstract fun markerDao(): MarkerDao
    abstract fun pathDao(): PathDao
    abstract fun collectionDao(): CollectionDao
    abstract fun photoDao(): PhotoDao
}

/**
 * v11 → v12: weather pins. Adds the [MarkerEntity.markerKind] discriminator plus the
 * cached-forecast columns to the `marker` table **without wiping data** — existing pins
 * default to `"Standard"` with no cached forecast. `ADD COLUMN` is the whole change.
 */
val MIGRATION_11_12 = androidx.room.migration.Migration(11, 12) { db ->
    db.execSQL("ALTER TABLE marker ADD COLUMN markerKind TEXT NOT NULL DEFAULT 'Standard'")
    db.execSQL("ALTER TABLE marker ADD COLUMN cachedForecast TEXT")
    db.execSQL("ALTER TABLE marker ADD COLUMN forecastFetchedAtEpochMs INTEGER")
}
