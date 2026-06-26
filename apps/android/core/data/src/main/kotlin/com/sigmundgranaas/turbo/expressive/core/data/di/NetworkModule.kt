package com.sigmundgranaas.turbo.expressive.core.data.di

import com.sigmundgranaas.turbo.expressive.core.data.HttpRadarRepository
import com.sigmundgranaas.turbo.expressive.core.data.RadarRepository
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import io.ktor.client.HttpClient
import io.ktor.client.engine.okhttp.OkHttp
import io.ktor.client.plugins.cache.HttpCache
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.serialization.kotlinx.json.json
import kotlinx.serialization.json.Json
import javax.inject.Singleton

/**
 * Invariant network wiring shared by every build variant: the [HttpClient] and
 * the always-live repositories.
 *
 * The repositories that have an offline *synthetic* stand-in (search, routing,
 * conditions, tides, …) are bound per-variant instead — see
 * `RemoteRepositoriesModule` in the `debug` and `release` source sets. The
 * synthetic impls live only in `src/debug`, so they never compile into a release
 * APK; release always binds the live HTTP impls.
 */
@Module
@InstallIn(SingletonComponent::class)
object NetworkModule {

    /**
     * Gridded weather for the cloud overlay (MET locationforecast samples).
     * Always the live impl — even in DEBUG — so the overlay shows real weather
     * where there's a network; the feature layer falls back to its synthetic
     * storm when a fetch fails (offline). No emulator stand-in, so it stays here
     * rather than in the per-variant module.
     */
    @Provides
    @Singleton
    fun provideRadarRepository(http: HttpRadarRepository): RadarRepository = http

    @Provides
    @Singleton
    fun provideHttpClient(): HttpClient = HttpClient(OkHttp) {
        install(ContentNegotiation) {
            json(Json { ignoreUnknownKeys = true })
        }
        // Honour MET/Varsom Expires + ETag so weather/forecast/avalanche reads
        // serve from cache within their validity window (and 304-revalidate after).
        install(HttpCache)
    }
}
