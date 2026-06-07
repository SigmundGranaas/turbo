package com.sigmundgranaas.turbo.expressive.core.sync.di

import com.sigmundgranaas.turbo.expressive.core.sync.CollectionRemote
import com.sigmundgranaas.turbo.expressive.core.sync.CollectionSyncApi
import com.sigmundgranaas.turbo.expressive.core.sync.CollectionSyncer
import com.sigmundgranaas.turbo.expressive.core.sync.DomainSyncer
import com.sigmundgranaas.turbo.expressive.core.sync.LocationRemote
import com.sigmundgranaas.turbo.expressive.core.sync.LocationSyncApi
import com.sigmundgranaas.turbo.expressive.core.sync.MarkerSyncer
import com.sigmundgranaas.turbo.expressive.core.sync.SyncClient
import com.sigmundgranaas.turbo.expressive.core.sync.SyncController
import com.sigmundgranaas.turbo.expressive.core.sync.SyncEngine
import com.sigmundgranaas.turbo.expressive.core.sync.TrackRemote
import com.sigmundgranaas.turbo.expressive.core.sync.TrackSyncApi
import com.sigmundgranaas.turbo.expressive.core.sync.TrackSyncer
import dagger.Binds
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet
import io.ktor.client.HttpClient
import io.ktor.client.engine.okhttp.OkHttp
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.serialization.kotlinx.json.json
import kotlinx.serialization.json.Json
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
abstract class SyncModule {

    @Binds
    abstract fun bindSyncController(impl: SyncEngine): SyncController

    @Binds
    abstract fun bindTrackRemote(impl: TrackSyncApi): TrackRemote

    @Binds
    @IntoSet
    abstract fun bindTrackSyncer(impl: TrackSyncer): DomainSyncer

    @Binds
    abstract fun bindLocationRemote(impl: LocationSyncApi): LocationRemote

    @Binds
    @IntoSet
    abstract fun bindMarkerSyncer(impl: MarkerSyncer): DomainSyncer

    @Binds
    abstract fun bindCollectionRemote(impl: CollectionSyncApi): CollectionRemote

    @Binds
    @IntoSet
    abstract fun bindCollectionSyncer(impl: CollectionSyncer): DomainSyncer

    companion object {
        @Provides
        @Singleton
        @SyncClient
        fun provideSyncHttpClient(): HttpClient = HttpClient(OkHttp) {
            expectSuccess = false
            install(ContentNegotiation) {
                json(
                    Json {
                        ignoreUnknownKeys = true
                        explicitNulls = false
                        isLenient = true
                    },
                )
            }
        }
    }
}
