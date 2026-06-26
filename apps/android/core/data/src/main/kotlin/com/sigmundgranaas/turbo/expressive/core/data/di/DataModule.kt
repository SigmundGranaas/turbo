package com.sigmundgranaas.turbo.expressive.core.data.di

import android.content.Context
import androidx.room.Room
import com.sigmundgranaas.turbo.expressive.core.common.StringProvider
import com.sigmundgranaas.turbo.expressive.core.data.AndroidStringProvider
import com.sigmundgranaas.turbo.expressive.core.data.CollectionRepository
import com.sigmundgranaas.turbo.expressive.core.data.DataStoreSettingsRepository
import com.sigmundgranaas.turbo.expressive.core.data.PhotoRepository
import com.sigmundgranaas.turbo.expressive.core.data.RoomCollectionRepository
import com.sigmundgranaas.turbo.expressive.core.data.RoomPhotoRepository
import com.sigmundgranaas.turbo.expressive.core.data.database.CollectionDao
import com.sigmundgranaas.turbo.expressive.core.data.database.PhotoDao
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.core.data.DataStoreRecentSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.data.RecentSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.RoomMarkerRepository
import com.sigmundgranaas.turbo.expressive.core.data.RoomPathRepository
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.core.data.SyncCursorStore
import com.sigmundgranaas.turbo.expressive.core.data.DataStoreSyncCursorStore
import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerDao
import com.sigmundgranaas.turbo.expressive.core.data.database.PathDao
import com.sigmundgranaas.turbo.expressive.core.data.database.TurboDatabase
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
            // Starts empty: the map opens on the user's real GPS location, and markers
            // are created by the user (or arrive via sync) — no seeded sample content.
            Room.databaseBuilder(context, TurboDatabase::class.java, "turbo.db")
                .fallbackToDestructiveMigration(dropAllTables = true)
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
