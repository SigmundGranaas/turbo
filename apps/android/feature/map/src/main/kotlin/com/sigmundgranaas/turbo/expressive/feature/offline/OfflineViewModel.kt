package com.sigmundgranaas.turbo.expressive.feature.offline

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.core.geo.formatCoords
import com.sigmundgranaas.turbo.expressive.core.map.OfflineTileManager
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.DetailLevel
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
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
    private val reverseGeocode: ReverseGeocodeRepository,
) : ViewModel() {

    val regions: StateFlow<List<OfflineRegionInfo>> = manager.regions

    init { manager.refresh() }

    fun refresh() = manager.refresh()

    /**
     * Download the currently-visible [bounds] at base map [base], spanning a few zoom
     * levels around the current camera [fromZoom] so the area is usable when offline.
     * Names the region by the reverse-geocoded place at [centre] ("Storfjellet" /
     * "Tromsø") rather than raw coordinates, falling back to coordinates off-grid.
     */
    fun download(
        centre: LatLng,
        base: BaseLayer,
        bounds: GeoBounds,
        fromZoom: Double,
        overlays: Set<OverlayId> = emptySet(),
        detail: DetailLevel = DetailLevel.Standard,
    ) {
        viewModelScope.launch {
            val place = (reverseGeocode.describe(centre) as? Outcome.Success)?.value?.title
            manager.download(
                spec(place ?: formatCoords(centre), base, bounds, fromZoom, overlays, detail),
            )
        }
    }

    /** Pre-flight tile/byte estimate (+ within-limits guard) for the visible [bounds]. */
    fun estimate(
        base: BaseLayer,
        bounds: GeoBounds,
        fromZoom: Double,
        overlays: Set<OverlayId> = emptySet(),
        detail: DetailLevel = DetailLevel.Standard,
    ) = manager.estimate(spec("", base, bounds, fromZoom, overlays, detail))

    /** One zoom policy for estimate + download, so the dialog's number is what ships. */
    private fun spec(
        name: String,
        base: BaseLayer,
        bounds: GeoBounds,
        fromZoom: Double,
        overlays: Set<OverlayId>,
        detail: DetailLevel,
    ): DownloadSpec {
        val minZoom = floor(fromZoom).coerceIn(MIN_ZOOM, MAX_ZOOM)
        return DownloadSpec(
            name = name,
            base = base,
            bounds = bounds,
            minZoom = minZoom,
            maxZoom = (minZoom + detail.zoomSpan).coerceAtMost(MAX_ZOOM),
            overlays = overlays,
        )
    }

    fun retry(id: Long) = manager.retry(id)

    fun pause(id: Long) = manager.pause(id)

    fun resume(id: Long) = manager.resume(id)

    fun delete(id: Long) = manager.delete(id)

    private companion object {
        const val MIN_ZOOM = 8.0
        const val MAX_ZOOM = 16.0
    }
}
