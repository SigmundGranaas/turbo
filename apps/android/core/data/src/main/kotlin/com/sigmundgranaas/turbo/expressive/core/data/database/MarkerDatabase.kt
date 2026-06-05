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

@Database(entities = [MarkerEntity::class, PathEntity::class], version = 3, exportSchema = false)
abstract class TurboDatabase : RoomDatabase() {
    abstract fun markerDao(): MarkerDao
    abstract fun pathDao(): PathDao
}
