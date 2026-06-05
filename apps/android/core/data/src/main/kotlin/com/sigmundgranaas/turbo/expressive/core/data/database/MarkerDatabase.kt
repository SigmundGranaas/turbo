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

@Database(entities = [MarkerEntity::class], version = 1, exportSchema = false)
abstract class TurboDatabase : RoomDatabase() {
    abstract fun markerDao(): MarkerDao
}
