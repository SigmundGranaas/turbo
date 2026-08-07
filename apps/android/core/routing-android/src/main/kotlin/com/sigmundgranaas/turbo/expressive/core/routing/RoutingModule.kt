package com.sigmundgranaas.turbo.expressive.core.routing

import android.content.Context
import com.sigmundgranaas.turbo.expressive.core.data.RouteDiagnostics
import com.sigmundgranaas.turbo.expressive.core.data.RouteRepository
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.core.data.di.Remote
import com.sigmundgranaas.turbo.expressive.core.map.NetworkMonitor
import com.sigmundgranaas.turbo.expressive.domain.RoutingPack
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient
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

    /**
     * The pack builder writes into the same directory [PackStore] reads
     * and [com.sigmundgranaas.turbo.expressive.core.map.PackDownloader]
     * writes, so a pack cut here is found by exactly the same lookup as
     * one that was downloaded.
     */
    @Provides
    @Singleton
    fun provideDevicePackBuilder(
        @ApplicationContext context: Context,
        // The shared client. This used to construct `OkHttpClient()`
        // right here, under a comment claiming it did the opposite.
        http: OkHttpClient,
    ): DevicePackBuilder =
        DevicePackBuilder(
            root = File(context.filesDir, RoutingPack.DIR),
            newHttp = { OkHttpPackHttp(http) },
        )

    @Provides
    @Singleton
    fun provideDevicePackBuild(
        builder: DevicePackBuilder,
        settings: SettingsRepository,
    ): com.sigmundgranaas.turbo.expressive.core.map.WgpuOfflineTileManager.DevicePackBuild =
        DevicePackBuild(builder, settings)

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
        settings: SettingsRepository,
        diagnostics: RouteDiagnostics,
    ): RouteRepository = FallbackRouteRepository(
        server = server,
        device = device,
        // `NetworkMonitor.state` is a StateFlow, so this is a field read
        // rather than a probe — cheap enough to do per request, and it
        // reports *validated* connectivity, which is the only kind worth
        // asking about.
        isOnline = { network.state.value.connected },
        deviceCanAnswer = device::canPlanOffline,
        // Mirrored into a plain field so the routing path stays
        // synchronous. Collecting a Flow per request to read one enum
        // would put a suspension point in front of every route for a
        // value that changes when someone taps a radio button.
        engineChoice = { engineOverride },
        diagnostics = diagnostics,
        shadowCompare = { shadowCompare },
    ).also {
        scope.launch {
            settings.settings.collect {
                engineOverride = it.routeEngine
                shadowCompare = it.routeShadowCompare
            }
        }
    }

    /** Latest persisted shadow-comparison choice; see [provideRouteRepository]. */
    @Volatile
    private var shadowCompare: Boolean = false

    /** Latest persisted engine choice; see [provideRouteRepository]. */
    @Volatile
    private var engineOverride: com.sigmundgranaas.turbo.expressive.domain.RouteEngine =
        com.sigmundgranaas.turbo.expressive.domain.RouteEngine.Auto

    private val scope = kotlinx.coroutines.CoroutineScope(
        kotlinx.coroutines.SupervisorJob() + kotlinx.coroutines.Dispatchers.Default,
    )
}
