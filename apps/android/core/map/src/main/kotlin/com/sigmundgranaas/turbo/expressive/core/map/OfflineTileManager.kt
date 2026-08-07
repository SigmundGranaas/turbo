package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.OfflineEstimate
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.flow.StateFlow
import okhttp3.OkHttpClient
import javax.inject.Singleton

/**
 * Manages offline map regions: downloading a region's tiles for no-network use,
 * tracking progress, and listing/deleting downloaded regions. The implementation
 * ([WgpuOfflineTileManager]) pre-populates the wgpu map's on-disk tile store; the
 * rest of the app sees only this seam + the domain [OfflineRegionInfo].
 */
interface OfflineTileManager {
    val regions: StateFlow<List<OfflineRegionInfo>>
    fun refresh()
    fun download(spec: DownloadSpec)

    /** Re-activate a failed region's download. */
    fun retry(id: Long)

    /** Stop a region's download without discarding the tiles already fetched. */
    fun pause(id: Long)

    /** Continue a paused region's download (subject to the network policy). */
    fun resume(id: Long)

    /** Rewrite a region's display name (the tiles are untouched). */
    fun rename(id: Long, name: String)
    fun delete(id: Long)

    /** Drop the ambient (browse) cache; explicit offline regions are untouched. */
    fun clearAmbientCache()

    /**
     * Gate all in-flight downloads on connectivity: when [allowed] is false the
     * active regions are paused; when it flips back they resume (except ones the
     * user paused explicitly). Driven by the foreground service's network policy.
     */
    fun setNetworkAllowed(allowed: Boolean)

    /** Pre-flight tile/byte estimate for [spec] (no I/O). */
    fun estimate(spec: DownloadSpec): OfflineEstimate
}

@Module
@InstallIn(SingletonComponent::class)
object OfflineModule {
    @Provides
    @Singleton
    fun provideOfflineTileManager(impl: WgpuOfflineTileManager): OfflineTileManager = impl

    @Provides
    @Singleton
    fun provideNetworkMonitor(impl: AndroidNetworkMonitor): NetworkMonitor = impl

    /**
     * One OkHttp client for everything that fetches map or pack data.
     *
     * There was no shared client before this, only the phrase "the app's
     * client" in comments describing one that did not exist: the tile
     * fetcher built one, the pack fetcher built a second, and the device
     * pack builder a third. Each carries its own connection pool, thread
     * pool and idle connections, and none of them share a socket to a
     * host all three talk to.
     *
     * Timeouts stay per-caller — a tile GET and a cold WCS coverage want
     * very different patience — via `newBuilder`, which shares the pool
     * and dispatcher rather than starting again.
     */
    @Provides
    @Singleton
    fun provideOkHttpClient(): OkHttpClient = OkHttpClient.Builder().build()
}
