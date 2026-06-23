package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.OfflineEstimate
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.flow.StateFlow
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
}
