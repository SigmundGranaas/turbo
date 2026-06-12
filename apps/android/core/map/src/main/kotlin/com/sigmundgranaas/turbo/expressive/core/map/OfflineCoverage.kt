package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus

/**
 * Pure check for whether a map position is covered by any downloaded offline
 * region — drives the "outside downloaded area" hint when the device is offline.
 */
object OfflineCoverage {

    /** True when [point] lies inside the bounds of any *complete* region. */
    fun covers(regions: List<OfflineRegionInfo>, point: LatLng): Boolean =
        regions.any { region ->
            region.status == OfflineStatus.Complete &&
                region.bounds?.let { b ->
                    point.lat in b.south..b.north && point.lng in b.west..b.east
                } == true
        }
}
