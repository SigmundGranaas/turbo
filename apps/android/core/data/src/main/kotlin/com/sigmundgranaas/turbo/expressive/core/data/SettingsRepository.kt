package com.sigmundgranaas.turbo.expressive.core.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.doublePreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.ThemeMode
import com.sigmundgranaas.turbo.expressive.domain.UserSettings
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject
import javax.inject.Singleton

/** Persisted user settings, exposed as a [Flow] so the UI re-renders on change. */
interface SettingsRepository {
    val settings: Flow<UserSettings>
    suspend fun setCompassOrientation(enabled: Boolean)
    suspend fun setFollowLocation(enabled: Boolean)
    suspend fun setMetricUnits(metric: Boolean)
    suspend fun setThemeMode(mode: ThemeMode)
    suspend fun setCloudSyncEnabled(enabled: Boolean)
    suspend fun setDownloadOverWifiOnly(enabled: Boolean)
    suspend fun setBaseLayer(layer: BaseLayer)
    suspend fun setExperimentalWgpuMap(enabled: Boolean)

    /** Persist the map camera so reopening the app returns to where the user left it. */
    suspend fun setLastCamera(lat: Double, lng: Double, zoom: Double)
}

private val Context.settingsDataStore: DataStore<Preferences> by preferencesDataStore(name = "user_settings")

@Singleton
class DataStoreSettingsRepository @Inject constructor(
    @param:ApplicationContext private val context: Context,
) : SettingsRepository {

    private object Keys {
        val COMPASS = booleanPreferencesKey("compass_orientation")
        val FOLLOW = booleanPreferencesKey("follow_location")
        val METRIC = booleanPreferencesKey("metric_units")
        val THEME_MODE = stringPreferencesKey("theme_mode")
        val CLOUD_SYNC = booleanPreferencesKey("cloud_sync_enabled")
        val WIFI_ONLY = booleanPreferencesKey("download_wifi_only")
        val BASE_LAYER = stringPreferencesKey("base_layer")
        val WGPU_MAP = booleanPreferencesKey("experimental_wgpu_map")
        val CAM_LAT = doublePreferencesKey("last_camera_lat")
        val CAM_LNG = doublePreferencesKey("last_camera_lng")
        val CAM_ZOOM = doublePreferencesKey("last_camera_zoom")
    }

    override val settings: Flow<UserSettings> = context.settingsDataStore.data.map { prefs ->
        UserSettings(
            compassOrientation = prefs[Keys.COMPASS] ?: true,
            followLocation = prefs[Keys.FOLLOW] ?: false,
            metricUnits = prefs[Keys.METRIC] ?: true,
            themeMode = prefs[Keys.THEME_MODE]
                ?.let { runCatching { ThemeMode.valueOf(it) }.getOrNull() }
                ?: ThemeMode.System,
            cloudSyncEnabled = prefs[Keys.CLOUD_SYNC] ?: true,
            downloadOverWifiOnly = prefs[Keys.WIFI_ONLY] ?: false,
            baseLayer = prefs[Keys.BASE_LAYER]
                ?.let { id -> BaseLayer.entries.firstOrNull { it.id == id } }
                ?: BaseLayer.Norgeskart,
            experimentalWgpuMap = prefs[Keys.WGPU_MAP] ?: false,
            lastCameraLat = prefs[Keys.CAM_LAT],
            lastCameraLng = prefs[Keys.CAM_LNG],
            lastCameraZoom = prefs[Keys.CAM_ZOOM],
        )
    }

    override suspend fun setCompassOrientation(enabled: Boolean) {
        context.settingsDataStore.edit { it[Keys.COMPASS] = enabled }
    }

    override suspend fun setFollowLocation(enabled: Boolean) {
        context.settingsDataStore.edit { it[Keys.FOLLOW] = enabled }
    }

    override suspend fun setMetricUnits(metric: Boolean) {
        context.settingsDataStore.edit { it[Keys.METRIC] = metric }
    }

    override suspend fun setThemeMode(mode: ThemeMode) {
        context.settingsDataStore.edit { it[Keys.THEME_MODE] = mode.name }
    }

    override suspend fun setCloudSyncEnabled(enabled: Boolean) {
        context.settingsDataStore.edit { it[Keys.CLOUD_SYNC] = enabled }
    }

    override suspend fun setDownloadOverWifiOnly(enabled: Boolean) {
        context.settingsDataStore.edit { it[Keys.WIFI_ONLY] = enabled }
    }

    override suspend fun setBaseLayer(layer: BaseLayer) {
        context.settingsDataStore.edit { it[Keys.BASE_LAYER] = layer.id }
    }

    override suspend fun setExperimentalWgpuMap(enabled: Boolean) {
        context.settingsDataStore.edit { it[Keys.WGPU_MAP] = enabled }
    }

    override suspend fun setLastCamera(lat: Double, lng: Double, zoom: Double) {
        context.settingsDataStore.edit {
            it[Keys.CAM_LAT] = lat
            it[Keys.CAM_LNG] = lng
            it[Keys.CAM_ZOOM] = zoom
        }
    }
}
