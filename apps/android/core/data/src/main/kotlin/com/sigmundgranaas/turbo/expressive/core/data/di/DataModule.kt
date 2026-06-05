package com.sigmundgranaas.turbo.expressive.core.data.di

import android.content.Context
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.sqlite.db.SupportSQLiteDatabase
import com.sigmundgranaas.turbo.expressive.core.data.DataStoreSettingsRepository
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.core.data.RoomMarkerRepository
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.core.data.database.MarkerDao
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
    abstract fun bindMarkerRepository(impl: RoomMarkerRepository): MarkerRepository

    @Binds
    abstract fun bindSettingsRepository(impl: DataStoreSettingsRepository): SettingsRepository

    companion object {
        @Provides
        @Singleton
        fun provideDatabase(@ApplicationContext context: Context): TurboDatabase =
            Room.databaseBuilder(context, TurboDatabase::class.java, "turbo.db")
                .addCallback(object : RoomDatabase.Callback() {
                    override fun onCreate(db: SupportSQLiteDatabase) {
                        // Seed the sample markers so the map opens populated.
                        SampleData.markers.forEach { m ->
                            db.execSQL(
                                "INSERT INTO marker (id, name, kind, lat, lng, colorArgb) VALUES (?, ?, ?, ?, ?, ?)",
                                arrayOf<Any?>(m.id, m.name, m.kind.key, m.position.lat, m.position.lng, m.colorArgb),
                            )
                        }
                    }
                })
                .build()

        @Provides
        fun provideMarkerDao(db: TurboDatabase): MarkerDao = db.markerDao()
    }
}
