package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.OfflineEstimate
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Offline stand-in for [OfflineTileManager]: the real one pulls tiles from the
 * Kartverket / tileserver WMTS via MapLibre's OfflineManager, which can't be
 * reached from the emulator — so the Offline Maps screen otherwise stays empty
 * and "download this area" does nothing. This keeps an in-memory region list so
 * the list, download-this-area, size estimate, retry and delete can all be driven,
 * including the [OfflineStatus.Failed] path (a too-large area "fails", and retry
 * then "succeeds"). Selected in DEBUG via [OfflineModule]. (No MapLibre here.)
 */
@Singleton
class SyntheticOfflineTileManager @Inject constructor() : OfflineTileManager {
    private val _regions = MutableStateFlow<List<OfflineRegionInfo>>(emptyList())
    override val regions: StateFlow<List<OfflineRegionInfo>> = _regions.asStateFlow()
    private var nextId = 1L

    override fun refresh() = Unit // state is already in memory

    override fun estimate(spec: DownloadSpec): OfflineEstimate = TileMath.estimate(spec)

    override fun download(spec: DownloadSpec) {
        val est = estimate(spec)
        val within = TileMath.isWithinLimits(spec)
        _regions.update {
            it + OfflineRegionInfo(
                id = nextId++,
                name = spec.name,
                status = if (within) OfflineStatus.Complete else OfflineStatus.Failed,
                progress = if (within) 1f else 0f,
                sizeBytes = if (within) est.bytes else 0L,
                tileCount = if (within) est.tiles else 0L,
                base = spec.base,
                overlays = spec.overlays,
                bounds = spec.bounds,
                minZoom = spec.minZoom,
                maxZoom = spec.maxZoom,
                createdAtEpochMs = System.currentTimeMillis(),
                errorReason = if (within) null else "Area too large",
            )
        }
    }

    override fun retry(id: Long) = complete(id)

    override fun pause(id: Long) = _regions.update { list ->
        list.map { if (it.id == id) it.copy(status = OfflineStatus.Paused) else it }
    }

    override fun resume(id: Long) = complete(id)

    // The synthetic manager has no real network, so connectivity gating is a no-op.
    override fun setNetworkAllowed(allowed: Boolean) = Unit

    override fun rename(id: Long, name: String) = _regions.update { list ->
        list.map { if (it.id == id) it.copy(name = name) else it }
    }

    override fun delete(id: Long) = _regions.update { list -> list.filterNot { it.id == id } }

    override fun clearAmbientCache() = Unit // no ambient cache in the simulator

    private fun complete(id: Long) = _regions.update { list ->
        list.map {
            if (it.id == id) it.copy(status = OfflineStatus.Complete, progress = 1f, errorReason = null) else it
        }
    }
}
