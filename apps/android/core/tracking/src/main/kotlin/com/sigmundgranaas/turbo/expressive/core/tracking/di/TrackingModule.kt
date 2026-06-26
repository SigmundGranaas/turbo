package com.sigmundgranaas.turbo.expressive.core.tracking.di

import com.sigmundgranaas.turbo.expressive.core.tracking.AndroidLocationRepository
import com.sigmundgranaas.turbo.expressive.core.tracking.DataStoreRecordingDraftStore
import com.sigmundgranaas.turbo.expressive.core.tracking.LocationRepository
import com.sigmundgranaas.turbo.expressive.core.tracking.RecordingDraftStore
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent

/**
 * Bindings for the live tracking runtime (location streaming + the recording
 * draft store). The stateful controllers ([FollowController],
 * [RecordingController], [CaptureSession], …) are `@Inject`-constructed, so only
 * the two interface-backed seams need binding here.
 */
@Module
@InstallIn(SingletonComponent::class)
abstract class TrackingModule {

    @Binds
    abstract fun bindLocationRepository(impl: AndroidLocationRepository): LocationRepository

    @Binds
    abstract fun bindRecordingDraftStore(impl: DataStoreRecordingDraftStore): RecordingDraftStore
}
