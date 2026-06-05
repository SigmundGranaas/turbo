package com.sigmundgranaas.turbo.expressive.feature.recording

import android.content.Context
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Inject

/**
 * Seam for starting/stopping the foreground recording service. Lets the
 * ViewModel drive recording without holding a Context or touching the service
 * directly, so it stays a pure-JVM unit-test target.
 */
interface RecordingLauncher {
    fun start()
    fun stop()
}

class ServiceRecordingLauncher @Inject constructor(
    @param:ApplicationContext private val context: Context,
) : RecordingLauncher {
    override fun start() = RecordingService.start(context)
    override fun stop() = RecordingService.stop(context)
}

@Module
@InstallIn(SingletonComponent::class)
abstract class RecordingModule {
    @Binds
    abstract fun bindRecordingLauncher(impl: ServiceRecordingLauncher): RecordingLauncher
}
