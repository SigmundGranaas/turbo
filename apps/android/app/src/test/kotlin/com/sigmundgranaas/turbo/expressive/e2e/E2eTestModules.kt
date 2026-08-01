package com.sigmundgranaas.turbo.expressive.e2e

import com.sigmundgranaas.turbo.expressive.core.data.ConditionsRepository
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
import com.sigmundgranaas.turbo.expressive.core.data.di.Remote
import com.sigmundgranaas.turbo.expressive.core.data.di.RemoteRepositoriesModule
import dagger.Module
import dagger.Provides
import dagger.hilt.components.SingletonComponent
import dagger.hilt.testing.TestInstallIn
import javax.inject.Singleton

/**
 * Deterministic, offline backends for E2E — no network in tests. Replaces the
 * debug [RemoteRepositoriesModule] (which only uses the synthetics on an
 * emulator). The `Synthetic*` impls already live in `core:data/src/debug`.
 *
 * The renderer is faked differently — not through Hilt — because it's a Compose
 * `CompositionLocal` (`LocalMapEngineOverride`), provided directly in each test's
 * `setContent`.
 */
@Module
@TestInstallIn(components = [SingletonComponent::class], replaces = [RemoteRepositoriesModule::class])
object FakeRemoteRepositoriesModule {
    @Provides @Singleton
    fun search(impl: SyntheticSearchRepository): SearchRepository = impl

    @Provides @Singleton
    fun trailSearch(impl: SyntheticTrailSearchRepository): TrailSearchRepository = impl

    @Provides @Singleton
    fun reverseGeocode(impl: SyntheticReverseGeocodeRepository): ReverseGeocodeRepository = impl

    /**
     * The **server** half, not the whole binding.
     *
     * `@Remote` matters and dropping it is what broke this graph once.
     * `:core:routing-android` binds the plain `RouteRepository` to
     * `FallbackRouteRepository`, which asks for `@Remote` as its server
     * side. Providing the synthetic unqualified made two modules claim
     * the same key — a duplicate binding — while leaving the qualified
     * one nobody satisfied.
     *
     * Qualified, the fallback wiring stays in the test graph and the
     * synthetic stands in for the network, which is what these tests
     * actually want. No pack is installed under test, so
     * `deviceCanAnswer` is false and every request goes straight here:
     * same behaviour as before, one more layer genuinely covered.
     */
    @Provides @Singleton
    @Remote
    fun route(impl: SyntheticRouteRepository): RouteRepository = impl

    @Provides @Singleton
    fun conditions(impl: SyntheticConditionsRepository): ConditionsRepository = impl

    @Provides @Singleton
    fun tide(impl: SyntheticTideRepository): TideRepository = impl
}
