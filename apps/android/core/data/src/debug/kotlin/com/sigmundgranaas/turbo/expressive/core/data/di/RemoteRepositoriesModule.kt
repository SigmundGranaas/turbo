package com.sigmundgranaas.turbo.expressive.core.data.di

import android.os.Build
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
import com.sigmundgranaas.turbo.expressive.core.data.SyntheticConditionsRepository
import com.sigmundgranaas.turbo.expressive.core.data.SyntheticReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.core.data.SyntheticRouteRepository
import com.sigmundgranaas.turbo.expressive.core.data.SyntheticSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.SyntheticTideRepository
import com.sigmundgranaas.turbo.expressive.core.data.SyntheticTrailSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.TideRepository
import com.sigmundgranaas.turbo.expressive.core.data.TrailSearchRepository
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

/**
 * Debug wiring for the networked, Norway-only backends.
 *
 * Every such backend has a synthetic stand-in so the app is driveable on the
 * emulator / a dev box with no network — that's the ONLY reason they exist, so
 * gate on "running on an emulator", NOT merely on a debug build. A debug build
 * dogfooded on a real device must hit the live backends (else the route is a
 * straight line, search is fake, etc.).
 *
 * This whole module — and the `Synthetic*` impls it references — lives only in
 * `src/debug`, so it cannot leak into a release build (see the `release`
 * variant's HTTP-only copy).
 */
@Module
@InstallIn(SingletonComponent::class)
object RemoteRepositoriesModule {

    /** Heuristic emulator check (no Context needed) — Android Studio AVD, Genymotion. */
    private fun isEmulator(): Boolean =
        Build.FINGERPRINT.startsWith("generic") ||
            Build.FINGERPRINT.startsWith("unknown") ||
            Build.FINGERPRINT.contains("emulator", ignoreCase = true) ||
            Build.MODEL.contains("Emulator", ignoreCase = true) ||
            Build.MODEL.contains("Android SDK built for", ignoreCase = true) ||
            Build.MANUFACTURER.contains("Genymotion", ignoreCase = true) ||
            Build.HARDWARE.contains("goldfish") ||
            Build.HARDWARE.contains("ranchu") ||
            Build.PRODUCT.contains("sdk")

    private val useSynthetic: Boolean = isEmulator()

    @Provides
    @Singleton
    fun provideSearchRepository(
        http: KartverketSearchRepository,
        synthetic: SyntheticSearchRepository,
    ): SearchRepository = if (useSynthetic) synthetic else http

    @Provides
    @Singleton
    fun provideTrailSearchRepository(
        http: GeonorgeTrailSearchRepository,
        synthetic: SyntheticTrailSearchRepository,
    ): TrailSearchRepository = if (useSynthetic) synthetic else http

    @Provides
    @Singleton
    fun provideReverseGeocodeRepository(
        http: KartverketReverseGeocodeRepository,
        synthetic: SyntheticReverseGeocodeRepository,
    ): ReverseGeocodeRepository = if (useSynthetic) synthetic else http

    /**
     * Pick the router: the real trail-bound SSE pathfinder on any real device,
     * the offline [SyntheticRouteRepository] only on the emulator (where the
     * router isn't reachable), so the Route builder + Follow can still be driven
     * there.
     */
    @Provides
    @Singleton
    fun provideRouteRepository(
        http: HttpRouteRepository,
        synthetic: SyntheticRouteRepository,
    ): RouteRepository = if (useSynthetic) synthetic else http

    /** Offline conditions stand-in on the emulator (MET/Varsom unreachable there). */
    @Provides
    @Singleton
    fun provideConditionsRepository(
        http: HttpConditionsRepository,
        synthetic: SyntheticConditionsRepository,
    ): ConditionsRepository = if (useSynthetic) synthetic else http

    /** Tide predictions (Kartverket sehavniva); synthetic on the emulator. */
    @Provides
    @Singleton
    fun provideTideRepository(
        http: KartverketTideRepository,
        synthetic: SyntheticTideRepository,
    ): TideRepository = if (useSynthetic) synthetic else http
}
