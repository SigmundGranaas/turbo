package com.sigmundgranaas.turbo.expressive.feature.settings

import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.domain.ThemeMode
import com.sigmundgranaas.turbo.expressive.domain.UserSettings
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update

/** Shared by the Settings and Advanced Settings screen tests. */
internal class FakeSettingsRepository : SettingsRepository {
    private val state = MutableStateFlow(UserSettings())
    override val settings: Flow<UserSettings> = state
    override suspend fun setCompassOrientation(enabled: Boolean) = state.update { it.copy(compassOrientation = enabled) }
    override suspend fun setFollowLocation(enabled: Boolean) = state.update { it.copy(followLocation = enabled) }
    override suspend fun setMetricUnits(metric: Boolean) = state.update { it.copy(metricUnits = metric) }
    override suspend fun setThemeMode(mode: ThemeMode) = state.update { it.copy(themeMode = mode) }
    override suspend fun setCloudSyncEnabled(enabled: Boolean) = state.update { it.copy(cloudSyncEnabled = enabled) }
    override suspend fun setDownloadOverWifiOnly(enabled: Boolean) = state.update { it.copy(downloadOverWifiOnly = enabled) }
    override suspend fun setBaseLayer(layer: com.sigmundgranaas.turbo.expressive.domain.BaseLayer) = state.update { it.copy(baseLayer = layer) }
    override suspend fun addCustomTileSource(source: com.sigmundgranaas.turbo.expressive.domain.CustomTileSource) =
        state.update { it.copy(customTileSources = it.customTileSources + source, selectedCustomSourceId = source.id) }
    override suspend fun removeCustomTileSource(id: String) =
        state.update { it.copy(customTileSources = it.customTileSources.filterNot { s -> s.id == id }) }
    override suspend fun selectCustomTileSource(id: String?) = state.update { it.copy(selectedCustomSourceId = id) }
    override suspend fun setLocationDotColor(colorHex: String?) = state.update { it.copy(locationDotColorHex = colorHex) }
    override suspend fun setShowHeadingBeam(enabled: Boolean) = state.update { it.copy(showHeadingBeam = enabled) }
    override suspend fun setGestures(gestures: com.sigmundgranaas.turbo.expressive.domain.GestureSettings) = Unit
    override suspend fun setRouteEngine(engine: com.sigmundgranaas.turbo.expressive.domain.RouteEngine) = Unit
    var packSourceUrl: String? = null
    override suspend fun setPackSourceUrl(url: String?) { packSourceUrl = url }
    override suspend fun setRouteShadowCompare(enabled: Boolean) = state.update { it.copy(routeShadowCompare = enabled) }
    override suspend fun setBuildPacksOnDevice(enabled: Boolean) = state.update { it.copy(buildPacksOnDevice = enabled) }
    override suspend fun setExperimentalTrails(enabled: Boolean) = Unit
    override suspend fun setExperimentalClouds(enabled: Boolean) = Unit
    override suspend fun setRotationLocked(enabled: Boolean) = Unit
    override suspend fun setLastCamera(lat: Double, lng: Double, zoom: Double) =
        state.update { it.copy(lastCameraLat = lat, lastCameraLng = lng, lastCameraZoom = zoom) }
}
