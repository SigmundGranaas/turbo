package com.sigmundgranaas.turbo.expressive.core.data.di

import android.content.Context
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.sqlite.db.SupportSQLiteDatabase
import com.sigmundgranaas.turbo.expressive.core.common.StringProvider
import com.sigmundgranaas.turbo.expressive.core.data.AndroidLocationRepository
import com.sigmundgranaas.turbo.expressive.core.data.AndroidStringProvider
import com.sigmundgranaas.turbo.expressive.core.data.CollectionRepository
import com.sigmundgranaas.turbo.expressive.core.data.DataStoreSettingsRepository
import com.sigmundgranaas.turbo.expressive.core.data.PhotoRepository
import com.sigmundgranaas.turbo.expressive.core.data.RoomCollectionRepository
import com.sigmundgranaas.turbo.expressive.core.data.RoomPhotoRepository
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionDao
import com.sigmundgranaas.turbo.expressive.core.data.database.PhotoDao
import com.sigmundgranaas.turbo.expressive.core.data.LocationRepository
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.core.data.DataStoreRecentSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.DataStoreRecordingDraftStore
import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.data.RecentSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.RecordingDraftStore
import com.sigmundgranaas.turbo.expressive.core.data.RoomMarkerRepository
import com.sigmundgranaas.turbo.expressive.core.data.RoomPathRepository
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.core.data.SyncCursorStore
import com.sigmundgranaas.turbo.expressive.core.data.DataStoreSyncCursorStore
import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerDao
import com.sigmundgranaas.turbo.expressive.core.data.database.PathDao
import com.sigmundgranaas.turbo.expressive.core.data.database.TurboDatabase
import com.sigmundgranaas.turbo.expressive.domain.SampleData
import dagger.Binds
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
abstract class DataModule {

    @Binds
    abstract fun bindStringProvider(impl: AndroidStringProvider): StringProvider

    @Binds
    abstract fun bindMarkerRepository(impl: RoomMarkerRepository): MarkerRepository

    @Binds
    abstract fun bindSettingsRepository(impl: DataStoreSettingsRepository): SettingsRepository

    @Binds
    abstract fun bindPathRepository(impl: RoomPathRepository): PathRepository

    @Binds
    abstract fun bindLocationRepository(impl: AndroidLocationRepository): LocationRepository

    @Binds
    abstract fun bindRecordingDraftStore(impl: DataStoreRecordingDraftStore): RecordingDraftStore

    @Binds
    abstract fun bindRecentSearchRepository(impl: DataStoreRecentSearchRepository): RecentSearchRepository

    @Binds
    abstract fun bindCollectionRepository(impl: RoomCollectionRepository): CollectionRepository

    @Binds
    abstract fun bindPhotoRepository(impl: RoomPhotoRepository): PhotoRepository

    @Binds
    abstract fun bindSyncCursorStore(impl: DataStoreSyncCursorStore): SyncCursorStore

    companion object {
        @Provides
        @Singleton
        fun provideDatabase(@ApplicationContext context: Context): TurboDatabase =
            Room.databaseBuilder(context, TurboDatabase::class.java, "turbo.db")
                .fallbackToDestructiveMigration(dropAllTables = true)
                .addCallback(object : RoomDatabase.Callback() {
                    override fun onCreate(db: SupportSQLiteDatabase) {
                        // Seed the sample markers so the map opens populated.
                        SampleData.markers.forEach { m ->
                            // dirty = 0: seed data is a clean local baseline, never pushed to the cloud.
                            db.execSQL(
                                "INSERT INTO marker (id, name, kind, lat, lng, colorArgb, dirty) VALUES (?, ?, ?, ?, ?, ?, 0)",
                                arrayOf<Any?>(m.id, m.name, m.kind.key, m.position.lat, m.position.lng, m.colorArgb),
                            )
                        }
                    }
                })
                .build()

        /** Application-lifetime scope for engines that outlive any screen (e.g. recording). */
        @Provides
        @Singleton
        fun provideAppScope(): kotlinx.coroutines.CoroutineScope =
            kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.SupervisorJob() + kotlinx.coroutines.Dispatchers.Default)

        @Provides
        fun provideMarkerDao(db: TurboDatabase): MarkerDao = db.markerDao()

        @Provides
        fun providePathDao(db: TurboDatabase): PathDao = db.pathDao()

        @Provides
        fun provideCollectionDao(db: TurboDatabase): CollectionDao = db.collectionDao()

        @Provides
        fun providePhotoDao(db: TurboDatabase): PhotoDao = db.photoDao()
    }
}
