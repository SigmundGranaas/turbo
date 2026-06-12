package com.sigmundgranaas.turbo.expressive.core.map

import android.content.Context
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.OfflineEstimate
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import org.maplibre.android.MapLibre
import org.maplibre.android.geometry.LatLng as MlLatLng
import org.maplibre.android.geometry.LatLngBounds
import org.maplibre.android.offline.OfflineManager
import org.maplibre.android.offline.OfflineRegion
import org.maplibre.android.offline.OfflineRegionError
import org.maplibre.android.offline.OfflineRegionStatus
import org.maplibre.android.offline.OfflineTilePyramidRegionDefinition
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Downloads and manages offline map regions via MapLibre's [OfflineManager].
 * Lives in :core:map (the only place allowed to touch MapLibre); the rest of the
 * app sees this seam + the domain [OfflineRegionInfo].
 */
interface OfflineTileManager {
    val regions: StateFlow<List<OfflineRegionInfo>>
    fun refresh()
    fun download(spec: DownloadSpec)

    /** Re-activate a [OfflineStatus.Failed] region's download. */
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

@Singleton
class MapLibreOfflineTileManager @Inject constructor(
    @param:ApplicationContext private val context: Context,
    private val serviceLauncher: OfflineServiceLauncher,
) : OfflineTileManager {

    private val manager: OfflineManager by lazy {
        MapLibre.getInstance(context)
        OfflineManager.getInstance(context).apply {
            setOfflineMapboxTileCountLimit(TileMath.MAX_TILES)
            // Size the browse cache up from MapLibre's ~50 MB default so recently
            // panned areas survive going offline even without an explicit region.
            setMaximumAmbientCacheSize(AMBIENT_CACHE_BYTES, noopFileSourceCallback)
        }
    }
    private val styleServer = LocalStyleServer()
    // MapLibre delivers callbacks off the registering thread in some paths, and a
    // refresh()'s batched statuses interleave with the per-region observer — so all
    // shared mutable state is concurrent and list publication is atomic (see below).
    private val regionsById = java.util.concurrent.ConcurrentHashMap<Long, OfflineRegion>()
    /** Regions that stopped on an error → reason, until the user retries/deletes. */
    private val failures = java.util.concurrent.ConcurrentHashMap<Long, String>()
    /** Regions the user paused explicitly — must NOT auto-resume on connectivity. */
    private val userPaused = java.util.concurrent.ConcurrentHashMap.newKeySet<Long>()
    @Volatile private var networkAllowed = true
    private val _regions = MutableStateFlow<List<OfflineRegionInfo>>(emptyList())
    override val regions: StateFlow<List<OfflineRegionInfo>> = _regions.asStateFlow()

    override fun estimate(spec: DownloadSpec): OfflineEstimate = TileMath.estimate(spec)

    override fun refresh() {
        manager.listOfflineRegions(object : OfflineManager.ListOfflineRegionsCallback {
            override fun onList(offlineRegions: Array<OfflineRegion>?) {
                val list = offlineRegions?.toList().orEmpty()
                regionsById.clear()
                list.forEach { regionsById[it.id] = it; it.setObserver(observerFor(it)) }
                if (list.isEmpty()) { _regions.value = emptyList(); return }
                val acc = java.util.Collections.synchronizedList(mutableListOf<OfflineRegionInfo>())
                val remaining = java.util.concurrent.atomic.AtomicInteger(list.size)
                fun publishIfDone() {
                    // Merge (don't replace): a region created while statuses were being
                    // gathered survives, and deleted regions drop out atomically.
                    if (remaining.decrementAndGet() == 0) {
                        val byId = acc.associateBy { it.id }
                        _regions.update { current ->
                            val currentIds = current.map { it.id }.toSet()
                            (current.map { byId[it.id] ?: it } + acc.filterNot { it.id in currentIds })
                                .filter { it.id in regionsById.keys }
                                .sortedBy { it.name }
                        }
                    }
                }
                list.forEach { region ->
                    region.getStatus(object : OfflineRegion.OfflineRegionStatusCallback {
                        override fun onStatus(status: OfflineRegionStatus?) {
                            if (status != null) acc += region.toInfo(status)
                            publishIfDone()
                        }
                        override fun onError(error: String?) = publishIfDone()
                    })
                }
            }
            override fun onError(error: String) = Unit
        })
    }

    override fun download(spec: DownloadSpec) {
        serviceLauncher.ensureRunning()
        val definition = OfflineTilePyramidRegionDefinition(
            styleServer.styleUrl(spec.base, spec.overlays),
            LatLngBounds.Builder()
                .include(MlLatLng(spec.bounds.north, spec.bounds.east))
                .include(MlLatLng(spec.bounds.south, spec.bounds.west))
                .build(),
            spec.minZoom,
            spec.maxZoom,
            context.resources.displayMetrics.density,
        )
        val metadata = OfflineRegionMetadata.encode(
            OfflineRegionMetadata.Meta(
                name = spec.name,
                base = spec.base,
                overlays = spec.overlays,
                bounds = spec.bounds,
                minZoom = spec.minZoom,
                maxZoom = spec.maxZoom,
                createdAtEpochMs = System.currentTimeMillis(),
            ),
        )
        manager.createOfflineRegion(definition, metadata, object : OfflineManager.CreateOfflineRegionCallback {
            override fun onCreate(offlineRegion: OfflineRegion) {
                regionsById[offlineRegion.id] = offlineRegion
                offlineRegion.setObserver(observerFor(offlineRegion))
                offlineRegion.setDownloadState(if (networkAllowed) OfflineRegion.STATE_ACTIVE else OfflineRegion.STATE_INACTIVE)
                refresh()
            }
            override fun onError(error: String) = Unit
        })
    }

    override fun retry(id: Long) {
        val region = regionsById[id] ?: return
        failures.remove(id)
        userPaused.remove(id)
        serviceLauncher.ensureRunning()
        region.setObserver(observerFor(region))
        region.setDownloadState(if (networkAllowed) OfflineRegion.STATE_ACTIVE else OfflineRegion.STATE_INACTIVE)
        upsert(region.meta())
    }

    override fun pause(id: Long) {
        val region = regionsById[id] ?: return
        userPaused += id
        region.setDownloadState(OfflineRegion.STATE_INACTIVE)
    }

    override fun resume(id: Long) {
        val region = regionsById[id] ?: return
        userPaused -= id
        failures.remove(id)
        serviceLauncher.ensureRunning()
        if (networkAllowed) region.setDownloadState(OfflineRegion.STATE_ACTIVE)
    }

    override fun setNetworkAllowed(allowed: Boolean) {
        networkAllowed = allowed
        regionsById.forEach { (id, region) ->
            val status = _regions.value.firstOrNull { it.id == id }?.status
            if (status == OfflineStatus.Complete || status == OfflineStatus.Failed) return@forEach
            if (id in userPaused) return@forEach
            region.setDownloadState(if (allowed) OfflineRegion.STATE_ACTIVE else OfflineRegion.STATE_INACTIVE)
        }
    }

    override fun rename(id: Long, name: String) {
        val region = regionsById[id] ?: return
        val current = OfflineRegionMetadata.decode(region.metadata)
            ?: OfflineRegionMetadata.Meta(name, BaseLayer.Norgeskart, emptySet(), null, 0.0, 0.0, 0L)
        region.updateMetadata(
            OfflineRegionMetadata.encode(current.copy(name = name)),
            object : OfflineRegion.OfflineRegionUpdateMetadataCallback {
                override fun onUpdate(metadata: ByteArray) {
                    _regions.value.firstOrNull { it.id == id }?.let { upsert(it.copy(name = name)) }
                }
                override fun onError(error: String) = Unit
            },
        )
    }

    override fun delete(id: Long) {
        val region = regionsById[id] ?: return
        region.setDownloadState(OfflineRegion.STATE_INACTIVE)
        region.delete(object : OfflineRegion.OfflineRegionDeleteCallback {
            override fun onDelete() {
                regionsById.remove(id); failures.remove(id); userPaused.remove(id); refresh()
            }
            override fun onError(error: String) = Unit
        })
    }

    override fun clearAmbientCache() {
        manager.clearAmbientCache(noopFileSourceCallback)
    }

    private fun observerFor(region: OfflineRegion) = object : OfflineRegion.OfflineRegionObserver {
        override fun onStatusChanged(status: OfflineRegionStatus) = upsert(region.toInfo(status))
        override fun onError(error: OfflineRegionError) =
            markFailed(region, error.reason?.takeIf { it.isNotBlank() } ?: error.message ?: "Download failed")
        override fun mapboxTileCountLimitExceeded(limit: Long) {
            region.setDownloadState(OfflineRegion.STATE_INACTIVE)
            markFailed(region, "Tile limit exceeded ($limit)")
        }
    }

    private fun markFailed(region: OfflineRegion, reason: String) {
        failures[region.id] = reason
        val current = _regions.value.firstOrNull { it.id == region.id } ?: region.meta()
        upsert(current.copy(status = OfflineStatus.Failed, errorReason = reason))
    }

    private fun upsert(info: OfflineRegionInfo) {
        _regions.update { current -> (current.filterNot { it.id == info.id } + info).sortedBy { it.name } }
    }

    private fun OfflineRegion.toInfo(status: OfflineRegionStatus): OfflineRegionInfo {
        val required = status.requiredResourceCount.coerceAtLeast(1)
        val progress = if (status.isComplete) 1f
            else (status.completedResourceCount.toDouble() / required).toFloat().coerceIn(0f, 1f)
        val st = when {
            failures.containsKey(id) -> OfflineStatus.Failed
            status.isComplete -> OfflineStatus.Complete
            status.downloadState == OfflineRegion.STATE_INACTIVE -> OfflineStatus.Paused
            else -> OfflineStatus.Downloading
        }
        return meta().copy(
            status = st,
            progress = progress,
            sizeBytes = status.completedResourceSize,
            tileCount = status.completedTileCount,
            errorReason = failures[id],
        )
    }

    /** Decode this region's metadata into an [OfflineRegionInfo] shell (status Downloading). */
    private fun OfflineRegion.meta(): OfflineRegionInfo {
        val m = OfflineRegionMetadata.decode(metadata)
        return OfflineRegionInfo(
            id = id,
            name = m?.name ?: "Region",
            status = OfflineStatus.Downloading,
            progress = 0f,
            sizeBytes = 0L,
            tileCount = 0L,
            base = m?.base ?: BaseLayer.Norgeskart,
            overlays = m?.overlays ?: emptySet(),
            bounds = m?.bounds,
            minZoom = m?.minZoom ?: 0.0,
            maxZoom = m?.maxZoom ?: 0.0,
            createdAtEpochMs = m?.createdAtEpochMs ?: 0L,
        )
    }

    private val noopFileSourceCallback = object : OfflineManager.FileSourceCallback {
        override fun onSuccess() = Unit
        override fun onError(message: String) = Unit
    }

    private companion object {
        const val AMBIENT_CACHE_BYTES = 256L * 1024 * 1024
    }
}

@Module
@InstallIn(SingletonComponent::class)
object OfflineModule {
    /** Real MapLibre downloader in release; in-memory [SyntheticOfflineTileManager]
     *  in DEBUG so the Offline Maps screen is driveable without the tileserver. */
    @Provides
    @Singleton
    fun provideOfflineTileManager(
        real: dagger.Lazy<MapLibreOfflineTileManager>,
        synthetic: SyntheticOfflineTileManager,
    ): OfflineTileManager = if (BuildConfig.DEBUG) synthetic else real.get()

    @Provides
    @Singleton
    fun provideNetworkMonitor(impl: AndroidNetworkMonitor): NetworkMonitor = impl
}
