package com.sigmundgranaas.turbo.expressive.core.data.di

import com.sigmundgranaas.turbo.expressive.core.data.ConditionsRepository
import com.sigmundgranaas.turbo.expressive.core.data.GeonorgeTrailSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.HttpConditionsRepository
import com.sigmundgranaas.turbo.expressive.core.data.HttpRouteRepository
import com.sigmundgranaas.turbo.expressive.core.data.KartverketReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.core.data.KartverketSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.KartverketTideRepository
import com.sigmundgranaas.turbo.expressive.core.data.ReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.core.data.RouteRepository
import com.sigmundgranaas.turbo.expressive.core.data.SearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.TideRepository
import com.sigmundgranaas.turbo.expressive.core.data.TrailSearchRepository
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

/**
 * Release wiring for the networked, Norway-only backends: always the live HTTP
 * impls. The offline synthetic stand-ins exist only in the `debug` source set —
 * see the `debug` variant's copy of this module — so a release build can neither
 * reference nor ship them.
 */
@Module
@InstallIn(SingletonComponent::class)
object RemoteRepositoriesModule {

    @Provides
    @Singleton
    fun provideSearchRepository(http: KartverketSearchRepository): SearchRepository = http

    @Provides
    @Singleton
    fun provideTrailSearchRepository(http: GeonorgeTrailSearchRepository): TrailSearchRepository = http

    @Provides
    @Singleton
    fun provideReverseGeocodeRepository(http: KartverketReverseGeocodeRepository): ReverseGeocodeRepository = http

    @Provides
    @Singleton
    fun provideRouteRepository(http: HttpRouteRepository): RouteRepository = http

    @Provides
    @Singleton
    fun provideConditionsRepository(http: HttpConditionsRepository): ConditionsRepository = http

    @Provides
    @Singleton
    fun provideTideRepository(http: KartverketTideRepository): TideRepository = http
}
