package com.sigmundgranaas.turbo.expressive.feature.offline

import android.content.Context
import com.sigmundgranaas.turbo.expressive.core.map.OfflineServiceLauncher
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

/**
 * Binds the [OfflineServiceLauncher] the (core:map) tile manager calls to start the
 * foreground download service, which lives in this feature module so it can see both
 * the manager and the settings/network it coordinates.
 */
@Module
@InstallIn(SingletonComponent::class)
object OfflineServiceModule {
    @Provides
    @Singleton
    fun provideOfflineServiceLauncher(@ApplicationContext context: Context): OfflineServiceLauncher =
        OfflineServiceLauncher { OfflineDownloadService.start(context) }
}
