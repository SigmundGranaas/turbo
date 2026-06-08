package com.sigmundgranaas.turbo.expressive.core.data.di

import com.sigmundgranaas.turbo.expressive.core.data.ConditionsRepository
import com.sigmundgranaas.turbo.expressive.core.data.HttpConditionsRepository
import com.sigmundgranaas.turbo.expressive.core.data.GeonorgeTrailSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.HttpRouteRepository
import com.sigmundgranaas.turbo.expressive.core.data.KartverketReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.core.data.KartverketSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.ReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.core.data.RouteRepository
import com.sigmundgranaas.turbo.expressive.core.data.SearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.SyntheticConditionsRepository
import com.sigmundgranaas.turbo.expressive.core.data.SyntheticRouteRepository
import com.sigmundgranaas.turbo.expressive.core.data.TrailSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.BuildConfig
import dagger.Binds
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

@Module
@InstallIn(SingletonComponent::class)
abstract class NetworkModule {

    @Binds
    abstract fun bindSearchRepository(impl: KartverketSearchRepository): SearchRepository

    @Binds
    abstract fun bindTrailSearchRepository(impl: GeonorgeTrailSearchRepository): TrailSearchRepository

    @Binds
    abstract fun bindReverseGeocodeRepository(impl: KartverketReverseGeocodeRepository): ReverseGeocodeRepository

    companion object {
        /**
         * Pick the router: the real trail-bound SSE pathfinder in release, the offline
         * [SyntheticRouteRepository] in DEBUG so the Route builder + Follow can be driven
         * anywhere (the real router covers northern Norway only / isn't reachable from the
         * emulator). Flip [BuildConfig.DEBUG] off here if you need the real router in a
         * debug build on-coverage.
         */
        @Provides
        @Singleton
        fun provideRouteRepository(
            http: HttpRouteRepository,
            synthetic: SyntheticRouteRepository,
        ): RouteRepository = if (BuildConfig.DEBUG) synthetic else http

        /** Offline conditions stand-in in DEBUG (MET/Varsom unreachable on the emulator). */
        @Provides
        @Singleton
        fun provideConditionsRepository(
            http: HttpConditionsRepository,
            synthetic: SyntheticConditionsRepository,
        ): ConditionsRepository = if (BuildConfig.DEBUG) synthetic else http

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
}
