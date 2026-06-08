package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.abs

/**
 * Offline stand-in for [OfflineTileManager]: the real one pulls tiles from the
 * Kartverket / tileserver WMTS via MapLibre's OfflineManager, which can't be
 * reached from the emulator — so the Offline Maps screen otherwise stays empty
 * and "download this area" does nothing. This keeps an in-memory region list so
 * the list, download-this-area, size estimate and delete can all be driven.
 * Selected in DEBUG via [OfflineModule]. (No MapLibre here — safe in :core:map.)
 */
@Singleton
class SyntheticOfflineTileManager @Inject constructor() : OfflineTileManager {
    private val _regions = MutableStateFlow<List<OfflineRegionInfo>>(emptyList())
    override val regions: StateFlow<List<OfflineRegionInfo>> = _regions.asStateFlow()
    private var nextId = 1L

    override fun refresh() = Unit // state is already in memory

    override fun download(name: String, base: BaseLayer, bounds: GeoBounds, minZoom: Double, maxZoom: Double) {
        // Estimate a plausible size from the area + zoom span so the list reads real.
        val degArea = abs(bounds.north - bounds.south) * abs(bounds.east - bounds.west)
        val sizeBytes = (degArea * (maxZoom - minZoom + 1) * 9_000_000L).toLong().coerceIn(2_000_000L, 80_000_000L)
        _regions.update { it + OfflineRegionInfo(id = nextId++, name = name, complete = true, progress = 1f, sizeBytes = sizeBytes) }
    }

    override fun delete(id: Long) = _regions.update { list -> list.filterNot { it.id == id } }
}
