package com.sigmundgranaas.turbo.expressive.core.data.di

import android.os.Build
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
import com.sigmundgranaas.turbo.expressive.core.data.SyntheticReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.core.data.SyntheticRouteRepository
import com.sigmundgranaas.turbo.expressive.core.data.SyntheticSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.SyntheticTideRepository
import com.sigmundgranaas.turbo.expressive.core.data.SyntheticTrailSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.KartverketTideRepository
import com.sigmundgranaas.turbo.expressive.core.data.HttpRadarRepository
import com.sigmundgranaas.turbo.expressive.core.data.RadarRepository
import com.sigmundgranaas.turbo.expressive.core.data.TideRepository
import com.sigmundgranaas.turbo.expressive.core.data.TrailSearchRepository
import com.sigmundgranaas.turbo.expressive.core.data.BuildConfig
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
object NetworkModule {
    // ── Offline stand-ins (emulator only) ───────────────────────────────────────
    // Every networked, Norway-only backend has a synthetic stand-in so the app is
    // driveable on the emulator / a dev box with no network. That's the ONLY reason
    // they exist — so gate on "running on an emulator", NOT on BuildConfig.DEBUG.
    // A debug build dogfooded on a real device must hit the live backends (else the
    // route is a straight line, search is fake, etc.). Release always uses HTTP.
    private val useSynthetic: Boolean = BuildConfig.DEBUG && isEmulator()

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
         * Pick the router: the real trail-bound SSE pathfinder on any real device
         * (debug or release), the offline [SyntheticRouteRepository] only on the
         * emulator (where the router isn't reachable), so the Route builder + Follow
         * can still be driven there. See [useSynthetic].
         */
        @Provides
        @Singleton
        fun provideRouteRepository(
            http: HttpRouteRepository,
            synthetic: SyntheticRouteRepository,
        ): RouteRepository = if (useSynthetic) synthetic else http

        /** Offline conditions stand-in in DEBUG (MET/Varsom unreachable on the emulator). */
        @Provides
        @Singleton
        fun provideConditionsRepository(
            http: HttpConditionsRepository,
            synthetic: SyntheticConditionsRepository,
        ): ConditionsRepository = if (useSynthetic) synthetic else http

        /** Tide predictions (Kartverket sehavniva); synthetic in DEBUG. */
        @Provides
        @Singleton
        fun provideTideRepository(
            http: KartverketTideRepository,
            synthetic: SyntheticTideRepository,
        ): TideRepository = if (useSynthetic) synthetic else http

        /**
         * Gridded weather for the cloud overlay (MET locationforecast samples).
         * Always the live impl — even in DEBUG — so the overlay shows real
         * weather where there's a network; the feature layer falls back to its
         * synthetic storm when a fetch fails (offline).
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
