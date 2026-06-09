package com.sigmundgranaas.turbo.expressive.feature.offline

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.map.NetworkMonitor
import com.sigmundgranaas.turbo.expressive.core.map.OfflineCoverage
import com.sigmundgranaas.turbo.expressive.core.map.OfflineTileManager
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

/**
 * Feeds the map's "you're offline" affordance: whether the device currently has
 * no validated connectivity, and whether a map position falls inside any
 * downloaded region (so the chip can say "outside downloaded areas" — the moment
 * a blank basemap most needs explaining).
 */
@HiltViewModel
class OfflineIndicatorViewModel @Inject constructor(
    network: NetworkMonitor,
    private val manager: OfflineTileManager,
) : ViewModel() {

    val offline: StateFlow<Boolean> = network.state
        .map { !it.connected }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), false)

    /** True when [centre] is inside a complete downloaded region. */
    fun covered(centre: LatLng): Boolean = OfflineCoverage.covers(manager.regions.value, centre)
}
