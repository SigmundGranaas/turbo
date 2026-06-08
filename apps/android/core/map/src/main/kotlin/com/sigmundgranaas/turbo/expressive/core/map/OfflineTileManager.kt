package com.sigmundgranaas.turbo.expressive.core.map

import android.content.Context
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
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
    fun download(name: String, base: BaseLayer, bounds: GeoBounds, minZoom: Double, maxZoom: Double)
    fun delete(id: Long)
}

@Singleton
class MapLibreOfflineTileManager @Inject constructor(
    @param:ApplicationContext private val context: Context,
) : OfflineTileManager {

    private val manager: OfflineManager by lazy {
        MapLibre.getInstance(context)
        OfflineManager.getInstance(context).apply { setOfflineMapboxTileCountLimit(MAX_TILES) }
    }
    private val styleServer = LocalStyleServer()
    private val regionsById = mutableMapOf<Long, OfflineRegion>()
    private val _regions = MutableStateFlow<List<OfflineRegionInfo>>(emptyList())
    override val regions: StateFlow<List<OfflineRegionInfo>> = _regions.asStateFlow()

    override fun refresh() {
        manager.listOfflineRegions(object : OfflineManager.ListOfflineRegionsCallback {
            override fun onList(offlineRegions: Array<OfflineRegion>?) {
                val list = offlineRegions?.toList().orEmpty()
                regionsById.clear()
                list.forEach { regionsById[it.id] = it }
                if (list.isEmpty()) { _regions.value = emptyList(); return }
                val acc = mutableListOf<OfflineRegionInfo>()
                var remaining = list.size
                list.forEach { region ->
                    region.getStatus(object : OfflineRegion.OfflineRegionStatusCallback {
                        override fun onStatus(status: OfflineRegionStatus?) {
                            if (status != null) acc += region.toInfo(status)
                            if (--remaining == 0) _regions.value = acc.sortedBy { it.name }
                        }
                        override fun onError(error: String?) {
                            if (--remaining == 0) _regions.value = acc.sortedBy { it.name }
                        }
                    })
                }
            }
            override fun onError(error: String) = Unit
        })
    }

    override fun download(name: String, base: BaseLayer, bounds: GeoBounds, minZoom: Double, maxZoom: Double) {
        val definition = OfflineTilePyramidRegionDefinition(
            styleUrl(base),
            LatLngBounds.Builder()
                .include(MlLatLng(bounds.north, bounds.east))
                .include(MlLatLng(bounds.south, bounds.west))
                .build(),
            minZoom,
            maxZoom,
            context.resources.displayMetrics.density,
        )
        manager.createOfflineRegion(definition, name.toByteArray(), object : OfflineManager.CreateOfflineRegionCallback {
            override fun onCreate(offlineRegion: OfflineRegion) {
                regionsById[offlineRegion.id] = offlineRegion
                offlineRegion.setObserver(observerFor(offlineRegion))
                offlineRegion.setDownloadState(OfflineRegion.STATE_ACTIVE)
                refresh()
            }
            override fun onError(error: String) = Unit
        })
    }

    override fun delete(id: Long) {
        val region = regionsById[id] ?: return
        region.setDownloadState(OfflineRegion.STATE_INACTIVE)
        region.delete(object : OfflineRegion.OfflineRegionDeleteCallback {
            override fun onDelete() { regionsById.remove(id); refresh() }
            override fun onError(error: String) = Unit
        })
    }

    private fun observerFor(region: OfflineRegion) = object : OfflineRegion.OfflineRegionObserver {
        override fun onStatusChanged(status: OfflineRegionStatus) = upsert(region.toInfo(status))
        override fun onError(error: OfflineRegionError) = Unit
        override fun mapboxTileCountLimitExceeded(limit: Long) = Unit
    }

    private fun upsert(info: OfflineRegionInfo) {
        _regions.value = (_regions.value.filterNot { it.id == info.id } + info).sortedBy { it.name }
    }

    private fun OfflineRegion.toInfo(status: OfflineRegionStatus): OfflineRegionInfo {
        val name = runCatching { String(metadata) }.getOrDefault("Region")
        val required = status.requiredResourceCount.coerceAtLeast(1)
        val progress = if (status.isComplete) 1f else (status.completedResourceCount.toDouble() / required).toFloat().coerceIn(0f, 1f)
        return OfflineRegionInfo(id, name, status.isComplete, progress, status.completedResourceSize)
    }

    private fun styleUrl(base: BaseLayer): String = styleServer.styleUrl(base)

    private companion object {
        const val MAX_TILES = 100_000L
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
        real: MapLibreOfflineTileManager,
        synthetic: SyntheticOfflineTileManager,
    ): OfflineTileManager = if (BuildConfig.DEBUG) synthetic else real
}
