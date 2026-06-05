package com.sigmundgranaas.turbo.expressive.feature.offline

import androidx.lifecycle.ViewModel
import com.sigmundgranaas.turbo.expressive.core.map.OfflineTileManager
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.StateFlow
import javax.inject.Inject
import kotlin.math.floor

/**
 * Drives the offline-maps screen and the "download this area" action. All MapLibre
 * work lives behind the [OfflineTileManager] seam in :core:map; this just exposes
 * the region list and translates a camera box into a download with a sensible zoom span.
 */
@HiltViewModel
class OfflineViewModel @Inject constructor(
    private val manager: OfflineTileManager,
) : ViewModel() {

    val regions: StateFlow<List<OfflineRegionInfo>> = manager.regions

    init { manager.refresh() }

    fun refresh() = manager.refresh()

    /**
     * Download the currently-visible [bounds] at base map [base], spanning a few zoom
     * levels around the current camera [fromZoom] so the area is usable when offline.
     */
    fun download(name: String, base: BaseLayer, bounds: GeoBounds, fromZoom: Double) {
        val minZoom = floor(fromZoom).coerceIn(MIN_ZOOM, MAX_ZOOM)
        val maxZoom = (minZoom + ZOOM_SPAN).coerceAtMost(MAX_ZOOM)
        manager.download(name, base, bounds, minZoom, maxZoom)
    }

    fun delete(id: Long) = manager.delete(id)

    private companion object {
        const val MIN_ZOOM = 8.0
        const val MAX_ZOOM = 16.0
        const val ZOOM_SPAN = 4.0
    }
}
