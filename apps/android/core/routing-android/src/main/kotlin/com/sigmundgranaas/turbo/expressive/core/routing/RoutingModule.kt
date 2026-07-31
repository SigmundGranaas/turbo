package com.sigmundgranaas.turbo.expressive.core.routing

import android.content.Context
import com.sigmundgranaas.turbo.expressive.core.data.RouteRepository
import com.sigmundgranaas.turbo.expressive.core.data.di.Remote
import com.sigmundgranaas.turbo.expressive.core.map.NetworkMonitor
import com.sigmundgranaas.turbo.expressive.domain.RoutingPack
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import java.io.File
import javax.inject.Singleton

/**
 * Binds routing to the server, with the phone behind it.
 *
 * This replaces the plain `provideRouteRepository(http)` binding in
 * `:core:data`. Nothing above the `RouteRepository` interface changes —
 * `RouteViewModel`, the route card, follow mode — which is the point of
 * that interface having existed before any of this.
 */
@Module
@InstallIn(SingletonComponent::class)
object RoutingModule {

    @Provides
    @Singleton
    fun providePackStore(@ApplicationContext context: Context): PackStore =
        PackStore(File(context.filesDir, RoutingPack.DIR))

    @Provides
    @Singleton
    fun provideOnDeviceRouteRepository(packs: PackStore): OnDeviceRouteRepository =
        OnDeviceRouteRepository(packs)

    @Provides
    @Singleton
    fun provideOfflineRoutingCoverage(
        device: OnDeviceRouteRepository,
    ): com.sigmundgranaas.turbo.expressive.core.data.OfflineRoutingCoverage = device

    @Provides
    @Singleton
    fun provideRouteRepository(
        @Remote server: RouteRepository,
        device: OnDeviceRouteRepository,
        network: NetworkMonitor,
    ): RouteRepository = FallbackRouteRepository(
        server = server,
        device = device,
        // `NetworkMonitor.state` is a StateFlow, so this is a field read
        // rather than a probe — cheap enough to do per request, and it
        // reports *validated* connectivity, which is the only kind worth
        // asking about.
        isOnline = { network.state.value.connected },
        deviceCanAnswer = device::canPlanOffline,
    )
}
