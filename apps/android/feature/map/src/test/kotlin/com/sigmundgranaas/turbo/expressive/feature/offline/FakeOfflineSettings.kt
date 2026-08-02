package com.sigmundgranaas.turbo.expressive.feature.offline

import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.CustomTileSource
import com.sigmundgranaas.turbo.expressive.domain.GestureSettings
import com.sigmundgranaas.turbo.expressive.domain.RouteEngine
import com.sigmundgranaas.turbo.expressive.domain.ThemeMode
import com.sigmundgranaas.turbo.expressive.domain.UserSettings
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow

/**
 * Settings for the offline tests: only [UserSettings.buildPacksOnDevice]
 * is ever read here, but the interface has to be satisfied whole.
 */
internal class FakeOfflineSettings(buildPacksOnDevice: Boolean = false) : SettingsRepository {
    val state = MutableStateFlow(UserSettings(buildPacksOnDevice = buildPacksOnDevice))
    override val settings: Flow<UserSettings> = state
    override suspend fun setCompassOrientation(enabled: Boolean) = Unit
    override suspend fun setFollowLocation(enabled: Boolean) = Unit
    override suspend fun setMetricUnits(metric: Boolean) = Unit
    override suspend fun setThemeMode(mode: ThemeMode) = Unit
    override suspend fun setCloudSyncEnabled(enabled: Boolean) = Unit
    override suspend fun setDownloadOverWifiOnly(enabled: Boolean) = Unit
    override suspend fun setBaseLayer(layer: BaseLayer) = Unit
    override suspend fun setLocationDotColor(colorHex: String?) = Unit
    override suspend fun setShowHeadingBeam(enabled: Boolean) = Unit
    override suspend fun addCustomTileSource(source: CustomTileSource) = Unit
    override suspend fun removeCustomTileSource(id: String) = Unit
    override suspend fun selectCustomTileSource(id: String?) = Unit
    override suspend fun setGestures(gestures: GestureSettings) = Unit
    override suspend fun setRouteEngine(engine: RouteEngine) = Unit
    override suspend fun setPackSourceUrl(url: String?) = Unit
    override suspend fun setRouteShadowCompare(enabled: Boolean) = Unit
    override suspend fun setBuildPacksOnDevice(enabled: Boolean) {
        state.value = state.value.copy(buildPacksOnDevice = enabled)
    }
    override suspend fun setExperimentalTrails(enabled: Boolean) = Unit
    override suspend fun setExperimentalClouds(enabled: Boolean) = Unit
    override suspend fun setRotationLocked(enabled: Boolean) = Unit
    override suspend fun setLastCamera(lat: Double, lng: Double, zoom: Double) = Unit
}
